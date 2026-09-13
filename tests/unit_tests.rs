//! Comprehensive unit tests for OptiOneClick.

use optioneclick::game::{GameInfo, GraphicsApi, Store, MANIFEST_NAME, OPTI_PROXY, RESHADE_DLL};
use optioneclick::gpu::Arch;
use optioneclick::installer::{generate_optiscaler_ini, generate_reshade_ini, uninstall, InstallConfig};
use optioneclick::renodx::{self, RenoDxMod};
use std::collections::HashMap;
use std::fs;

#[test]
fn test_title_normalization() {
    assert_eq!(renodx::normalize_title("Cyberpunk 2077"), "cyberpunk2077");
    assert_eq!(renodx::normalize_title("The Witcher 3: Wild Hunt"), "thewitcher3wildhunt");
    assert_eq!(renodx::normalize_title("Alan Wake II"), "alanwakeii");
}

#[test]
fn test_renodx_matching() {
    let mut catalog = HashMap::new();
    catalog.insert(
        "cyberpunk2077".to_string(),
        RenoDxMod {
            game_title: "Cyberpunk 2077".to_string(),
            filename: "renodx-cyberpunk2077.addon64".to_string(),
            download_url: "https://example.com/renodx-cyberpunk2077.addon64".to_string(),
            notes: None,
        },
    );

    // Direct match
    let matched = renodx::match_game("Cyberpunk 2077", &catalog);
    assert!(matched.is_some());
    assert_eq!(matched.unwrap().filename, "renodx-cyberpunk2077.addon64");

    // Substring match
    let matched2 = renodx::match_game("Cyberpunk 2077 (Steam Edition)", &catalog);
    assert!(matched2.is_some());
}

#[test]
fn test_optiscaler_ini_generation_rdna3() {
    let mut config = InstallConfig::default();
    config.enable_frame_generation = true;
    config.enable_fsr4_int8 = true;
    config.enable_reshade = true;
    let compat = optioneclick::compatibility::CompatibilityProfile::default();

    let ini = generate_optiscaler_ini(&config, &Arch::Rdna3, &compat);

    assert!(ini.contains("Dx12Upscaler = fsr31"));
    assert!(ini.contains("Dx11Upscaler = fsr31"));
    assert!(ini.contains("Fsr4Update = true"), "RDNA 3 must have Fsr4Update enabled for INT8 acceleration");
    assert!(ini.contains("Enabled = true"), "Frame generation must be enabled");
    assert!(ini.contains("FGInput = dlssg"));
    assert!(ini.contains("FGOutput = fsrfg"));
    assert!(ini.contains("LoadReshade = true"), "ReShade plugin loading must be enabled");
    assert!(ini.contains("ReshadePath = ReShade64.dll"));
}

#[test]
fn test_optiscaler_ini_generation_non_rdna3() {
    let mut config = InstallConfig::default();
    config.enable_fsr4_int8 = true;
    let compat = optioneclick::compatibility::CompatibilityProfile::default();

    let ini = generate_optiscaler_ini(&config, &Arch::Rdna2, &compat);

    assert!(ini.contains("Fsr4Update = false"), "Non-RDNA 3 should not auto-enable INT8 update");
}

#[test]
fn test_optiscaler_ini_auto_tuning() {
    let config = InstallConfig::default();
    let mut compat = optioneclick::compatibility::CompatibilityProfile::default();
    compat.auto_exposure = true;
    compat.non_linear_color_space = true;
    compat.disable_spoofing = true;
    compat.fsr2_pattern = true;
    compat.optipatcher_supported = true;

    let ini = generate_optiscaler_ini(&config, &Arch::Rdna3, &compat);

    assert!(ini.contains("AutoExposure = true"));
    assert!(ini.contains("FsrNonLinearColorSpace = true"));
    assert!(ini.contains("SpoofNvidia = false"));
    assert!(ini.contains("Fsr2Pattern = true"));
    assert!(ini.contains("LoadAsiPlugins = true"), "OptiPatcher should enable LoadAsiPlugins");
}

