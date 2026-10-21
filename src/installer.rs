//! 1-Click mod installation, configuration generation, and clean manifest-based rollback.

use crate::game::{GameInfo, LEGACY_MANIFEST_NAME, MANIFEST_NAME, OPTI_PROXY, RESHADE_DLL};
use crate::gpu::{self, Arch};
use crate::net::{self, ProgressCallback};
use crate::renodx::{self, RenoDxMod};
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const OPTISCALER_DIRECT_URL: &str =
    "https://github.com/optiscaler/OptiScaler/releases/download/v0.9.4/Optiscaler_0.9.4-final.20260718._MM.7z";

pub const OPTIPATCHER_URL: &str =
    "https://github.com/optiscaler/OptiPatcher/releases/latest/download/OptiPatcher.asi";

pub const RESHADE_ADDON_SETUP_URL: &str =
    "https://reshade.me/downloads/ReShade_Setup_6.4.1_Addon.exe";

pub const CAS_SHADER_RAW: &str =
    "https://raw.githubusercontent.com/OopyDoopy/CAS-for-Reshade/master/CAS.fx";
pub const RESHADE_FXH_RAW: &str =
    "https://raw.githubusercontent.com/crosire/reshade-shaders/master/Shaders/ReShade.fxh";
pub const RESHADE_UI_FXH_RAW: &str =
    "https://raw.githubusercontent.com/crosire/reshade-shaders/master/Shaders/ReShadeUI.fxh";

/// Universal REFramework nightly build; the archive contains `dinput8.dll`.
pub const REFRAMEWORK_ZIP_URL: &str =
    "https://github.com/praydog/REFramework-nightly/releases/latest/download/REFramework.zip";

/// Suffix given to a game's own file when an install has to overwrite it; uninstall restores it.
pub const BACKUP_SUFFIX: &str = ".fsroneclick-backup";

pub const CAS_SHADER_FALLBACK: &str = r#"// AMD FidelityFX Contrast Adaptive Sharpening (CAS) for ReShade
//
// Based on AMD FidelityFX CAS.
// Copyright (c) 2017-2019 Advanced Micro Devices, Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of
// the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
// THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
uniform float Contrast <
	ui_type = "drag";
    ui_label = "Contrast Adaptation";
    ui_tooltip = "Adjusts the range the shader adapts to high contrast.";
	ui_min = 0.0; ui_max = 1.0;
> = 0.0;

uniform float Sharpening <
	ui_type = "drag";
    ui_label = "Sharpening intensity";
    ui_tooltip = "Adjusts sharpening intensity. 1.0 is default.";
	ui_min = 0.0; ui_max = 1.0;
> = 1.0;

#include "ReShade.fxh"
texture TexColor : COLOR;
sampler sTexColor {Texture = TexColor; SRGBTexture = true;};

float3 CASPass(float4 vpos : SV_Position, float2 texcoord : TexCoord) : SV_Target
{
    float3 a = tex2Doffset(sTexColor, texcoord, int2(-1, -1)).rgb;
    float3 b = tex2Doffset(sTexColor, texcoord, int2(0, -1)).rgb;
    float3 c = tex2Doffset(sTexColor, texcoord, int2(1, -1)).rgb;
    float3 d = tex2Doffset(sTexColor, texcoord, int2(-1, 0)).rgb;
    float3 e = tex2Doffset(sTexColor, texcoord, int2(0, 0)).rgb;
    float3 f = tex2Doffset(sTexColor, texcoord, int2(1, 0)).rgb;
    float3 g = tex2Doffset(sTexColor, texcoord, int2(-1, 1)).rgb;
    float3 h = tex2Doffset(sTexColor, texcoord, int2(0, 1)).rgb;
    float3 i = tex2Doffset(sTexColor, texcoord, int2(1, 1)).rgb;

    float3 mnRGB = min(min(min(d, e), min(f, b)), h);
    float3 mnRGB2 = min(mnRGB, min(min(a, c), min(g, i)));
    mnRGB += mnRGB2;

    float3 mxRGB = max(max(max(d, e), max(f, b)), h);
    float3 mxRGB2 = max(mxRGB, max(max(a, c), max(g, i)));
    mxRGB += mxRGB2;

    float3 rcpMRGB = rcp(mxRGB);
    float3 ampRGB = saturate(min(mnRGB, 2.0 - mxRGB) * rcpMRGB);
    ampRGB = rsqrt(ampRGB);

    float peak = 8.0 - 3.0 * Contrast;
    float3 wRGB = -rcp(ampRGB * peak);
    float3 rcpWeightRGB = rcp(1.0 + 4.0 * wRGB);

    float3 window = (b + d) + (f + h);
    float3 outColor = saturate((window * wRGB + e) * rcpWeightRGB);
    return lerp(e, outColor, Sharpening);
}

