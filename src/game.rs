//! Game discovery, executable inspection, graphics API detection, and mod status tracking.

use crate::fidelityfx::{FidelityFxCatalog, FidelityFxTier};
use std::fs;
use std::path::{Path, PathBuf};

pub const OPTI_PROXY: &str = "dxgi.dll";
pub const RESHADE_DLL: &str = "ReShade64.dll";
pub const MANIFEST_NAME: &str = ".fsroneclick-manifest";
pub const LEGACY_MANIFEST_NAME: &str = ".optioneclick-manifest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsApi {
    DirectX12,
    DirectX11,
    DirectX9,
    Vulkan,
    Unknown,
}

impl GraphicsApi {
    pub fn label(&self) -> &'static str {
        match self {
            GraphicsApi::DirectX12 => "DirectX 12",
            GraphicsApi::DirectX11 => "DirectX 11",
            GraphicsApi::DirectX9 => "DirectX 9",
            GraphicsApi::Vulkan => "Vulkan",
            GraphicsApi::Unknown => "DirectX (Auto)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Steam,
    Epic,
    GOG,
    Xbox,
    NonSteam,
    Custom,
}

impl Store {
    pub fn label(&self) -> &'static str {
        match self {
            Store::Steam => "Steam",
            Store::Epic => "Epic",
            Store::GOG => "GOG",
            Store::Xbox => "Xbox",
            Store::NonSteam => "Non-Steam",
            Store::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameInfo {
    pub title: String,
    pub path: PathBuf,
    pub exe_path: Option<PathBuf>,
    pub store: Store,
    pub app_id: Option<String>,
    pub api: GraphicsApi,
    pub is_64bit: bool,
    pub has_native_dlss: bool,
    pub is_re_engine: bool,
    pub anticheat: Option<&'static str>,
    pub fidelityfx_tier: Option<FidelityFxTier>,
    pub compat: crate::compatibility::CompatibilityProfile,
}

#[derive(Debug, Clone, Default)]
pub struct ModStatus {
    pub is_optiscaler_installed: bool,
    pub is_reshade_installed: bool,
    pub installed_renodx_mod: Option<String>,
    pub manifest_files: Vec<String>,
}

impl GameInfo {
    pub fn check_mod_status(&self) -> ModStatus {
        let Some(ref exe) = self.exe_path else {
            return ModStatus::default();
        };
        let dir = exe.parent().unwrap_or(&self.path);
        let mut status = ModStatus::default();

        // 1. Check manifest first (fastest and most accurate for FSRoneclick installations)
        let manifest_path = dir.join(MANIFEST_NAME);
        let legacy_manifest_path = dir.join(LEGACY_MANIFEST_NAME);
        let manifest_to_read = if manifest_path.is_file() {
            Some(manifest_path)
        } else if legacy_manifest_path.is_file() {
            Some(legacy_manifest_path)
        } else {
            None
        };

        if let Some(mp) = manifest_to_read {
            if let Ok(content) = fs::read_to_string(&mp) {
                status.manifest_files = content
                    .lines()
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .map(|l| l.to_string())
                    .collect();
                if !status.manifest_files.is_empty() {
                    status.is_optiscaler_installed = true;
                }
                for file in &status.manifest_files {
                    if file.contains("ReShade") {
                        status.is_reshade_installed = true;
                    }
                    if file.starts_with("renodx-") && file.ends_with(".addon64") {
                        status.installed_renodx_mod = Some(file.clone());
                    }
                }
            }
        }

        // 2. Check for OptiScaler configuration / proxy files
        if !status.is_optiscaler_installed {
            if dir.join("OptiScaler.ini").is_file() || dir.join("OptiScaler.log").is_file() {
                status.is_optiscaler_installed = true;
            } else {
                use std::io::Read;
                for hook in [OPTI_PROXY, "winmm.dll", "d3d12.dll", "version.dll"] {
                    let hook_path = dir.join(hook);
                    if hook_path.is_file() {
                        if let Ok(mut f) = fs::File::open(&hook_path) {
                            let mut buf = [0u8; 8192];
                            if let Ok(n) = f.read(&mut buf) {
                                if buf[..n].windows(10).any(|w| w == b"OptiScaler") {
                                    status.is_optiscaler_installed = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Check for ReShade
        if !status.is_reshade_installed && (dir.join(RESHADE_DLL).is_file() || dir.join("ReShade.ini").is_file()) {
            status.is_reshade_installed = true;
        }

        // 4. Check for RenoDX mod if not already known from manifest
        if status.installed_renodx_mod.is_none() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("renodx-") && name.ends_with(".addon64") {
                        status.installed_renodx_mod = Some(name);
                        break;
                    }
                }
            }
        }

        status
    }
}

/// Discovers games installed across Steam, Epic Games, and standalone Non-Steam directories (e.g. D:\Games).
pub fn scan_all_games() -> Vec<GameInfo> {
    let mut games = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // 1. Scan Standalone / Non-Steam Games (e.g. D:\Games, C:\Games)
    for g in scan_standalone_games() {
        if seen_paths.insert(g.path.clone()) {
            games.push(g);
        }
    }

    // 2. Scan Steam
    for g in scan_steam_games() {
        if seen_paths.insert(g.path.clone()) {
            games.push(g);
        }
    }

    // 3. Scan Epic
    for g in scan_epic_games() {
        if seen_paths.insert(g.path.clone()) {
            games.push(g);
        }
    }

    games
}

/// Scans standalone game directories such as D:\Games, C:\Games, E:\Games.
pub fn scan_standalone_games() -> Vec<GameInfo> {
    let mut games = Vec::new();
    let ffx_catalog = FidelityFxCatalog::new();
    let root_dirs = [
        PathBuf::from(r"D:\Games"),
        PathBuf::from(r"C:\Games"),
        PathBuf::from(r"E:\Games"),
        PathBuf::from(r"C:\GOG Games"),
        PathBuf::from(r"D:\GOG Games"),
    ];

    for root in root_dirs {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let dir_name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if dir_name.starts_with('$')
                || dir_name.starts_with('.')
                || dir_name.eq_ignore_ascii_case("recycle.bin")
                || dir_name.eq_ignore_ascii_case("system volume information")
            {
                continue;
            }

            if let Some(exe) = find_main_exe(&p) {
                let (api, is_64, dlss) = inspect_binary(&exe);
                let is_re = p.join("re_chunk_000.pak").is_file()
                    || exe.parent().map_or(false, |par| par.join("re_chunk_000.pak").is_file());
                let anticheat = detect_anticheat(&p);
                let ffx_tier = ffx_catalog.check_game(&dir_name, exe.parent().unwrap_or(&p));
                let compat = crate::compatibility::lookup_compatibility(&dir_name);

                games.push(GameInfo {
                    title: dir_name,
                    path: p,
                    exe_path: Some(exe),
                    store: Store::NonSteam,
                    app_id: None,
                    api,
                    is_64bit: is_64,
                    has_native_dlss: dlss,
                    is_re_engine: is_re,
                    anticheat,
                    fidelityfx_tier: ffx_tier,
                    compat,
                });
            }
        }
    }

    games
}

/// Scans Steam library folders.
pub fn scan_steam_games() -> Vec<GameInfo> {
    let mut games = Vec::new();
    let ffx_catalog = FidelityFxCatalog::new();
    let common_steam_dirs = [
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Steam"),
        PathBuf::from(r"D:\Steam"),
        PathBuf::from(r"E:\Steam"),
    ];

    let mut library_paths = Vec::new();
    for steam_dir in &common_steam_dirs {
        let vdf = steam_dir.join("steamapps").join("libraryfolders.vdf");
        if vdf.is_file() {
            library_paths.push(steam_dir.join("steamapps"));
            if let Ok(content) = fs::read_to_string(&vdf) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("\"path\"") {
                        if let Some(start) = trimmed.find("\"path\"") {
                            let rest = &trimmed[start + 6..].trim();
                            let clean = rest.trim_matches(|c| c == '"' || c == '\t' || c == ' ');
                            let p = PathBuf::from(clean.replace("\\\\", "\\")).join("steamapps");
                            if p.is_dir() && !library_paths.contains(&p) {
                                library_paths.push(p);
                            }
                        }
                    }
                }
            }
            break;
        }
    }

    for lib in library_paths {
        let common = lib.join("common");
        if !common.is_dir() {
            continue;
        }

        let Ok(manifests) = fs::read_dir(&lib) else { continue };
        for entry in manifests.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                let Ok(acf) = fs::read_to_string(entry.path()) else { continue };
                let app_id = extract_vdf_key(&acf, "appid");
                let name = extract_vdf_key(&acf, "name");
                let installdir = extract_vdf_key(&acf, "installdir");

                if let (Some(name), Some(installdir)) = (name, installdir) {
                    let game_dir = common.join(&installdir);
                    if game_dir.is_dir() {
                        let exe = find_main_exe(&game_dir);
                        let (api, is_64, dlss) = if let Some(ref e) = exe {
                            inspect_binary(e)
                        } else {
                            (GraphicsApi::Unknown, true, false)
                        };

                        let is_re = game_dir.join("re_chunk_000.pak").is_file()
                            || exe.as_ref().map_or(false, |e| {
                                e.parent().map_or(false, |p| p.join("re_chunk_000.pak").is_file())
                            });

                        let anticheat = detect_anticheat(&game_dir);
                        let ffx_tier = ffx_catalog.check_game(
                            &name,
                            exe.as_ref().and_then(|e| e.parent()).unwrap_or(&game_dir),
                        );
                        let compat = crate::compatibility::lookup_compatibility(&name);

                        games.push(GameInfo {
                            title: name,
                            path: game_dir,
                            exe_path: exe,
                            store: Store::Steam,
                            app_id,
                            api,
                            is_64bit: is_64,
                            has_native_dlss: dlss,
                            is_re_engine: is_re,
                            anticheat,
                            fidelityfx_tier: ffx_tier,
                            compat,
                        });
                    }
                }
            }
        }
    }

    games
}

/// Scans Epic Games Launcher manifests.
pub fn scan_epic_games() -> Vec<GameInfo> {
    let mut games = Vec::new();
    let ffx_catalog = FidelityFxCatalog::new();
    let manifests_dir = PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(entries) = fs::read_dir(manifests_dir) else { return games };

    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().map_or(false, |ext| ext == "item") {
            if let Ok(content) = fs::read_to_string(&p) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let title = json["DisplayName"].as_str().map(|s| s.to_string());
                    let install_loc = json["InstallLocation"].as_str().map(PathBuf::from);

                    if let (Some(title), Some(install_loc)) = (title, install_loc) {
                        if install_loc.is_dir() {
                            let exe = find_main_exe(&install_loc);
                            let (api, is_64, dlss) = if let Some(ref e) = exe {
                                inspect_binary(e)
                            } else {
                                (GraphicsApi::Unknown, true, false)
                            };

                            let is_re = install_loc.join("re_chunk_000.pak").is_file();
                            let anticheat = detect_anticheat(&install_loc);
                            let ffx_tier = ffx_catalog.check_game(
                                &title,
                                exe.as_ref().and_then(|e| e.parent()).unwrap_or(&install_loc),
                            );
                            let compat = crate::compatibility::lookup_compatibility(&title);

                            games.push(GameInfo {
                                title,
                                path: install_loc,
                                exe_path: exe,
                                store: Store::Epic,
                                app_id: json["AppName"].as_str().map(|s| s.to_string()),
                                api,
                                is_64bit: is_64,
                                has_native_dlss: dlss,
                                is_re_engine: is_re,
                                anticheat,
                                fidelityfx_tier: ffx_tier,
                                compat,
                            });
                        }
                    }
                }
            }
        }
    }

    games
}

