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

// Pinned downloads: bump the version, URL, and SHA-256 together.
pub const OPTISCALER_VERSION: &str = "0.9.4";
pub const OPTISCALER_DIRECT_URL: &str =
    "https://github.com/optiscaler/OptiScaler/releases/download/v0.9.4/Optiscaler_0.9.4-final.20260718._MM.7z";
/// Matches the digest GitHub publishes for the v0.9.4 release asset.
pub const OPTISCALER_SHA256: &str =
    "575cb4df866116093df75af607e37fd70e10f5163e0f23fd5c804142e80ef0ad";

pub const RESHADE_VERSION: &str = "6.4.1";
pub const RESHADE_ADDON_SETUP_URL: &str =
    "https://reshade.me/downloads/ReShade_Setup_6.4.1_Addon.exe";
pub const RESHADE_ADDON_SETUP_SHA256: &str =
    "130800ad62ee9743f85f24542a7b97fd29b2c18d427f5ad3540cddfeb6ce6e62";

// Rolling downloads: always the latest release, verified against the SHA-256 GitHub publishes.
pub const OPTIPATCHER_REPO: &str = "optiscaler/OptiPatcher";
pub const REFRAMEWORK_REPO: &str = "praydog/REFramework-nightly";

/// Progress messages for optional components that could not be installed start with this, so
/// front ends can keep them visible after the install finishes.
pub const SKIPPED_PREFIX: &str = "Skipping ";

/// Extraction folders written by earlier versions, whose contents were never tied to a verified
/// archive.
const LEGACY_EXTRACT_DIRS: [&str; 4] = [
    "optiscaler_extracted_0.9.4",
    "reshade_extracted",
    "reshade_6.4.1_extracted",
    "reframework_extracted",
];

pub const CAS_SHADER_RAW: &str =
    "https://raw.githubusercontent.com/OopyDoopy/CAS-for-Reshade/master/CAS.fx";
pub const RESHADE_FXH_RAW: &str =
    "https://raw.githubusercontent.com/crosire/reshade-shaders/master/Shaders/ReShade.fxh";
pub const RESHADE_UI_FXH_RAW: &str =
    "https://raw.githubusercontent.com/crosire/reshade-shaders/master/Shaders/ReShadeUI.fxh";

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
    /// Upgrade FSR 3.1 to FSR 4 on GPUs that support it (FP8 on RDNA 4, INT8 on RDNA 3).
    pub enable_fsr4: bool,
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
            enable_fsr4: true,
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
    let cache = net::cache_dir();
    for legacy in LEGACY_EXTRACT_DIRS {
        let _ = fs::remove_dir_all(cache.join(legacy));
    }

    // 1. Install Capcom REFramework if game runs on RE Engine
    if game.is_re_engine && !target_dir.join("dinput8.dll").is_file() {
        progress(0.1, "Downloading REFramework for RE Engine");
        let reframework = net::get_latest_github_asset(
            client,
            REFRAMEWORK_REPO,
            "REFramework.zip",
            Box::new(|p, msg| progress(0.1 * p, msg)),
        )
        .and_then(|(zip, sha256)| {
            net::extract_once(&cache, &zip, &sha256, "reframework", net::extract_zip)
        })
        .and_then(|extracted| {
            find_file_in_dir(&extracted, "dinput8.dll")
                .ok_or_else(|| anyhow!("dinput8.dll not found in REFramework.zip"))
        });
        match reframework {
            Ok(src) => deployment.copy_file(&src, "dinput8.dll")?,
            Err(e) => progress(0.15, &format!("{SKIPPED_PREFIX}REFramework: {e:#}")),
        }
    }

    // 2. OptiScaler Deployment (FSR 3.1 / FSR 4.1.1 + Frame Gen)
    if config.enable_optiscaler {
        progress(
            0.2,
            &format!("Acquiring OptiScaler v{OPTISCALER_VERSION} release"),
        );
        let opti_archive = net::get_or_download_verified(
            client,
            OPTISCALER_DIRECT_URL,
            &format!("OptiScaler_{OPTISCALER_VERSION}.7z"),
            OPTISCALER_SHA256,
            Box::new(|p, msg| progress(0.2 + 0.2 * p, msg)),
        )?;

        progress(0.4, "Decompressing OptiScaler assets");
        let temp_extract = net::extract_once(
            &cache,
            &opti_archive,
            OPTISCALER_SHA256,
            "optiscaler",
            net::extract_7z,
        )?;

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
            let patcher = net::get_latest_github_asset(
                client,
                OPTIPATCHER_REPO,
                "OptiPatcher.asi",
                Box::new(|_, _| {}),
            );
            match patcher {
                Ok((src, _)) => {
                    let _ = deployment.copy_file(&src, "plugins/OptiPatcher.asi");
                }
                Err(e) => progress(0.49, &format!("{SKIPPED_PREFIX}OptiPatcher: {e:#}")),
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
        let reshade_exe = net::get_or_download_verified(
            client,
            RESHADE_ADDON_SETUP_URL,
            &format!("ReShade_Setup_{RESHADE_VERSION}_Addon.exe"),
            RESHADE_ADDON_SETUP_SHA256,
            Box::new(|p, msg| progress(0.55 + 0.15 * p, msg)),
        )?;

        progress(0.7, "Extracting ReShade64.dll from installer");
        // The add-on setup is a self-extracting archive: read it as a zip, or as 7z if that fails.
        let temp_reshade = net::extract_once(
            &cache,
            &reshade_exe,
            RESHADE_ADDON_SETUP_SHA256,
            "reshade",
            |archive, out| {
                net::extract_zip(archive, out).or_else(|_| net::extract_7z(archive, out))
            },
        )?;
        let src_reshade = find_file_in_dir(&temp_reshade, "ReShade64.dll")
            .ok_or_else(|| anyhow!("ReShade64.dll not found in the ReShade installer"))?;
        deployment.copy_file(&src_reshade, RESHADE_DLL)?;

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
            // RenoDX snapshots are rebuilt in place without checksums, so fetch the current
            // build each time and only fall back to the cached copy when offline.
            let cached_addon = net::cache_dir().join(net::url_cache_name(
                &matched_mod.download_url,
                &matched_mod.filename,
            ));
            let downloaded = net::download_refreshing(
                client,
                &matched_mod.download_url,
                &cached_addon,
                Box::new(|p, msg| progress(0.75 + 0.1 * p, msg)),
            );
            match downloaded {
                Ok(()) => deployment.copy_file(&cached_addon, &matched_mod.filename)?,
                Err(e) => progress(0.85, &format!("{SKIPPED_PREFIX}RenoDX HDR mod: {e:#}")),
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
    // OptiScaler v0.9.4: Fsr4Update upgrades FSR 3.1 to FSR 4, and Fsr4ForceEnableInt8 selects
    // the INT8 model that GPUs without FP8 support (RDNA 3) need.
    let fsr4_update = config.enable_fsr4 && gpu_arch.supports_fsr4();
    let fsr4_int8 = config.enable_fsr4 && gpu_arch.supports_fsr4_int8();

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
Fsr4ForceEnableInt8 = {fsr4_int8}
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

[Spoofing]
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
        fsr4_int8 = fsr4_int8,
        auto_exp = auto_exp,
        non_linear = non_linear,
        fsr2_pattern = fsr2_pattern,
        fg_enabled = config.enable_frame_generation,
        load_asi = load_asi,
        load_reshade = config.enable_reshade,
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