technique ContrastAdaptiveSharpen
{
	pass
	{
		VertexShader = PostProcessVS;
		PixelShader = CASPass;
		SRGBWriteEnable = true;
	}
}
"#;

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub enable_optiscaler: bool,
    pub enable_frame_generation: bool,
    pub enable_fsr4_int8: bool,
    pub enable_reshade: bool,
    pub enable_renodx: bool,
    pub enable_cas_sharpening: bool,
    pub enable_lumenite_fx: bool,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            enable_optiscaler: true,
            enable_frame_generation: true,
            enable_fsr4_int8: true,
            enable_reshade: true,
            enable_renodx: true,
            enable_cas_sharpening: true,
            enable_lumenite_fx: false,
        }
    }
}

/// Performs a 1-click installation of the full enhancement suite to a game.
pub fn install(
    client: &Client,
    game: &GameInfo,
    config: &InstallConfig,
    gpu_arch: &Arch,
    renodx_catalog: &HashMap<String, RenoDxMod>,
    mut progress: ProgressCallback,
) -> Result<Vec<String>> {
    let Some(ref exe_path) = game.exe_path else {
        return Err(anyhow!(
            "Cannot install: no game executable found in {}",
            game.path.display()
        ));
    };
    if !game.is_64bit {
        return Err(anyhow!(
            "Cannot install: {} is a 32-bit game, and OptiScaler and ReShade64 only support 64-bit games",
            game.title
        ));
    }
    let target_dir = exe_path.parent().unwrap_or(&game.path);

    // Ensure high-performance GPU preference is active in Windows
    gpu::set_high_performance_preference(exe_path);

    // Each placed file is recorded in the manifest as it is written, so even a failed or
    // interrupted install can be rolled back.
    let mut deployment = Deployment::begin(target_dir)?;
    deploy_components(
        client,
        game,
        config,
        gpu_arch,
        renodx_catalog,
        &mut deployment,
        &mut progress,
    )?;

    progress(1.0, "Enhancements installed successfully!");
    Ok(deployment.finish())
}

