//! RenoDX wiki parsing: remote rows decide what gets downloaded and where it is written.

use fsroneclick::renodx::parse_mods_catalog;

const WIKI: &str = r#"
| Name | Maintainer | Links | Status |
| :--- | :--- | :--- | :---: |
| Absolum | OopyDoopy | [![Snapshot](https://img.shields.io/x)](https://oopydoopy.github.io/renodx/renodx-absolum.addon64) | ok |
| 007 First Light | Musa | [![Snapshot](https://img.shields.io/x)](https://github.com/mqhaji/renodx/releases/download/snapshot/renodx-007firstlight.addon64) | ok |
| Evil Path | x | [![Snapshot](https://img.shields.io/x)](https://evil.github.io/..\..\Windows\renodx-evil.addon64) | ok |
| Plain Http | x | [![Snapshot](https://img.shields.io/x)](http://example.github.io/renodx-plain.addon64) | ok |
| Other Host | x | [![Snapshot](https://img.shields.io/x)](https://downloads.example.com/renodx-other.addon64) | ok |
| Lookalike Host | x | [![Snapshot](https://img.shields.io/x)](https://github.com.example.com/renodx-lookalike.addon64) | ok |
| Backslash Host | x | [![Snapshot](https://img.shields.io/x)](https://evil.example\.github.io/renodx-backslash.addon64) | ok |
| Query Host | x | [![Snapshot](https://img.shields.io/x)](https://evil.example?.github.io/renodx-query.addon64) | ok |
| Fragment Host | x | [![Snapshot](https://img.shields.io/x)](https://evil.example#.github.io/renodx-fragment.addon64) | ok |
| Userinfo Host | x | [![Snapshot](https://img.shields.io/x)](https://x.github.io@evil.example/renodx-userinfo.addon64) | ok |
| Issue Attachment | x | [![Snapshot](https://img.shields.io/x)](https://github.com/user-attachments/files/123/renodx-attachment.addon64) | ok |
| Repo File | x | [![Snapshot](https://img.shields.io/x)](https://github.com/someone/renodx/raw/main/renodx-repofile.addon64) | ok |
"#;

#[test]
fn parses_valid_wiki_rows() {
    let catalog = parse_mods_catalog(WIKI);

    let absolum = catalog.get("absolum").expect("github.io row is parsed");
    assert_eq!(absolum.game_title, "Absolum");
    assert_eq!(absolum.filename, "renodx-absolum.addon64");
    assert_eq!(
        absolum.download_url,
        "https://oopydoopy.github.io/renodx/renodx-absolum.addon64"
    );
    assert!(
        catalog.contains_key("007firstlight"),
        "github.com release row is parsed"
    );
}

#[test]
fn rejects_unsafe_file_names_and_insecure_urls() {
    let catalog = parse_mods_catalog(WIKI);

    assert!(
        !catalog.contains_key("evilpath"),
        "file names with path separators must be rejected"
    );
    assert!(
        !catalog.contains_key("plainhttp"),
        "downloads must use HTTPS"
    );
}

#[test]
fn only_accepts_github_hosted_downloads() {
    let catalog = parse_mods_catalog(WIKI);

    assert!(!catalog.contains_key("otherhost"));
    assert!(!catalog.contains_key("lookalikehost"));
}

#[test]
fn rejects_urls_whose_real_host_is_not_github() {
    let catalog = parse_mods_catalog(WIKI);

    // Each of these names a GitHub host only to a naive parser; the HTTP client would
    // actually connect to evil.example.
    for key in ["backslashhost", "queryhost", "fragmenthost", "userinfohost"] {
        assert!(!catalog.contains_key(key), "{key} must be rejected");
    }
}

#[test]
fn github_com_downloads_must_be_release_assets() {
    let catalog = parse_mods_catalog(WIKI);

    // Anyone can upload issue attachments to github.com; only release assets are accepted.
    assert!(!catalog.contains_key("issueattachment"));
    assert!(!catalog.contains_key("repofile"));
    assert!(catalog.contains_key("007firstlight"));
}
