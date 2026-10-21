//! Executable discovery: which file in a game folder is treated as the game.

use fsroneclick::game::{find_main_exe, inspect_binary, GraphicsApi};
use std::fs;
use std::path::{Path, PathBuf};

/// Creates `<temp>/fsroneclick_scan_<case>/<game_folder>` containing `files` (each `(path, bytes)`).
fn game_folder(case: &str, game_folder: &str, files: &[(&str, usize)]) -> PathBuf {
    let base = std::env::temp_dir().join(format!("fsroneclick_scan_{case}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let root = base.join(game_folder);
    fs::create_dir_all(&root).unwrap();
    for (file, size) in files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![b'x'; *size]).unwrap();
    }
    root
}

fn main_exe(root: &Path) -> Option<String> {
    let found = find_main_exe(root).map(|p| {
        p.strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    });
    let _ = fs::remove_dir_all(root.parent().unwrap());
    found
}

#[test]
fn prefers_the_unreal_shipping_binary() {
    let root = game_folder(
        "shipping",
        "My Game",
        &[
            ("Launcher.exe", 10),
            ("MyGame/Binaries/Win64/MyGame-Win64-Shipping.exe", 10),
            ("Engine/Binaries/Win64/CrashReportClient.exe", 10),
        ],
    );
    assert_eq!(
        main_exe(&root).as_deref(),
        Some("MyGame/Binaries/Win64/MyGame-Win64-Shipping.exe")
    );
}

#[test]
fn matches_the_exe_named_after_the_game_folder() {
    let root = game_folder(
        "name_match",
        "Far Cry 5",
        &[("bin/FarCry5.exe", 10), ("bin/upc.exe", 50)],
    );
    assert_eq!(main_exe(&root).as_deref(), Some("bin/FarCry5.exe"));
}

#[test]
fn skips_crash_handlers_and_redistributables() {
    let root = game_folder(
        "helpers",
        "Some Unity Game",
        &[
            ("UnityCrashHandler64.exe", 100),
            ("_CommonRedist/vcredist_x64.exe", 100),
            ("Game.exe", 10),
        ],
    );
    assert_eq!(main_exe(&root).as_deref(), Some("Game.exe"));
}

#[test]
fn finds_nothing_when_only_redistributables_exist() {
    let root = game_folder(
        "only_redist",
        "Installer Leftovers",
        &[
            ("_CommonRedist/DirectX/DXSETUP.exe", 10),
            ("Redist/vc_redist.x64.exe", 10),
        ],
    );
    assert_eq!(main_exe(&root), None);
}

#[test]
fn prefers_a_bin_folder_then_the_largest_exe() {
    let in_bin = game_folder(
        "bin_pref",
        "Some Title",
        &[("Tools/Editor.exe", 500), ("bin/Game.exe", 10)],
    );
    assert_eq!(main_exe(&in_bin).as_deref(), Some("bin/Game.exe"));

    let largest = game_folder(
        "largest",
        "Another Title",
        &[("small.exe", 10), ("large.exe", 500)],
    );
    assert_eq!(main_exe(&largest).as_deref(), Some("large.exe"));
}

/// A minimal PE image for `machine` whose import strings are `imports`.
fn fake_exe(case: &str, machine: u16, imports: &[u8]) -> PathBuf {
    let mut image = vec![0u8; 0x200];
    image[..2].copy_from_slice(b"MZ");
    image[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    image[0x40..0x44].copy_from_slice(b"PE\0\0");
    image[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
    image.extend_from_slice(imports);

    let dir = std::env::temp_dir().join(format!("fsroneclick_pe_{case}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("game.exe");
    fs::write(&exe, image).unwrap();
    exe
}

#[test]
fn detects_graphics_api_bitness_and_dlss_from_imports() {
    let dx12 = fake_exe("dx12", 0x8664, b"KERNEL32.dll\0D3D12.DLL\0nvngx_dlss.dll\0");
    assert_eq!(inspect_binary(&dx12), (GraphicsApi::DirectX12, true, true));

    let vulkan = fake_exe("vulkan", 0x8664, b"Vulkan-1.dll\0sl.interactor.dll\0");
    assert_eq!(inspect_binary(&vulkan), (GraphicsApi::Vulkan, true, true));

    let dx9 = fake_exe("dx9", 0x014C, b"d3d9.dll\0");
    assert_eq!(inspect_binary(&dx9), (GraphicsApi::DirectX9, false, false));

    for exe in [dx12, vulkan, dx9] {
        let _ = fs::remove_dir_all(exe.parent().unwrap());
    }
}

#[test]
fn searches_four_folders_deep() {
    let deep = game_folder("deep", "Deep Game", &[("a/b/c/d/Deep.exe", 10)]);
    assert_eq!(main_exe(&deep).as_deref(), Some("a/b/c/d/Deep.exe"));

    let too_deep = game_folder("too_deep", "Deeper Game", &[("a/b/c/d/e/TooDeep.exe", 10)]);
    assert_eq!(main_exe(&too_deep), None);
}