fn deploy_components(
    client: &Client,
    game: &GameInfo,
    config: &InstallConfig,
    gpu_arch: &Arch,
    renodx_catalog: &HashMap<String, RenoDxMod>,
    deployment: &mut Deployment,
    progress: &mut ProgressCallback,
) -> Result<()> {
    let target_dir = deployment.target_dir.clone();

    // 1. Install Capcom REFramework if game runs on RE Engine
    if game.is_re_engine && !target_dir.join("dinput8.dll").is_file() {
        progress(0.1, "Downloading REFramework for RE Engine");
        let reframework = net::get_or_download_cached(
            client,
            REFRAMEWORK_ZIP_URL,
            "REFramework.zip",
            Box::new(|p, msg| progress(0.1 * p, msg)),
        )
        .and_then(|zip| {
            let extracted = net::cache_dir().join("reframework_extracted");
            net::extract_zip(&zip, &extracted)?;
            find_file_in_dir(&extracted, "dinput8.dll")
                .ok_or_else(|| anyhow!("dinput8.dll not found in REFramework.zip"))
        });
        match reframework {
            Ok(src) => deployment.copy_file(&src, "dinput8.dll")?,
            Err(e) => progress(0.15, &format!("Skipping REFramework: {e:#}")),
        }
    }

    // 2. OptiScaler Deployment (FSR 3.1 / FSR 4.1.1 + Frame Gen)
    if config.enable_optiscaler {
        progress(0.2, "Acquiring OptiScaler v0.9.4 release");
        let opti_archive = net::get_or_download_cached(
            client,
            OPTISCALER_DIRECT_URL,
            "OptiScaler_0.9.4.7z",
            Box::new(|p, msg| progress(0.2 + 0.2 * p, msg)),
        )?;

        let temp_extract = net::cache_dir().join("optiscaler_extracted_0.9.4");
        if !temp_extract.join("OptiScaler.dll").is_file() {
            progress(0.4, "Decompressing OptiScaler assets");
            if let Err(e) = net::extract_7z(&opti_archive, &temp_extract) {
                // Drop the cached archive so the next attempt downloads a fresh copy.
                let _ = fs::remove_file(&opti_archive);
                return Err(e);
            }
        }

        progress(0.45, "Deploying OptiScaler & FakeNVAPI proxies");
        // Deploy OptiScaler.dll as recommended hook (e.g. dxgi.dll or winmm.dll)
        let src_opti_dll = find_file_in_dir(&temp_extract, "OptiScaler.dll")
            .ok_or_else(|| anyhow!("OptiScaler.dll not found in extracted package"))?;
        let hook_dll_name = game.compat.recommended_hook.unwrap_or(OPTI_PROXY);
        deployment.copy_file(&src_opti_dll, hook_dll_name)?;

        // Copy auxiliary DLLs (fakenvapi, dlssg_to_fsr3, FFX FG & upscaler, libxess, etc.)
        let aux_files = [
            "nvapi64.dll",
            "fakenvapi.dll",
            "fakenvapi.ini",
            "dlssg_to_fsr3_amd_is_better.dll",
            "amd_fidelityfx_dx12.dll",
            "amd_fidelityfx_framegeneration_dx12.dll",
            "amd_fidelityfx_upscaler_dx12.dll",
            "amd_fidelityfx_vk.dll",
            "libxell.dll",
            "libxess.dll",
            "libxess_dx11.dll",
            "libxess_fg.dll",
        ];
        for aux in aux_files {
            if let Some(src) = find_file_in_dir(&temp_extract, aux) {
                let _ = deployment.copy_file(&src, aux);
            }
        }

        // Deploy D3D12_Optiscaler (DirectX 12 Agility SDK)
        let d3d12_src = temp_extract.join("D3D12_Optiscaler").join("D3D12Core.dll");
        if d3d12_src.is_file() {
            let _ = deployment.copy_file(&d3d12_src, "D3D12_Optiscaler/D3D12Core.dll");
        }

        // Deploy OptiPatcher for UE DLSS/DLSSG titles if supported
        if game.compat.optipatcher_supported {
            progress(0.48, "Deploying OptiPatcher for UE DLSS & Frame Gen unlock");
            let patcher_cached = net::get_or_download_cached(
                client,
                OPTIPATCHER_URL,
                "OptiPatcher.asi",
                Box::new(|_, _| {}),
            );
            if let Ok(patcher_src) = patcher_cached {
                let _ = deployment.copy_file(&patcher_src, "plugins/OptiPatcher.asi");
            }
        }

        // Generate customized OptiScaler.ini tailored for AMD GPU and specific game profile
        progress(
            0.5,
            "Generating AMD RDNA optimized OptiScaler configuration",
        );
        let ini_content = generate_optiscaler_ini(config, gpu_arch, &game.compat);
        deployment.write_file("OptiScaler.ini", ini_content)?;
    }

    // 3. ReShade (Add-on build) Deployment
    if config.enable_reshade {
        progress(0.55, "Fetching ReShade with Add-on support");
        let reshade_exe = net::get_or_download_cached(
            client,
            RESHADE_ADDON_SETUP_URL,
            "ReShade_Setup_Addon.exe",
            Box::new(|p, msg| progress(0.55 + 0.15 * p, msg)),
        )?;

        let temp_reshade = net::cache_dir().join("reshade_extracted");
        if !temp_reshade.join("ReShade64.dll").is_file() {
            progress(0.7, "Extracting ReShade64.dll from installer");
            let _ = net::extract_7z(&reshade_exe, &temp_reshade);
            let _ = net::extract_zip(&reshade_exe, &temp_reshade);
        }

        if let Some(src_reshade) = find_file_in_dir(&temp_reshade, "ReShade64.dll") {
            deployment.copy_file(&src_reshade, RESHADE_DLL)?;
        }

        // Generate ReShade.ini, keeping any configuration the player already has
        if !target_dir.join("ReShade.ini").is_file() {
            deployment.write_file("ReShade.ini", generate_reshade_ini())?;
        }
    }

    // 4. RenoDX HDR Mod Matching & Deployment
    if config.enable_renodx {
        if let Some(matched_mod) = renodx::match_game(&game.title, renodx_catalog) {
            progress(
                0.75,
                &format!("Matched RenoDX HDR mod for {}", matched_mod.game_title),
            );
            if let Ok(cached_addon) = net::get_or_download_cached(
                client,
                &matched_mod.download_url,
                &matched_mod.filename,
                Box::new(|p, msg| progress(0.75 + 0.1 * p, msg)),
            ) {
                deployment.copy_file(&cached_addon, &matched_mod.filename)?;
            }
        }
    }

    // 5. AMD FidelityFX CAS & ReShade Headers
    if config.enable_cas_sharpening {
        progress(0.85, "Installing AMD FidelityFX CAS & shader headers");
        let cas_entry = "reshade-shaders/Shaders/CAS.fx";
        let downloaded =
            net::get_or_download_cached(client, CAS_SHADER_RAW, "CAS.fx", Box::new(|_, _| {}))
                .and_then(|cached| deployment.copy_file(&cached, cas_entry));
        if downloaded.is_err() && !target_dir.join(cas_entry).is_file() {
            let _ = deployment.write_file(cas_entry, CAS_SHADER_FALLBACK);
        }

        let header_files = [
            (RESHADE_FXH_RAW, "ReShade.fxh"),
            (RESHADE_UI_FXH_RAW, "ReShadeUI.fxh"),
        ];

        for (url, name) in header_files {
            let entry = format!("reshade-shaders/Shaders/{name}");
            if !target_dir.join(&entry).is_file() {
                if let Ok(cached) =
                    net::get_or_download_cached(client, url, name, Box::new(|_, _| {}))
                {
                    let _ = deployment.copy_file(&cached, &entry);
                }
            }
        }
    }

    Ok(())
}