#[test]
fn test_compatibility_database_lookups() {
    use optioneclick::compatibility::{lookup_compatibility, OptiStatus};

    // 1. Star Wars Outlaws (Working in OptiScaler + Redstone with FG)
    let swo = lookup_compatibility("Star Wars Outlaws");
    assert_eq!(swo.opti_status, Some(OptiStatus::Working));
    assert!(swo.fsr_redstone_supported, "Star Wars Outlaws must be in FSR Redstone catalog");
    assert!(swo.fsr_redstone_native_fg, "Star Wars Outlaws has native Redstone Frame Gen");

    // 2. Mortal Shell II (In both OptiScaler and Redstone)
    let ms2 = lookup_compatibility("Mortal Shell II");
    assert_eq!(ms2.opti_status, Some(OptiStatus::Working));
    assert!(ms2.fsr_redstone_supported);

    // 3. Cyberpunk 2077 (In OptiScaler + Redstone with FG)
    let cp = lookup_compatibility("Cyberpunk 2077");
    assert_eq!(cp.opti_status, Some(OptiStatus::Working));
    assert!(cp.fsr_redstone_supported);
    assert!(cp.fsr_redstone_native_fg);

    // 4. Avowed (Has AutoExposure tweak)
    let avowed = lookup_compatibility("Avowed");
    assert_eq!(avowed.opti_status, Some(OptiStatus::Working));
    assert!(avowed.auto_exposure, "Avowed should auto-tune AutoExposure");

    // 5. A Quiet Place: The Road Ahead (Requires winmm.dll hook)
    let qp = lookup_compatibility("A Quiet Place: The Road Ahead");
    assert_eq!(qp.recommended_hook, Some("winmm.dll"));
}

#[test]
fn test_reshade_ini_generation() {
    let ini = generate_reshade_ini();
    assert!(ini.contains("EffectSearchPaths = .\\reshade-shaders\\Shaders"));
    assert!(ini.contains("TextureSearchPaths = .\\reshade-shaders\\Textures"));
}

