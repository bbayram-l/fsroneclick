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
        .user_agent("FSRoneclick-Installer/1.0")
        .build()
        .context("Failed to build HTTP client")
}

/// Downloads a URL to a specified file, reporting progress.
pub fn download_with_progress(
    client: &Client,
    url: &str,
    dest: &Path,
    mut progress: ProgressCallback,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

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

    progress(1.0, "Download completed");
    Ok(())
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

/// Finds the latest asset download URL for a GitHub repo matching a file extension or regex pattern.
pub fn get_github_latest_asset_url(
    client: &Client,
    owner_repo: &str,
    extension: &str,
) -> Result<String> {
    let api_url = format!("https://api.github.com/repos/{owner_repo}/releases/latest");
    let resp: serde_json::Value = client.get(&api_url).send()?.json()?;

    let assets = resp["assets"]
        .as_array()
        .ok_or_else(|| anyhow!("No assets found in release"))?;

    for asset in assets {
        if let (Some(name), Some(url)) = (
            asset["name"].as_str(),
            asset["browser_download_url"].as_str(),
        ) {
            if name.ends_with(extension) {
                return Ok(url.to_string());
            }
        }
    }

    Err(anyhow!("No asset with extension {extension} found in {owner_repo}"))
}
