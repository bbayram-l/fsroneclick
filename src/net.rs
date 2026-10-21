//! Network downloading, GitHub release queries, and local cache management.

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub type ProgressCallback<'a> = Box<dyn FnMut(f32, &str) + 'a>;

pub fn cache_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local).join("FSRoneclick").join("cache");
        let _ = fs::create_dir_all(&p);
        p
    } else {
        let p = PathBuf::from(".cache");
        let _ = fs::create_dir_all(&p);
        p
    }
}

pub fn create_http_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("FSRoneclick/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build HTTP client")
}

/// Downloads a URL to a specified file, reporting progress.
///
/// Data is streamed into `<dest>.part` and only renamed to `dest` once complete, so an
/// interrupted download is never mistaken for a valid cached file.
pub fn download_with_progress(
    client: &Client,
    url: &str,
    dest: &Path,
    mut progress: ProgressCallback,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut partial = dest.as_os_str().to_owned();
    partial.push(".part");
    let partial = PathBuf::from(partial);

    let finished = stream_to_file(client, url, &partial, &mut progress).and_then(|()| {
        fs::rename(&partial, dest)
            .with_context(|| format!("Failed to finalize download {}", dest.display()))
    });
    if let Err(e) = finished {
        let _ = fs::remove_file(&partial);
        return Err(e);
    }

    progress(1.0, "Download completed");
    Ok(())
}

fn stream_to_file(
    client: &Client,
    url: &str,
    dest: &Path,
    progress: &mut ProgressCallback,
) -> Result<()> {
    progress(0.0, &format!("Connecting to {url}"));
    let mut resp = client.get(url).send()?.error_for_status()?;

    let total_len = resp.content_length().unwrap_or(0);
    let mut file = File::create(dest)?;
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 16384];

    loop {
        let read_bytes = resp.read(&mut buf)?;
        if read_bytes == 0 {
            break;
        }
        file.write_all(&buf[..read_bytes])?;
        downloaded += read_bytes as u64;

        if total_len > 0 {
            let pct = (downloaded as f32) / (total_len as f32);
            progress(
                pct,
                &format!(
                    "Downloading ({:.1} MB / {:.1} MB)",
                    downloaded as f64 / 1_048_576.0,
                    total_len as f64 / 1_048_576.0
                ),
            );
        } else {
            progress(
                0.5,
                &format!("Downloading ({:.1} MB)", downloaded as f64 / 1_048_576.0),
            );
        }
    }
    Ok(())
}

/// SHA-256 of a file as lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};

    let mut file =
        File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads `url` to `dest` unless `dest` already holds content matching `sha256`, then checks
/// the result. A cached copy that does not match is downloaded again; a download that does not
/// match is deleted and reported as an error.
pub fn download_verified(
    client: &Client,
    url: &str,
    dest: &Path,
    sha256: &str,
    progress: ProgressCallback,
) -> Result<()> {
    if matches_sha256(dest, sha256) {
        return Ok(());
    }
    download_with_progress(client, url, dest, progress)?;
    let actual = sha256_file(dest)?;
    if !actual.eq_ignore_ascii_case(sha256) {
        let _ = fs::remove_file(dest);
        return Err(anyhow!(
            "Checksum mismatch for {url}: expected SHA-256 {sha256}, got {actual}. \
             The download may be corrupted or tampered with."
        ));
    }
    Ok(())
}

/// Whether `path` is a readable file whose SHA-256 is `sha256`.
fn matches_sha256(path: &Path, sha256: &str) -> bool {
    path.is_file() && sha256_file(path).is_ok_and(|actual| actual.eq_ignore_ascii_case(sha256))
}