#[test]
fn test_manifest_uninstallation_clean_rollback() {
    let temp_dir = std::env::temp_dir().join(format!("optioneclick_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a dummy game file that should NOT be deleted
    let game_exe = temp_dir.join("game.exe");
    fs::write(&game_exe, b"dummy game executable").unwrap();
    let game_data = temp_dir.join("data.pak");
    fs::write(&game_data, b"dummy game data").unwrap();

    // Create mock installed mod files
    let mod_dxgi = temp_dir.join(OPTI_PROXY);
    fs::write(&mod_dxgi, b"mock OptiScaler dxgi.dll").unwrap();
    let mod_reshade = temp_dir.join(RESHADE_DLL);
    fs::write(&mod_reshade, b"mock ReShade64.dll").unwrap();
    let mod_ini = temp_dir.join("OptiScaler.ini");
    fs::write(&mod_ini, b"mock OptiScaler.ini").unwrap();

    // Create mock plugins/OptiPatcher.asi
    let plugins_dir = temp_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    let mod_patcher = plugins_dir.join("OptiPatcher.asi");
    fs::write(&mod_patcher, b"mock OptiPatcher.asi").unwrap();

    // Create mock D3D12_Optiscaler/D3D12Core.dll
    let d3d12_dir = temp_dir.join("D3D12_Optiscaler");
    fs::create_dir_all(&d3d12_dir).unwrap();
    let mod_d3d12 = d3d12_dir.join("D3D12Core.dll");
    fs::write(&mod_d3d12, b"mock D3D12Core.dll").unwrap();

    // Write manifest
    let manifest_path = temp_dir.join(MANIFEST_NAME);
    let manifest_content = format!(
        "{OPTI_PROXY}\n{RESHADE_DLL}\nOptiScaler.ini\nplugins/OptiPatcher.asi\nD3D12_Optiscaler/D3D12Core.dll\n"
    );
    fs::write(&manifest_path, manifest_content).unwrap();

    let game_info = GameInfo {
        title: "Test Game".to_string(),
        path: temp_dir.clone(),
        exe_path: Some(game_exe.clone()),
        store: Store::Custom,
        app_id: None,
        api: GraphicsApi::DirectX12,
        is_64bit: true,
        has_native_dlss: true,
        is_re_engine: false,
        anticheat: None,
        fidelityfx_tier: None,
        compat: optioneclick::compatibility::CompatibilityProfile::default(),
    };

    // Verify initial state
    let mod_status = game_info.check_mod_status();
    assert!(mod_status.is_optiscaler_installed);

    // Perform rollback
    let removed = uninstall(&game_info).expect("Uninstall should succeed");
    assert_eq!(removed.len(), 6); // dxgi, reshade, ini, patcher, d3d12core, manifest

    // Verify that mod files are gone
    assert!(!mod_dxgi.exists(), "dxgi.dll should be removed");
    assert!(!mod_reshade.exists(), "ReShade64.dll should be removed");
    assert!(!mod_ini.exists(), "OptiScaler.ini should be removed");
    assert!(!mod_patcher.exists(), "OptiPatcher.asi should be removed");
    assert!(!plugins_dir.exists(), "plugins dir should be cleaned up");
    assert!(!mod_d3d12.exists(), "D3D12Core.dll should be removed");
    assert!(!d3d12_dir.exists(), "D3D12_Optiscaler dir should be cleaned up");
    assert!(!manifest_path.exists(), "Manifest should be removed");

    // Verify that original game files are untouched!
    assert!(game_exe.is_file(), "Original game.exe must be preserved");
    assert!(game_data.is_file(), "Original data.pak must be preserved");

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_fidelityfx_catalog() {
    let catalog = optioneclick::fidelityfx::FidelityFxCatalog::new();
    let dummy_path = std::path::Path::new("dummy");

    assert_eq!(
        catalog.check_game("Starfield", dummy_path),
        Some(optioneclick::fidelityfx::FidelityFxTier::Fsr3WithFrameGen)
    );
    assert_eq!(
        catalog.check_game("Ghost of Tsushima", dummy_path),
        Some(optioneclick::fidelityfx::FidelityFxTier::Fsr3WithFrameGen)
    );
    assert_eq!(
        catalog.check_game("Far Cry 6", dummy_path),
        Some(optioneclick::fidelityfx::FidelityFxTier::Fsr2)
    );
    assert_eq!(
        catalog.check_game("Far Cry 5", dummy_path),
        None // Far Cry 5 is a pre-FSR title
    );
}

#[test]
fn test_updater_parse_raw_compatibility_md() {
    use optioneclick::updater::parse_raw_compatibility_md;

    let sample_md = r#"
# Compatibility List

| Game | Compatibility | Inputs | OptiPatcher | Notes / Workarounds | Last Tested |
| :--- | :--- | :--- | :--- | :--- | :--- |
| [Far Cry 5](https://store.steampowered.com/app/552520/) | ✅ Working | DLSS | ❌ | Requires winmm.dll hook and AutoExposure on. | v0.9.4 |
| [Sample Broken Game](https://example.com) | ❌ Broken | XeSS | ❌ | Crashing at startup. | v0.9.3 |
| [Special Title](https://example.com) | 🟡 Partial | DLSS<br>FSR2 | ✨ Yes | Use Non-Linear Color Space and fsr2pattern. | v0.9.4 |
"#;

    let entries = parse_raw_compatibility_md(sample_md);
    assert_eq!(entries.len(), 3);

    // Entry 1: Far Cry 5
    assert_eq!(entries[0].name, "Far Cry 5");
    assert_eq!(entries[0].status, "Working");
    assert_eq!(entries[0].recommended_hook, Some("winmm.dll".to_string()));
    assert!(entries[0].auto_exposure);

    // Entry 2: Sample Broken Game
    assert_eq!(entries[1].name, "Sample Broken Game");
    assert_eq!(entries[1].status, "NotWorking");

    // Entry 3: Special Title
    assert_eq!(entries[2].name, "Special Title");
    assert_eq!(entries[2].status, "Partial");
    assert!(entries[2].optipatcher);
    assert!(entries[2].non_linear_color_space);
    assert!(entries[2].fsr2_pattern);
}

#[test]
fn test_updater_dynamic_database_overlay() {
    use optioneclick::compatibility::lookup_compatibility;
    use optioneclick::updater::parse_raw_compatibility_md;

    // Parse a custom dummy entry
    let custom_md = r#"
| Game | Compatibility | Inputs | OptiPatcher | Notes / Workarounds | Last Tested |
| :--- | :--- | :--- | :--- | :--- | :--- |
| [Brand New Test Game 2026](https://example.com) | ✅ Working | DLSS | ✨ Yes | Works perfectly with AutoExposure on. | v0.9.5 |
"#;
    let entries = parse_raw_compatibility_md(custom_md);
    assert_eq!(entries.len(), 1);

    // Load into dynamic cache overlay
    optioneclick::compatibility::load_dynamic_entries(&entries);

    // Verify lookup finds the newly injected dynamic entry
    let found = lookup_compatibility("Brand New Test Game 2026");
    assert_eq!(found.opti_status, Some(optioneclick::compatibility::OptiStatus::Working));
    assert!(found.optipatcher_supported);
    assert!(found.auto_exposure);
}