/// Helper to check if a subfolder is a binary container rather than the actual game root.
fn is_subfolder_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "bin" | "binaries" | "win64" | "x64" | "shipping" | "release" | "x86"
    )
}

/// Creates a `GameInfo` for an arbitrary user-selected folder or exe, resolving subdirectories like `Far Cry 5\bin`.
pub fn from_custom_path(path: &Path) -> Option<GameInfo> {
    let (game_dir, exe_path, title) = if path.is_file() && path.extension().map_or(false, |e| e == "exe") {
        let parent = path.parent().unwrap_or(path);
        let parent_name = parent.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let title = if is_subfolder_name(&parent_name) {
            parent
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(parent_name)
        } else {
            parent_name
        };
        (parent.to_path_buf(), Some(path.to_path_buf()), title)
    } else if path.is_dir() {
        let dir_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (root_dir, title) = if is_subfolder_name(&dir_name) {
            let parent = path.parent().unwrap_or(path);
            let parent_name = parent
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir_name.clone());
            (path.to_path_buf(), parent_name)
        } else {
            (path.to_path_buf(), dir_name)
        };
        let exe = find_main_exe(path);
        (root_dir, exe, title)
    } else {
        return None;
    };

    let (api, is_64, dlss) = if let Some(ref e) = exe_path {
        inspect_binary(e)
    } else {
        (GraphicsApi::Unknown, true, false)
    };

    let is_re = game_dir.join("re_chunk_000.pak").is_file()
        || exe_path.as_ref().map_or(false, |e| {
            e.parent().map_or(false, |p| p.join("re_chunk_000.pak").is_file())
        });

    let anticheat = detect_anticheat(&game_dir);
    let ffx_catalog = FidelityFxCatalog::new();
    let ffx_tier = ffx_catalog.check_game(
        &title,
        exe_path.as_ref().and_then(|e| e.parent()).unwrap_or(&game_dir),
    );
    let compat = crate::compatibility::lookup_compatibility(&title);

    Some(GameInfo {
        title,
        path: game_dir,
        exe_path,
        store: Store::Custom,
        app_id: None,
        api,
        is_64bit: is_64,
        has_native_dlss: dlss,
        is_re_engine: is_re,
        anticheat,
        fidelityfx_tier: ffx_tier,
        compat,
    })
}