/// Extracts `archive` (already verified to have checksum `sha256`) into `<base>/<prefix>-<sha256>`
/// and returns that folder. The folder only appears once extraction finished, so an interrupted
/// extraction, or one from a different archive, is never mistaken for this archive's contents.
/// Extractions of other archives with the same `prefix` are removed.
pub fn extract_once(
    base: &Path,
    archive: &Path,
    sha256: &str,
    prefix: &str,
    extract: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<PathBuf> {
    let name = format!("{prefix}-{sha256}");
    let dest = base.join(&name);
    if !dest.is_dir() {
        let staging = base.join(format!("{name}.tmp"));
        let _ = fs::remove_dir_all(&staging);
        let extracted = extract(archive, &staging).and_then(|()| {
            fs::rename(&staging, &dest)
                .with_context(|| format!("Failed to finalize {}", dest.display()))
        });
        if let Err(e) = extracted {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
    }

    if let Ok(entries) = fs::read_dir(base) {
        let older = format!("{prefix}-");
        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().into_owned();
            if entry_name != name && entry_name.starts_with(&older) && entry.path().is_dir() {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(dest)
}

/// Cached variant of [`download_verified`].
pub fn get_or_download_verified(
    client: &Client,
    url: &str,
    filename: &str,
    sha256: &str,
    progress: ProgressCallback,
) -> Result<PathBuf> {
    let dest = cache_dir().join(filename);
    download_verified(client, url, &dest, sha256, progress)?;
    Ok(dest)
}

/// Downloads `url` to `dest`, replacing an older copy. If the download fails, an existing copy is
/// kept and used instead. Meant for files that upstream rebuilds in place without a checksum.
pub fn download_refreshing(
    client: &Client,
    url: &str,
    dest: &Path,
    progress: ProgressCallback,
) -> Result<()> {
    match download_with_progress(client, url, dest, progress) {
        Err(_) if dest.is_file() => Ok(()),
        result => result,
    }
}

/// Cache file name for `filename` downloaded from `url`, so equally named files from different
/// sources never share a cache entry.
pub fn url_cache_name(url: &str, filename: &str) -> String {
    use sha2::{Digest, Sha256};

    format!("{}-{filename}", &hex(&Sha256::digest(url.as_bytes()))[..12])
}

/// A GitHub release asset together with the SHA-256 GitHub publishes for it.
#[derive(Debug, Clone)]
pub struct GithubAsset {
    pub tag: String,
    pub download_url: String,
    pub sha256: String,
}

/// Picks `asset_name` out of a GitHub release API response. Assets without a published SHA-256
/// digest are rejected, because they could not be verified.
pub fn parse_release_asset(release: &serde_json::Value, asset_name: &str) -> Result<GithubAsset> {
    let asset = release["assets"]
        .as_array()
        .and_then(|assets| assets.iter().find(|a| a["name"] == asset_name))
        .ok_or_else(|| anyhow!("The release has no asset named {asset_name}"))?;
    let sha256 = asset["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| anyhow!("GitHub publishes no SHA-256 checksum for {asset_name}"))?;
    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow!("The release lists no download URL for {asset_name}"))?;
    Ok(GithubAsset {
        tag: release["tag_name"].as_str().unwrap_or_default().to_string(),
        download_url: download_url.to_string(),
        sha256: sha256.to_ascii_lowercase(),
    })
}

/// Downloads `asset_name` from the latest release of `owner_repo`, verified against the SHA-256
/// GitHub publishes for it, and returns the file and its checksum. Cached copies are named
/// `<sha256>-<asset_name>`, so a new upstream build is picked up even when it is re-published
/// under the same tag. When GitHub cannot be reached (offline, rate-limited), a cached copy that
/// still matches the checksum in its name is used instead.
pub fn get_latest_github_asset(
    client: &Client,
    owner_repo: &str,
    asset_name: &str,
    progress: ProgressCallback,
) -> Result<(PathBuf, String)> {
    let latest = (|| -> Result<(PathBuf, String)> {
        let api_url = format!("https://api.github.com/repos/{owner_repo}/releases/latest");
        let release: serde_json::Value = client.get(&api_url).send()?.error_for_status()?.json()?;
        let asset = parse_release_asset(&release, asset_name)?;
        let cache_name = format!("{}-{asset_name}", asset.sha256);
        let dest = cache_dir().join(&cache_name);
        download_verified(client, &asset.download_url, &dest, &asset.sha256, progress)?;
        remove_superseded_copies(&cache_dir(), &cache_name, asset_name);
        Ok((dest, asset.sha256))
    })();
    latest.or_else(|e| find_cached_github_asset(&cache_dir(), asset_name).ok_or(e))
}

/// Finds a cached `<sha256>-<asset_name>` whose content still matches the checksum in its name.
pub fn find_cached_github_asset(dir: &Path, asset_name: &str) -> Option<(PathBuf, String)> {
    let suffix = format!("-{asset_name}");
    fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        let sha256 = name.strip_suffix(&suffix)?;
        let is_checksum = sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit());
        (is_checksum && matches_sha256(&entry.path(), sha256))
            .then(|| (entry.path(), sha256.to_string()))
    })
}

/// Removes cached `<sha256>-<asset_name>` files other than `current`.
pub fn remove_superseded_copies(dir: &Path, current: &str, asset_name: &str) {
    let suffix = format!("-{asset_name}");
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_cached_copy = name.strip_suffix(&suffix).is_some_and(|sha256| {
            sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit())
        });
        if name != current && is_cached_copy {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Downloads and caches a file if not already present in the cache.
pub fn get_or_download_cached(
    client: &Client,
    url: &str,
    filename: &str,
    progress: ProgressCallback,
) -> Result<PathBuf> {
    let dest = cache_dir().join(filename);
    if dest.is_file() && fs::metadata(&dest)?.len() > 0 {
        return Ok(dest);
    }
    download_with_progress(client, url, &dest, progress)?;
    Ok(dest)
}

/// Extracts a zip file to a destination directory.
pub fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    fs::create_dir_all(dest_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

/// Extracts a 7-Zip file to a destination directory.
pub fn extract_7z(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    fs::create_dir_all(dest_dir)?;
    sevenz_rust::decompress_file(archive_path, dest_dir)
        .map_err(|e| anyhow!("Failed to decompress 7z: {e}"))?;
    Ok(())
}
