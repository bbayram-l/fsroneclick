//! 1-Click mod installation, configuration generation, and clean manifest-based rollback.

use crate::game::{GameInfo, MANIFEST_NAME, OPTI_PROXY, RESHADE_DLL};
use crate::gpu::{self, Arch};
use crate::net::{self, ProgressCallback};
use crate::renodx::{self, RenoDxMod};
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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

pub const REFRAMEWORK_DINPUT8_URL: &str =
    "https://github.com/praydog/REFramework-nightly/releases/download/latest/dinput8.dll";

pub const CAS_SHADER_FALLBACK: &str = r#"// AMD FidelityFX Contrast Adaptive Sharpening (CAS) for ReShade
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
        return Err(anyhow!("Cannot install: no game executable found in {}", game.path.display()));
    };
    let target_dir = exe_path.parent().unwrap_or(&game.path);

    // Ensure high-performance GPU preference is active in Windows
    gpu::set_high_performance_preference(exe_path);

    let mut placed_files: Vec<String> = Vec::new();

    // 1. Install Capcom REFramework if game runs on RE Engine
    if game.is_re_engine {
        progress(0.1, "Downloading REFramework dinput8.dll for RE Engine");
        let dinput8_dest = target_dir.join("dinput8.dll");
        if !dinput8_dest.is_file() {
            if let Ok(cached) = net::get_or_download_cached(
                client,
                REFRAMEWORK_DINPUT8_URL,
                "reframework_dinput8.dll",
                Box::new(|p, msg| progress(0.1 * p, msg)),
            ) {
                fs::copy(&cached, &dinput8_dest)?;
                placed_files.push("dinput8.dll".to_string());
            }
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
            let _ = net::extract_7z(&opti_archive, &temp_extract);
        }

        progress(0.45, "Deploying OptiScaler & FakeNVAPI proxies");
        // Deploy OptiScaler.dll as recommended hook (e.g. dxgi.dll or winmm.dll)
        let src_opti_dll = find_file_in_dir(&temp_extract, "OptiScaler.dll")
            .ok_or_else(|| anyhow!("OptiScaler.dll not found in extracted package"))?;
        let hook_dll_name = game.compat.recommended_hook.unwrap_or(OPTI_PROXY);
        let dest_hook = target_dir.join(hook_dll_name);
        fs::copy(&src_opti_dll, &dest_hook)?;
        placed_files.push(hook_dll_name.to_string());

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
                let dest = target_dir.join(aux);
                if fs::copy(&src, &dest).is_ok() {
                    placed_files.push(aux.to_string());
                }
            }
        }

        // Deploy D3D12_Optiscaler (DirectX 12 Agility SDK)
        let d3d12_src = temp_extract.join("D3D12_Optiscaler").join("D3D12Core.dll");
        if d3d12_src.is_file() {
            let d3d12_dest_dir = target_dir.join("D3D12_Optiscaler");
            if fs::create_dir_all(&d3d12_dest_dir).is_ok() {
                let d3d12_dest = d3d12_dest_dir.join("D3D12Core.dll");
                if fs::copy(&d3d12_src, &d3d12_dest).is_ok() {
                    placed_files.push("D3D12_Optiscaler/D3D12Core.dll".to_string());
                }
            }
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
                let plugins_dir = target_dir.join("plugins");
                if fs::create_dir_all(&plugins_dir).is_ok() {
                    let patcher_dest = plugins_dir.join("OptiPatcher.asi");
                    if fs::copy(&patcher_src, &patcher_dest).is_ok() {
                        placed_files.push("plugins/OptiPatcher.asi".to_string());
                    }
                }
            }
        }

        // Generate customized OptiScaler.ini tailored for AMD GPU and specific game profile
        progress(0.5, "Generating AMD RDNA optimized OptiScaler configuration");
        let ini_content = generate_optiscaler_ini(config, gpu_arch, &game.compat);
        let ini_path = target_dir.join("OptiScaler.ini");
        fs::write(&ini_path, ini_content)?;
        placed_files.push("OptiScaler.ini".to_string());
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
            let dest_reshade = target_dir.join(RESHADE_DLL);
            fs::copy(&src_reshade, &dest_reshade)?;
            placed_files.push(RESHADE_DLL.to_string());
        }

        // Generate ReShade.ini
        let reshade_ini_content = generate_reshade_ini();
        let reshade_ini_path = target_dir.join("ReShade.ini");
        if !reshade_ini_path.is_file() {
            fs::write(&reshade_ini_path, reshade_ini_content)?;
            placed_files.push("ReShade.ini".to_string());
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
                let dest_addon = target_dir.join(&matched_mod.filename);
                fs::copy(&cached_addon, &dest_addon)?;
                placed_files.push(matched_mod.filename.clone());
            }
        }
    }

    // 5. AMD FidelityFX CAS & ReShade Headers
    if config.enable_cas_sharpening {
        progress(0.85, "Installing AMD FidelityFX CAS & shader headers");
        let shaders_dir = target_dir.join("reshade-shaders").join("Shaders");
        let _ = fs::create_dir_all(&shaders_dir);

        let cas_dest = shaders_dir.join("CAS.fx");
        let mut cas_written = false;
        if let Ok(cached) = net::get_or_download_cached(
            client,
            CAS_SHADER_RAW,
            "CAS.fx",
            Box::new(|_, _| {}),
        ) {
            if fs::copy(&cached, &cas_dest).is_ok() {
                cas_written = true;
            }
        }
        if !cas_written && !cas_dest.is_file() {
            let _ = fs::write(&cas_dest, CAS_SHADER_FALLBACK);
            cas_written = true;
        }
        if cas_written || cas_dest.is_file() {
            placed_files.push("reshade-shaders/Shaders/CAS.fx".to_string());
        }

        let header_files = [
            (RESHADE_FXH_RAW, "ReShade.fxh"),
            (RESHADE_UI_FXH_RAW, "ReShadeUI.fxh"),
        ];

        for (url, name) in header_files {
            let dest = shaders_dir.join(name);
            if !dest.is_file() {
                if let Ok(cached) = net::get_or_download_cached(
                    client,
                    url,
                    name,
                    Box::new(|_, _| {}),
                ) {
                    if fs::copy(&cached, &dest).is_ok() {
                        placed_files.push(format!("reshade-shaders/Shaders/{name}"));
                    }
                }
            }
        }
    }

    // 6. Write Manifest for 1-Click Clean Rollback
    progress(0.95, "Recording uninstaller manifest");
    let manifest_path = target_dir.join(MANIFEST_NAME);
    let mut manifest_content = String::from("# FSRoneclick Installation Manifest\n");
    for file in &placed_files {
        manifest_content.push_str(file);
        manifest_content.push('\n');
    }
    fs::write(manifest_path, manifest_content)?;
    placed_files.push(MANIFEST_NAME.to_string());

    progress(1.0, "Enhancements installed successfully!");
    Ok(placed_files)
}