fn extract_vdf_key(vdf: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    for line in vdf.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&pattern) {
            let parts: Vec<&str> = trimmed.split('"').filter(|p| !p.trim().is_empty()).collect();
            if parts.len() >= 2 {
                return Some(parts[1].to_string());
            }
        }
    }
    None
}

/// Finds the most likely game executable inside a game directory.
pub fn find_main_exe(dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    find_exes_recursive(dir, 0, 4, &mut candidates);

    if candidates.is_empty() {
        return None;
    }

    // Filter out obvious crash helpers, installers, launcher wrappers, redistributables
    let filtered: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            let parent_name = p
                .parent()
                .and_then(|par| par.file_name())
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if parent_name == "_redist"
                || parent_name == "redist"
                || parent_name == "support"
                || parent_name == "extras"
            {
                return false;
            }

            !name.contains("crash")
                && !name.contains("report")
                && !name.contains("unins")
                && !name.contains("setup")
                && !name.contains("config")
                && !name.contains("launcher")
                && !name.contains("redist")
                && !name.contains("eac")
                && !name.contains("easyanticheat")
                && !name.contains("unitycrash")
                && !name.contains("epiconlineservices")
                && !name.contains("quicksfv")
                && !name.contains("dotnet")
                && !name.contains("ndp")
                && !name.contains("arcadeeditor")
                && !name.contains("epicwebhelper")
        })
        .collect();

    // 1. Prefer Shipping or Win64 exes (Unreal Engine convention)
    if let Some(shipping) = filtered.iter().find(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        n.contains("-shipping") || n.contains("win64")
    }) {
        return Some(shipping.clone());
    }

    // 2. Check if an exe name closely matches the root game folder name (e.g. FarCry5.exe matches Far Cry 5)
    let folder_norm = crate::renodx::normalize_title(&dir.file_name().unwrap_or_default().to_string_lossy());
    if let Some(matching) = filtered.iter().find(|p| {
        let exe_stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let stem_norm = crate::renodx::normalize_title(&exe_stem);
        !stem_norm.is_empty()
            && (stem_norm == folder_norm
                || folder_norm.contains(&stem_norm)
                || stem_norm.contains(&folder_norm))
    }) {
        return Some(matching.clone());
    }

    // 3. Prefer an exe inside a "bin" folder
    if let Some(bin_exe) = filtered.iter().find(|p| {
        p.parent()
            .and_then(|par| par.file_name())
            .map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("bin"))
    }) {
        return Some(bin_exe.clone());
    }

    // 4. Otherwise prefer largest exe
    filtered
        .into_iter()
        .max_by_key(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
}