/// Places files into a game folder while recording them for rollback.
///
/// A game file that would be overwritten is first renamed to `<name>` + [`BACKUP_SUFFIX`], and
/// [`uninstall`] puts it back. Entries from an earlier install's manifest are carried over, so
/// re-running an install never orphans files, and the manifest is rewritten after every file so
/// even an interrupted install can be rolled back.
pub struct Deployment {
    target_dir: PathBuf,
    /// Everything the game folder's manifest lists: earlier installs first, then this run.
    entries: Vec<String>,
    placed: Vec<String>,
}

impl Deployment {
    /// Starts a deployment into `target_dir`, carrying over any earlier install's manifest.
    pub fn begin(target_dir: &Path) -> Result<Self> {
        let mut entries: Vec<String> = Vec::new();
        if let Some(manifest) = find_manifest(target_dir) {
            let content = fs::read_to_string(&manifest).with_context(|| {
                format!(
                    "Could not read {}; fix or delete it before installing again",
                    manifest.display()
                )
            })?;
            for entry in manifest_entries(&content) {
                if !entries.iter().any(|e| same_entry(e, entry)) {
                    entries.push(entry.to_string());
                }
            }
        }
        Ok(Self {
            target_dir: target_dir.to_path_buf(),
            entries,
            placed: Vec::new(),
        })
    }

    /// Copies `src` to `entry`, a `/`-separated path relative to the game folder.
    pub fn copy_file(&mut self, src: &Path, entry: &str) -> Result<()> {
        self.place(entry, |dest| fs::copy(src, dest).map(drop))
    }

    /// Writes `contents` to `entry`, a `/`-separated path relative to the game folder.
    pub fn write_file(&mut self, entry: &str, contents: impl AsRef<[u8]>) -> Result<()> {
        self.place(entry, |dest| fs::write(dest, contents))
    }

    /// Returns the entries placed by this run, manifest included.
    pub fn finish(self) -> Vec<String> {
        let mut placed = self.placed;
        if !placed.is_empty() {
            placed.push(MANIFEST_NAME.to_string());
        }
        placed
    }

