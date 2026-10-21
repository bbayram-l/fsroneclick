//! Rollback safety: files placed by an install must be undone exactly, and game files they replaced restored.

use fsroneclick::game::{GameInfo, GraphicsApi, Store, MANIFEST_NAME};
use fsroneclick::installer::{uninstall, Deployment, BACKUP_SUFFIX};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_game_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fsroneclick_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("game.exe"), b"exe").unwrap();
    dir
}

fn game_in(dir: &Path) -> GameInfo {
    GameInfo {
        title: "Test Game".to_string(),
        path: dir.to_path_buf(),
        exe_path: Some(dir.join("game.exe")),
        store: Store::Custom,
        app_id: None,
        api: GraphicsApi::DirectX12,
        is_64bit: true,
        has_native_dlss: false,
        is_re_engine: false,
        anticheat: None,
        fidelityfx_tier: None,
        compat: Default::default(),
    }
}

fn backup_of(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{BACKUP_SUFFIX}"))
}

#[test]
fn uninstall_restores_game_file_overwritten_by_install() {
    let dir = temp_game_dir("restore");
    fs::write(dir.join("amd_fidelityfx_dx12.dll"), b"original").unwrap();

    let mut deployment = Deployment::begin(&dir).unwrap();
    deployment
        .write_file("amd_fidelityfx_dx12.dll", b"optiscaler")
        .unwrap();
    deployment.finish();
    assert_eq!(
        fs::read(dir.join("amd_fidelityfx_dx12.dll")).unwrap(),
        b"optiscaler"
    );

    uninstall(&game_in(&dir)).unwrap();

    assert_eq!(
        fs::read(dir.join("amd_fidelityfx_dx12.dll")).unwrap(),
        b"original"
    );
    assert!(!backup_of(&dir, "amd_fidelityfx_dx12.dll").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reinstall_keeps_earlier_files_and_original_backup() {
    let dir = temp_game_dir("reinstall");
    fs::write(dir.join("dxgi.dll"), b"original").unwrap();

    let mut first = Deployment::begin(&dir).unwrap();
    first.write_file("dxgi.dll", b"opti v1").unwrap();
    first.write_file("ReShade.ini", b"[GENERAL]").unwrap();
    first.finish();

    // "Update Configuration" with ReShade disabled only rewrites the proxy DLL.
    let mut second = Deployment::begin(&dir).unwrap();
    second.write_file("dxgi.dll", b"opti v2").unwrap();
    second.finish();

    uninstall(&game_in(&dir)).unwrap();

    assert_eq!(fs::read(dir.join("dxgi.dll")).unwrap(), b"original");
    assert!(
        !dir.join("ReShade.ini").exists(),
        "files from the first install must not be orphaned"
    );
    assert!(!dir.join(MANIFEST_NAME).exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reinstall_does_not_back_up_our_own_files() {
    let dir = temp_game_dir("own_files");

    let mut first = Deployment::begin(&dir).unwrap();
    first.write_file("dxgi.dll", b"opti v1").unwrap();
    first.finish();
    let mut second = Deployment::begin(&dir).unwrap();
    second.write_file("DXGI.dll", b"opti v2").unwrap();
    second.finish();

    uninstall(&game_in(&dir)).unwrap();

    assert!(
        !dir.join("dxgi.dll").exists(),
        "no stale copy of our own file may come back"
    );
    assert!(!backup_of(&dir, "dxgi.dll").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn nested_files_are_tracked_and_removed() {
    let dir = temp_game_dir("nested");

    let mut deployment = Deployment::begin(&dir).unwrap();
    deployment
        .write_file("plugins/OptiPatcher.asi", b"asi")
        .unwrap();
    let placed = deployment.finish();
    assert!(placed.contains(&"plugins/OptiPatcher.asi".to_string()));
    assert!(dir.join("plugins").join("OptiPatcher.asi").is_file());

    uninstall(&game_in(&dir)).unwrap();

    assert!(!dir.join("plugins").exists());
    assert!(dir.join("game.exe").is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn failed_copy_puts_the_original_back() {
    let dir = temp_game_dir("failed_copy");
    fs::write(dir.join("libxess.dll"), b"original").unwrap();

    let mut deployment = Deployment::begin(&dir).unwrap();
    let missing_source = dir.join("does-not-exist.dll");
    assert!(deployment
        .copy_file(&missing_source, "libxess.dll")
        .is_err());

    assert_eq!(fs::read(dir.join("libxess.dll")).unwrap(), b"original");
    assert!(!backup_of(&dir, "libxess.dll").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn interrupted_install_can_still_be_rolled_back() {
    let dir = temp_game_dir("interrupted");
    fs::write(dir.join("dxgi.dll"), b"original").unwrap();

    let mut deployment = Deployment::begin(&dir).unwrap();
    deployment.write_file("dxgi.dll", b"optiscaler").unwrap();
    // Simulate the app being closed mid-install: `finish` never runs.
    drop(deployment);

    uninstall(&game_in(&dir)).unwrap();

    assert_eq!(fs::read(dir.join("dxgi.dll")).unwrap(), b"original");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_manifest_entries_do_not_delete_the_restored_original() {
    let dir = temp_game_dir("duplicates");
    fs::write(dir.join("dxgi.dll"), b"optiscaler").unwrap();
    fs::write(backup_of(&dir, "dxgi.dll"), b"original").unwrap();
    fs::write(dir.join(MANIFEST_NAME), "dxgi.dll\nDXGI.dll\ndxgi.dll\n").unwrap();

    uninstall(&game_in(&dir)).unwrap();

    assert_eq!(fs::read(dir.join("dxgi.dll")).unwrap(), b"original");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn uninstall_of_a_locked_file_keeps_the_manifest_for_a_retry() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = temp_game_dir("locked");
    fs::write(dir.join("dxgi.dll"), b"original").unwrap();
    let mut deployment = Deployment::begin(&dir).unwrap();
    deployment.write_file("dxgi.dll", b"optiscaler").unwrap();
    deployment
        .write_file("OptiScaler.ini", b"[General]")
        .unwrap();
    deployment.finish();

    // Like a running game holding its proxy DLL open: no sharing, so it cannot be deleted.
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dir.join("dxgi.dll"))
        .unwrap();
    assert!(
        uninstall(&game_in(&dir)).is_err(),
        "a file that could not be removed must be reported"
    );
    assert!(dir.join(MANIFEST_NAME).is_file());
    assert!(!dir.join("OptiScaler.ini").exists());
    drop(lock);

    uninstall(&game_in(&dir)).unwrap();

    assert_eq!(fs::read(dir.join("dxgi.dll")).unwrap(), b"original");
    assert!(!dir.join(MANIFEST_NAME).exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unreadable_manifest_is_reported_instead_of_overwritten() {
    let dir = temp_game_dir("unreadable");
    // UTF-16 text, as Notepad's "Unicode" encoding would save it.
    fs::write(dir.join(MANIFEST_NAME), [0xFF, 0xFE, b'd', 0, b'x', 0]).unwrap();

    assert!(Deployment::begin(&dir).is_err());
    let _ = fs::remove_dir_all(&dir);
}
