//! Download integrity, GitHub release metadata, and update-check helpers.

use fsroneclick::net::{self, parse_release_asset};
use fsroneclick::updater::{format_utc, is_newer_version, parse_reshade_latest_version};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

const HELLO_SHA256: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// Serves `body` to the next `connections` requests on a local port.
fn serve(body: &'static [u8], connections: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/file.bin", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming().take(connections) {
            let mut stream = stream.unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
        }
    });
    url
}

/// A URL nothing listens on, so any request to it fails.
fn dead_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}/file.bin", listener.local_addr().unwrap())
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fsroneclick_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
}

#[test]
fn sha256_file_matches_known_vector() {
    let dir = temp_dir("sha_vector");
    fs::write(dir.join("abc.txt"), b"abc").unwrap();

    assert_eq!(net::sha256_file(&dir.join("abc.txt")).unwrap(), ABC_SHA256);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn verified_download_accepts_matching_content() {
    let dir = temp_dir("verified_ok");
    let dest = dir.join("file.bin");

    net::download_verified(
        &client(),
        &serve(b"hello world", 1),
        &dest,
        HELLO_SHA256,
        Box::new(|_, _| {}),
    )
    .unwrap();

    assert_eq!(fs::read(&dest).unwrap(), b"hello world");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn verified_download_rejects_tampered_content() {
    let dir = temp_dir("verified_bad");
    let dest = dir.join("file.bin");

    let result = net::download_verified(
        &client(),
        &serve(b"hello world", 1),
        &dest,
        ABC_SHA256,
        Box::new(|_, _| {}),
    );

    assert!(result.is_err());
    assert!(
        !dest.exists(),
        "a file that fails verification must not be kept"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn verified_download_replaces_corrupt_cached_copy() {
    let dir = temp_dir("verified_corrupt");
    let dest = dir.join("file.bin");
    fs::write(&dest, b"corrupt").unwrap();

    net::download_verified(
        &client(),
        &serve(b"hello world", 1),
        &dest,
        HELLO_SHA256,
        Box::new(|_, _| {}),
    )
    .unwrap();

    assert_eq!(fs::read(&dest).unwrap(), b"hello world");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn verified_download_reuses_valid_cached_copy_offline() {
    let dir = temp_dir("verified_cached");
    let dest = dir.join("file.bin");
    fs::write(&dest, b"hello world").unwrap();

    net::download_verified(
        &client(),
        &dead_url(),
        &dest,
        HELLO_SHA256,
        Box::new(|_, _| {}),
    )
    .unwrap();

    assert_eq!(fs::read(&dest).unwrap(), b"hello world");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn refreshing_download_updates_cached_copy() {
    let dir = temp_dir("refresh_new");
    let dest = dir.join("mod.addon64");
    fs::write(&dest, b"old build").unwrap();

    net::download_refreshing(
        &client(),
        &serve(b"hello world", 1),
        &dest,
        Box::new(|_, _| {}),
    )
    .unwrap();

    assert_eq!(fs::read(&dest).unwrap(), b"hello world");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn refreshing_download_falls_back_to_cached_copy_offline() {
    let dir = temp_dir("refresh_offline");
    let dest = dir.join("mod.addon64");
    fs::write(&dest, b"old build").unwrap();

    net::download_refreshing(&client(), &dead_url(), &dest, Box::new(|_, _| {})).unwrap();

    assert_eq!(fs::read(&dest).unwrap(), b"old build");
    assert!(net::download_refreshing(
        &client(),
        &dead_url(),
        &dir.join("missing.addon64"),
        Box::new(|_, _| {})
    )
    .is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn release_asset_requires_a_published_sha256() {
    let release: serde_json::Value = serde_json::json!({
        "tag_name": "rolling",
        "assets": [
            {
                "name": "OptiPatcher.asi",
                "browser_download_url": "https://github.com/optiscaler/OptiPatcher/releases/download/rolling/OptiPatcher.asi",
                "digest": format!("sha256:{HELLO_SHA256}")
            },
            { "name": "NoDigest.zip", "browser_download_url": "https://github.com/x/y/releases/download/t/NoDigest.zip" },
            { "name": "NullDigest.zip", "browser_download_url": "https://github.com/x/y/releases/download/t/NullDigest.zip", "digest": null },
            { "name": "Md5.zip", "browser_download_url": "https://github.com/x/y/releases/download/t/Md5.zip", "digest": "md5:abc" },
            { "name": "Short.zip", "browser_download_url": "https://github.com/x/y/releases/download/t/Short.zip", "digest": format!("sha256:{}", &HELLO_SHA256[1..]) },
            { "name": "NotHex.zip", "browser_download_url": "https://github.com/x/y/releases/download/t/NotHex.zip", "digest": format!("sha256:{}g", &HELLO_SHA256[1..]) },
            { "name": "Upper.zip", "browser_download_url": "https://github.com/x/y/releases/download/t/Upper.zip", "digest": format!("sha256:{}", HELLO_SHA256.to_uppercase()) }
        ]
    });

    let asset = parse_release_asset(&release, "OptiPatcher.asi").unwrap();
    assert_eq!(asset.tag, "rolling");
    assert_eq!(asset.sha256, HELLO_SHA256);
    assert!(asset.download_url.ends_with("/OptiPatcher.asi"));
    assert_eq!(
        parse_release_asset(&release, "Upper.zip").unwrap().sha256,
        HELLO_SHA256
    );

    for name in [
        "NoDigest.zip",
        "NullDigest.zip",
        "Md5.zip",
        "Short.zip",
        "NotHex.zip",
        "Missing.zip",
    ] {
        assert!(parse_release_asset(&release, name).is_err(), "{name}");
    }
}

#[test]
fn extraction_is_published_only_when_complete() {
    let dir = temp_dir("extract_once");
    let archive = dir.join("package.zip");
    fs::write(&archive, b"archive").unwrap();

    // A failed extraction leaves nothing that a later install could mistake for the package.
    let failed = net::extract_once(&dir, &archive, HELLO_SHA256, "pkg", |_, out| {
        fs::create_dir_all(out)?;
        fs::write(out.join("OptiScaler.dll"), b"truncated")?;
        anyhow::bail!("disk full")
    });
    assert!(failed.is_err());
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        1,
        "only the archive may remain"
    );

    let extracted = net::extract_once(&dir, &archive, HELLO_SHA256, "pkg", |_, out| {
        fs::create_dir_all(out)?;
        fs::write(out.join("OptiScaler.dll"), b"complete")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        fs::read(extracted.join("OptiScaler.dll")).unwrap(),
        b"complete"
    );

    // The same archive is not extracted twice.
    let reused = net::extract_once(&dir, &archive, HELLO_SHA256, "pkg", |_, _| {
        panic!("must reuse")
    })
    .unwrap();
    assert_eq!(reused, extracted);

    // An archive with a different checksum gets its own folder; the old extraction is removed.
    let newer = net::extract_once(&dir, &archive, ABC_SHA256, "pkg", |_, out| {
        fs::create_dir_all(out)?;
        Ok(())
    })
    .unwrap();
    assert_ne!(newer, extracted);
    assert!(!extracted.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cached_github_asset_is_reused_only_while_intact() {
    let dir = temp_dir("cached_asset");
    let cached = dir.join(format!("{HELLO_SHA256}-OptiPatcher.asi"));
    fs::write(&cached, b"hello world").unwrap();

    let (path, sha256) = net::find_cached_github_asset(&dir, "OptiPatcher.asi").unwrap();
    assert_eq!(path, cached);
    assert_eq!(sha256, HELLO_SHA256);

    fs::write(&cached, b"tampered").unwrap();
    assert!(net::find_cached_github_asset(&dir, "OptiPatcher.asi").is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn superseded_github_assets_are_removed() {
    let dir = temp_dir("superseded");
    let older = dir.join(format!("{HELLO_SHA256}-REFramework.zip"));
    let current = format!("{ABC_SHA256}-REFramework.zip");
    let other_asset = dir.join(format!("{HELLO_SHA256}-OptiPatcher.asi"));
    let unrelated = dir.join("REFramework.zip");
    for path in [&older, &dir.join(&current), &other_asset, &unrelated] {
        fs::write(path, b"x").unwrap();
    }

    net::remove_superseded_copies(&dir, &current, "REFramework.zip");

    assert!(!older.exists());
    assert!(dir.join(&current).exists());
    assert!(other_asset.exists() && unrelated.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pinned_downloads_are_consistent() {
    use fsroneclick::installer::{
        OPTISCALER_DIRECT_URL, OPTISCALER_SHA256, OPTISCALER_VERSION, RESHADE_ADDON_SETUP_SHA256,
        RESHADE_ADDON_SETUP_URL, RESHADE_VERSION,
    };

    assert!(
        RESHADE_ADDON_SETUP_URL.ends_with(&format!("ReShade_Setup_{RESHADE_VERSION}_Addon.exe"))
    );
    assert!(OPTISCALER_DIRECT_URL.contains(&format!("/v{OPTISCALER_VERSION}/")));
    assert!(OPTISCALER_DIRECT_URL.contains(&format!("Optiscaler_{OPTISCALER_VERSION}")));
    for sha256 in [OPTISCALER_SHA256, RESHADE_ADDON_SETUP_SHA256] {
        assert!(sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn version_comparison_is_numeric() {
    assert!(is_newer_version("v0.9.5", "v0.9.4"));
    assert!(is_newer_version("0.9.10", "0.9.9"));
    assert!(is_newer_version("v1.0.0", "0.9.10"));
    assert!(is_newer_version("6.8.0", "v6.4.1 (Addon)"));
    assert!(!is_newer_version("v0.9.4", "0.9.4"));
    assert!(!is_newer_version("v0.9.3", "v0.9.4"));
    assert!(!is_newer_version("0.9.4-final", "0.9.4"));
    assert!(!is_newer_version("nightly", "0.9.4"));
}

#[test]
fn reshade_latest_version_is_read_from_download_links() {
    let html = r#"<a href="/downloads/ReShade_Setup_6.8.0.exe">ReShade 6.8.0</a>
        <a href="/downloads/ReShade_Setup_6.8.0_Addon.exe">with full add-on support</a>
        <a href="/downloads/ReShade_Setup_6.7.3_Addon.exe">older</a>"#;

    assert_eq!(parse_reshade_latest_version(html).as_deref(), Some("6.8.0"));
    assert_eq!(
        parse_reshade_latest_version("<html>maintenance</html>"),
        None
    );
}

#[test]
fn utc_timestamps_are_formatted_as_calendar_dates() {
    assert_eq!(format_utc(0), "1970-01-01 00:00 UTC");
    assert_eq!(format_utc(951_782_400), "2000-02-29 00:00 UTC");
    assert_eq!(format_utc(1_700_000_000), "2023-11-14 22:13 UTC");
}