/// Completely and cleanly rolls back all installed mods recorded in the manifest.
pub fn uninstall(game: &GameInfo) -> Result<Vec<String>> {
    let Some(ref exe_path) = game.exe_path else {
        return Err(anyhow!("No game executable found"));
    };
    let target_dir = exe_path.parent().unwrap_or(&game.path);
    let manifest_path = target_dir.join(MANIFEST_NAME);
    let legacy_manifest_path = target_dir.join(crate::game::LEGACY_MANIFEST_NAME);

    let active_manifest = if manifest_path.is_file() {
        manifest_path
    } else if legacy_manifest_path.is_file() {
        legacy_manifest_path
    } else {
        return Err(anyhow!("No installation manifest found in game folder"));
    };

    let content = fs::read_to_string(&active_manifest)?;
    let mut removed = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let normalized = trimmed.replace('/', std::path::MAIN_SEPARATOR_STR);
        let file_path = target_dir.join(&normalized);
        if file_path.is_file() {
            if fs::remove_file(&file_path).is_ok() {
                removed.push(trimmed.to_string());
            }
        }
    }

    // Remove manifest itself
    let _ = fs::remove_file(&active_manifest);
    removed.push(active_manifest.file_name().unwrap_or_default().to_string_lossy().to_string());

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
    let non_linear = if compat.non_linear_color_space { "true" } else { "false" };
    let spoof = if compat.disable_spoofing { "false" } else { "true" };
    let fsr2_pattern = if compat.fsr2_pattern { "true" } else { "false" };
    let load_asi = if compat.optipatcher_supported { "true" } else { "false" };

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

fn find_file_in_dir(dir: &Path, target_name: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(current) else { continue };
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