fn find_exes_recursive(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension() {
                if ext.eq_ignore_ascii_case("exe") {
                    out.push(p);
                }
            }
        } else if p.is_dir() {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            if name != "directx"
                && name != "redist"
                && name != "_redist"
                && name != "_commonredist"
                && name != ".git"
                && name != "content"
                && name != "paks"
                && name != "engine"
                && name != "plugins"
                && name != "saved"
                && name != "saves"
                && name != "support"
                && name != "output"
                && name != "data_final"
            {
                find_exes_recursive(&p, depth + 1, max_depth, out);
            }
        }
    }
}

/// Parses the PE binary to determine: GraphicsApi, 64-bit status, and native DLSS presence.
pub fn inspect_binary(exe_path: &Path) -> (GraphicsApi, bool, bool) {
    use std::io::Read;

    let dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
    let mut has_dlss = dir.join("nvngx_dlss.dll").is_file();

    // Check parent directories for nvngx_dlss (up to 3 levels)
    let mut check_p = dir.to_path_buf();
    for _ in 0..3 {
        if check_p.join("nvngx_dlss.dll").is_file() {
            has_dlss = true;
            break;
        }
        if let Some(parent) = check_p.parent() {
            check_p = parent.to_path_buf();
        } else {
            break;
        }
    }

    // DirectAgility redist check
    let has_d3d12_agility = dir.join("D3D12").join("D3D12Core.dll").is_file();

    let Ok(mut file) = fs::File::open(exe_path) else {
        return (GraphicsApi::Unknown, true, has_dlss);
    };

    // Read only the first 1MB of the executable (headers + import table)
    let mut buf = vec![0u8; 1024 * 1024];
    let bytes_read = file.read(&mut buf).unwrap_or(0);
    let data = &buf[..bytes_read];

    // Basic PE parsing
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return (GraphicsApi::Unknown, true, has_dlss);
    }

    let pe_offset = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
    if data.len() < pe_offset + 24 || &data[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return (GraphicsApi::Unknown, true, has_dlss);
    }

    let machine = u16::from_le_bytes([data[pe_offset + 4], data[pe_offset + 5]]);
    let is_64bit = machine == 0x8664 || machine == 0xAA64; // AMD64 or ARM64

    let contains_ci = |needle: &[u8]| -> bool {
        data.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
    };

    let mut imported_d3d12 = contains_ci(b"d3d12.dll") || has_d3d12_agility;
    let mut imported_d3d11 = contains_ci(b"d3d11.dll");
    let imported_vulkan = contains_ci(b"vulkan-1.dll");
    let imported_d3d9 = contains_ci(b"d3d9.dll");

    if contains_ci(b"nvngx_dlss.dll") || contains_ci(b"sl.interactor.dll") {
        has_dlss = true;
    }

    // If main exe is just a launcher stub without direct D3D imports, inspect companion engine DLLs
    if !imported_d3d12 && !imported_d3d11 && !imported_vulkan {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    if name.ends_with("_m64.dll")
                        || name == "unityplayer.dll"
                        || name == "d3dcompiler_47.dll"
                        || name == "amd_ags_x64.dll"
                    {
                        if let Ok(mut comp_file) = fs::File::open(&p) {
                            let mut comp_buf = vec![0u8; 1024 * 1024];
                            let comp_read = comp_file.read(&mut comp_buf).unwrap_or(0);
                            let comp_data = &comp_buf[..comp_read];
                            let comp_ci = |needle: &[u8]| {
                                comp_data.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
                            };
                            if comp_ci(b"d3d12.dll") {
                                imported_d3d12 = true;
                            } else if comp_ci(b"d3d11.dll") || name == "d3dcompiler_47.dll" {
                                imported_d3d11 = true;
                            }
                        }
                    }
                }
            }
        }
    }

    let api = if imported_d3d12 {
        GraphicsApi::DirectX12
    } else if imported_d3d11 {
        GraphicsApi::DirectX11
    } else if imported_vulkan {
        GraphicsApi::Vulkan
    } else if imported_d3d9 {
        GraphicsApi::DirectX9
    } else {
        GraphicsApi::Unknown
    };

    (api, is_64bit, has_dlss)
}

/// Detects if an anti-cheat engine is present.
pub fn detect_anticheat(dir: &Path) -> Option<&'static str> {
    let mut check_dirs = vec![dir.to_path_buf()];
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                check_dirs.push(e.path());
            }
        }
    }

    for d in check_dirs {
        if d.join("EasyAntiCheat").is_dir()
            || d.join("EasyAntiCheat_EOS.sys").is_file()
            || d.join("start_protected_game.exe").is_file()
        {
            return Some("Easy Anti-Cheat");
        }
        if d.join("BattlEye").is_dir() || d.join("BEService.exe").is_file() {
            return Some("BattlEye");
        }
        if d.join("vgk.sys").is_file() {
            return Some("Vanguard");
        }
    }

    None
}