    fn place(
        &mut self,
        entry: &str,
        write: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> Result<()> {
        let dest = resolve_entry(&self.target_dir, entry)
            .ok_or_else(|| anyhow!("Refusing to write outside the game folder: {entry}"))?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        let existed = dest.is_file();
        let ours = self.entries.iter().any(|e| same_entry(e, entry));
        let backup = backup_path(&dest);
        let backed_up = existed && !ours && !backup.exists();
        if backed_up {
            fs::rename(&dest, &backup)
                .with_context(|| format!("Failed to back up {}", dest.display()))?;
        }
        if let Err(e) = write(&dest) {
            // Leave the folder as it was: drop the partial file and put the original back.
            if backed_up || !existed {
                let _ = fs::remove_file(&dest);
            }
            if backed_up {
                let _ = fs::rename(&backup, &dest);
            }
            return Err(e).with_context(|| format!("Failed to write {}", dest.display()));
        }

        if !ours {
            self.entries.push(entry.to_string());
        }
        if !self.placed.iter().any(|e| same_entry(e, entry)) {
            self.placed.push(entry.to_string());
        }
        write_manifest(&self.target_dir.join(MANIFEST_NAME), &self.entries)?;
        // Any legacy manifest has just been merged into the new one.
        let _ = fs::remove_file(self.target_dir.join(LEGACY_MANIFEST_NAME));
        Ok(())
    }
}

/// Completely and cleanly rolls back all installed mods recorded in the manifest.
///
/// Files that cannot be removed or restored (typically because the game is still running) stay
/// listed in the manifest and are reported as an error, so uninstall can simply be run again.
pub fn uninstall(game: &GameInfo) -> Result<Vec<String>> {
    let Some(ref exe_path) = game.exe_path else {
        return Err(anyhow!("No game executable found"));
    };
    let target_dir = exe_path.parent().unwrap_or(&game.path);
    let active_manifest = find_manifest(target_dir)
        .ok_or_else(|| anyhow!("No installation manifest found in game folder"))?;

    let content = fs::read_to_string(&active_manifest)
        .with_context(|| format!("Could not read {}", active_manifest.display()))?;
    let mut removed = Vec::new();
    let mut remaining = Vec::new();
    let mut handled = HashSet::new();

    for entry in manifest_entries(&content) {
        // Entries that would resolve outside the game folder are never touched.
        let Some(file_path) = resolve_entry(target_dir, entry) else {
            continue;
        };
        // Handle each file once (NTFS ignores case): a repeated entry would otherwise delete
        // the original that was just restored.
        if !handled.insert(file_path.to_string_lossy().to_lowercase()) {
            continue;
        }

        if file_path.is_file() {
            if fs::remove_file(&file_path).is_err() {
                remaining.push(entry.to_string());
                continue;
            }
            removed.push(entry.to_string());
        }
        let backup = backup_path(&file_path);
        if backup.is_file() && !file_path.exists() && fs::rename(&backup, &file_path).is_err() {
            remaining.push(entry.to_string());
        }
    }

    if !remaining.is_empty() {
        write_manifest(&active_manifest, &remaining)?;
        return Err(anyhow!(
            "Could not remove or restore {}. Close the game and run uninstall again.",
            remaining.join(", ")
        ));
    }

    // Remove manifest itself
    let _ = fs::remove_file(&active_manifest);
    removed.push(
        active_manifest
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    );

    // Clean up empty directories if present
    let _ = fs::remove_dir(target_dir.join("plugins"));
    let _ = fs::remove_dir(target_dir.join("D3D12_Optiscaler"));
    let _ = fs::remove_dir(target_dir.join("reshade-shaders").join("Shaders"));
    let _ = fs::remove_dir(target_dir.join("reshade-shaders").join("Textures"));
    let _ = fs::remove_dir(target_dir.join("reshade-shaders"));

    Ok(removed)
}

pub fn generate_optiscaler_ini(
    config: &InstallConfig,
    gpu_arch: &Arch,
    compat: &crate::compatibility::CompatibilityProfile,
) -> String {
    let fsr4_update = if config.enable_fsr4_int8 && gpu_arch.supports_fsr4_int8() {
        "true"
    } else {
        "false"
    };

    let auto_exp = if compat.auto_exposure { "true" } else { "auto" };
    let non_linear = if compat.non_linear_color_space {
        "true"
    } else {
        "false"
    };
    let spoof = if compat.disable_spoofing {
        "false"
    } else {
        "true"
    };
    let fsr2_pattern = if compat.fsr2_pattern { "true" } else { "false" };
    let load_asi = if compat.optipatcher_supported {
        "true"
    } else {
        "false"
    };

    format!(
        r#"; FSRoneclick Configuration - Optimized for AMD Radeon & Universal GPUs
[General]
LogLevel = 1
LogToConsole = false
LogToFile = true

[Upscalers]
Dx11Upscaler = fsr31
Dx12Upscaler = fsr31
VulkanUpscaler = fsr31

[FrameGen]
Enabled = {fg_enabled}
FGInput = dlssg
FGOutput = fsrfg
PreserveSwapChain = true

[FSR]
Fsr4Update = {fsr4_update}
Fsr4ForceEnableInt8 = false
Fsr4EnableWatermark = false
FsrAgilitySDKUpgrade = true
FsrNonLinearColorSpace = {non_linear}

[FSRFG]
EnableWatermark = false

[InitFlags]
AutoExposure = {auto_exp}
HDR = auto
DepthInverted = auto
JitterCancellation = auto
DisplayResolution = auto
DisableReactiveMask = auto

[Plugins]
LoadAsiPlugins = {load_asi}
LoadReshade = {load_reshade}
ReshadePath = {reshade_dll}

[Spoofing]
SpoofNvidia = {spoof}
SpoofGpu = {spoof}
Dxgi = {spoof}
StreamlineSpoofing = true

[Inputs]
EnableDlssInputs = true
EnableXeSSInputs = true
EnableFsr2Inputs = true
EnableFsr3Inputs = true
Fsr2Pattern = {fsr2_pattern}
Fsr3Pattern = false
EnableFfxInputs = true
UseFfxInputs = true

[Log]
LogToFile = true
LogLevel = 1
SingleFile = true
LogFileName = OptiScaler.log

[Hotfix]
ColorResourceBarrier = auto
MotionVectorResourceBarrier = auto
"#,
        fsr4_update = fsr4_update,
        auto_exp = auto_exp,
        non_linear = non_linear,
        fsr2_pattern = fsr2_pattern,
        fg_enabled = config.enable_frame_generation,
        load_asi = load_asi,
        load_reshade = config.enable_reshade,
        reshade_dll = RESHADE_DLL,
        spoof = spoof,
    )
}

pub fn generate_reshade_ini() -> String {
    r#"[GENERAL]
EffectSearchPaths = .\reshade-shaders\Shaders
TextureSearchPaths = .\reshade-shaders\Textures
PreprocessorDefinitions = RESHADE_DEPTH_INPUT_IS_UPSCALED=0
ShowOverlayMessage = 1
"#
    .to_string()
}

fn find_manifest(dir: &Path) -> Option<PathBuf> {
    [MANIFEST_NAME, LEGACY_MANIFEST_NAME]
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn manifest_entries(content: &str) -> impl Iterator<Item = &str> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

/// Resolves a manifest entry inside `dir`. Absolute paths, drive prefixes and `..` are rejected,
/// so neither a tampered manifest nor a remote file name can reach outside the game folder.
fn resolve_entry(dir: &Path, entry: &str) -> Option<PathBuf> {
    let mut resolved = dir.to_path_buf();
    for component in Path::new(entry).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            _ => return None,
        }
    }
    (resolved != dir).then_some(resolved)
}

fn write_manifest(path: &Path, entries: &[String]) -> Result<()> {
    let mut content = String::from("# FSRoneclick Installation Manifest\n");
    for entry in entries {
        content.push_str(entry);
        content.push('\n');
    }
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))
}

/// Manifest entries name files on a case-insensitive file system.
fn same_entry(a: &str, b: &str) -> bool {
    a.replace('\\', "/")
        .eq_ignore_ascii_case(&b.replace('\\', "/"))
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(BACKUP_SUFFIX);
    PathBuf::from(name)
}

fn find_file_in_dir(dir: &Path, target_name: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(current) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(name) = p.file_name() {
                    if name.to_string_lossy().eq_ignore_ascii_case(target_name) {
                        return Some(p);
                    }
                }
            } else if p.is_dir() {
                stack.push(p);
            }
        }
    }
    None
}
