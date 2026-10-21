//! Online Update Checker and Compatibility Database Manager.
//! Checks GitHub releases for OptiScaler, live wiki updates, and RenoDX catalog updates.

use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CURRENT_APP_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));
pub const COMPAT_WIKI_RAW_URL: &str =
    "https://raw.githubusercontent.com/wiki/optiscaler/OptiScaler/Compatibility-List.md";
const RESHADE_HOMEPAGE: &str = "https://reshade.me";
/// A synced compatibility list older than this is reported as due for a refresh.
const COMPAT_SYNC_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCompatEntry {
    pub name: String,
    pub norm_name: String,
    pub status: String,
    pub inputs: String,
    pub optipatcher: bool,
    pub notes: String,
    pub recommended_hook: Option<String>,
    pub auto_exposure: bool,
    pub non_linear_color_space: bool,
    pub disable_spoofing: bool,
    pub fsr2_pattern: bool,
    pub force_dx12_param: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCompatDatabase {
    /// Human-readable sync time, e.g. "2026-10-21 14:05 UTC".
    pub updated_at: String,
    /// Sync time in seconds since the Unix epoch (0 for caches written by older versions).
    #[serde(default)]
    pub synced_unix: u64,
    pub total_entries: usize,
    pub entries: Vec<CachedCompatEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentState {
    UpToDate,
    UpdateAvailable,
    /// No online source could be checked (offline, rate-limited, or the data is embedded).
    NotChecked,
}

#[derive(Debug, Clone)]
pub struct ComponentStatus {
    pub name: String,
    pub installed_version: String,
    pub latest_version: Option<String>,
    pub state: ComponentState,
    pub details: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckReport {
    pub components: Vec<ComponentStatus>,
    pub has_app_update: bool,
}

pub fn cache_file_path() -> PathBuf {
    crate::net::cache_dir().join("compatibility_cache.json")
}

/// Parses the raw markdown from GitHub wiki into structured cached entries.
pub fn parse_raw_compatibility_md(markdown: &str) -> Vec<CachedCompatEntry> {
    let mut entries = Vec::new();
    let mut in_table = false;

    for line in markdown.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
        // A table row with 6 columns produces 8 parts when split on '|': ["", col1, col2, col3, col4, col5, col6, ""]
        if parts.len() < 7 {
            continue;
        }

        let col_game = parts[1];
        let col_compat = parts[2];
        let col_inputs = parts[3];
        let col_patcher = parts[4];
        let col_notes = parts[5];

        if col_game.contains("Game") && col_compat.contains("Compatibility") {
            in_table = true;
            continue;
        }
        if col_game.starts_with("---") || col_game.starts_with(":--") || !in_table {
            continue;
        }

        // Clean name from markdown link: [Title](link) -> Title
        let clean_name = if let Some(start) = col_game.find('[') {
            if let Some(end) = col_game.find(']') {
                if end > start {
                    &col_game[start + 1..end]
                } else {
                    col_game
                }
            } else {
                col_game
            }
        } else {
            col_game
        }
        .trim();

        if clean_name.is_empty() {
            continue;
        }

        let status = if col_compat.contains("✅") {
            "Working"
        } else if col_compat.contains("❌") {
            "NotWorking"
        } else {
            "Partial"
        };

        let inputs = col_inputs.replace("<br>", " ").trim().to_string();
        let optipatcher = col_patcher.contains("✨");

        // Clean notes
        let mut notes_clean = col_notes.replace('`', "").replace("<br>", " ");
        // Strip markdown links in notes: [text](url) -> text
        while let Some(start) = notes_clean.find('[') {
            if let Some(mid) = notes_clean[start..].find(']') {
                let mid_idx = start + mid;
                if let Some(end) = notes_clean[mid_idx..].find(')') {
                    let end_idx = mid_idx + end;
                    let link_text = &notes_clean[start + 1..mid_idx];
                    notes_clean = format!(
                        "{}{}{}",
                        &notes_clean[..start],
                        link_text,
                        &notes_clean[end_idx + 1..]
                    );
                    continue;
                }
            }
            break;
        }
        let notes_clean = notes_clean.trim().to_string();

        let notes_lower = notes_clean.to_lowercase();
        let rec_hook = if notes_lower.contains("winmm.dll") || notes_lower.contains("winmm") {
            Some("winmm.dll".to_string())
        } else if notes_lower.contains("d3d12.dll") {
            Some("d3d12.dll".to_string())
        } else if notes_lower.contains("version.dll") {
            Some("version.dll".to_string())
        } else {
            None
        };

        let auto_exp =
            notes_lower.contains("autoexposure on") || notes_lower.contains("autoexposure");
        let non_linear = notes_lower.contains("non-linear")
            || notes_lower.contains("srgb")
            || notes_lower.contains("pq input");
        let disable_spoof =
            notes_lower.contains("disable spoof") || notes_lower.contains("without spoof");
        let fsr2_pat = notes_lower.contains("fsr2pattern");
        let force_dx12 = notes_lower.contains("-dx12") || notes_lower.contains("-force-d3d12");

        let norm_name = crate::compatibility::normalize_title(clean_name);

        entries.push(CachedCompatEntry {
            name: clean_name.to_string(),
            norm_name,
            status: status.to_string(),
            inputs,
            optipatcher,
            notes: notes_clean,
            recommended_hook: rec_hook,
            auto_exposure: auto_exp,
            non_linear_color_space: non_linear,
            disable_spoofing: disable_spoof,
            fsr2_pattern: fsr2_pat,
            force_dx12_param: force_dx12,
        });
    }

    entries
}

/// Fetches the live compatibility markdown from GitHub wiki, parses it, and caches it locally.
pub fn fetch_and_cache_compatibility_db(client: &Client) -> Result<CachedCompatDatabase> {
    let resp = client.get(COMPAT_WIKI_RAW_URL).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch live compatibility list: HTTP {}",
            resp.status()
        ));
    }
    let body = resp.text()?;
    let entries = parse_raw_compatibility_md(&body);
    if entries.is_empty() {
        return Err(anyhow!("Parsed 0 entries from compatibility list"));
    }

    let count = entries.len();
    let now = unix_now();
    let db = CachedCompatDatabase {
        updated_at: format_utc(now),
        synced_unix: now,
        total_entries: count,
        entries,
    };

    let cache_path = cache_file_path();
    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&db)?;
    fs::write(&cache_path, json)?;

    // Update the in-memory runtime database
    crate::compatibility::load_dynamic_entries(&db.entries);

    Ok(db)
}

/// Loads cached compatibility entries if available.
pub fn load_cached_compatibility_db() -> Option<CachedCompatDatabase> {
    let cache_path = cache_file_path();
    if !cache_path.is_file() {
        return None;
    }
    let Ok(content) = fs::read_to_string(&cache_path) else {
        return None;
    };
    let Ok(db) = serde_json::from_str::<CachedCompatDatabase>(&content) else {
        return None;
    };
    crate::compatibility::load_dynamic_entries(&db.entries);
    Some(db)
}

/// Checks online versions and returns a structured update report.
pub fn check_all_updates(client: &Client) -> UpdateCheckReport {
    let mut components = Vec::new();

    // 1. OptiScaler release (the installer pins a specific, checksum-verified build)
    components.push(versioned_status(
        "OptiScaler Engine",
        &format!("v{}", crate::installer::OPTISCALER_VERSION),
        github_latest_tag(client, "optiscaler/OptiScaler"),
        "Pinned, checksum-verified release used for installs (FSR 3.1 / FSR 4, frame generation)",
    ));

    // 2. OptiScaler compatibility list (embedded, optionally synced from the live wiki)
    let cached_db = load_cached_compatibility_db();
    let (installed_version, state, details) = match &cached_db {
        Some(db) => {
            let stale = unix_now().saturating_sub(db.synced_unix) > COMPAT_SYNC_MAX_AGE_SECS;
            (
                format!("{} games", db.total_entries),
                if stale {
                    ComponentState::UpdateAvailable
                } else {
                    ComponentState::UpToDate
                },
                format!("Synced from the live wiki on {}", db.updated_at),
            )
        }
        None => (
            format!(
                "{} games",
                crate::compatibility::embedded_optiscaler_count()
            ),
            ComponentState::UpdateAvailable,
            "Embedded list; sync to get the latest community results".to_string(),
        ),
    };
    components.push(ComponentStatus {
        name: "OptiScaler Compatibility DB".to_string(),
        installed_version,
        latest_version: Some("Live wiki".to_string()),
        state,
        details,
    });

    // 3. AMD FSR Redstone catalog (embedded only; there is no machine-readable upstream)
    components.push(ComponentStatus {
        name: "AMD FSR Redstone DB".to_string(),
        installed_version: format!("{} games", crate::compatibility::embedded_redstone_count()),
        latest_version: None,
        state: ComponentState::NotChecked,
        details: "Embedded AMD FSR 4 'Redstone' catalog, updated with FSRoneclick releases"
            .to_string(),
    });

    // 4. ReShade add-on build (pinned and checksum-verified, like OptiScaler)
    let reshade_latest = client
        .get(RESHADE_HOMEPAGE)
        .send()
        .and_then(|resp| resp.error_for_status())
        .and_then(|resp| resp.text())
        .ok()
        .and_then(|html| parse_reshade_latest_version(&html))
        .map(|version| format!("v{version}"));
    components.push(versioned_status(
        "ReShade Add-on Build",
        &format!("v{}", crate::installer::RESHADE_VERSION),
        reshade_latest,
        "Pinned, checksum-verified ReShade with add-on support (required by RenoDX)",
    ));

    // 5. RenoDX HDR mod catalog (always read live from the wiki)
    let renodx_count = crate::renodx::fetch_mods_catalog(client).len();
    let renodx_live = renodx_count > 0;
    components.push(ComponentStatus {
        name: "RenoDX HDR Catalog".to_string(),
        installed_version: format!("{renodx_count} mods"),
        latest_version: renodx_live.then(|| format!("{renodx_count} mods")),
        state: if renodx_live {
            ComponentState::UpToDate
        } else {
            ComponentState::NotChecked
        },
        details: if renodx_live {
            "Read live from the RenoDX wiki at every install".to_string()
        } else {
            "The RenoDX wiki could not be reached".to_string()
        },
    });

    // 6. FSRoneclick itself
    let app_repo = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/");
    let app = versioned_status(
        "FSRoneclick App",
        CURRENT_APP_VERSION,
        github_latest_tag(client, app_repo),
        "Download new versions from the GitHub releases page",
    );
    let has_app_update = app.state == ComponentState::UpdateAvailable;
    components.push(app);

    UpdateCheckReport {
        components,
        has_app_update,
    }
}

fn versioned_status(
    name: &str,
    installed: &str,
    latest: Option<String>,
    details: &str,
) -> ComponentStatus {
    let state = match &latest {
        Some(latest) if is_newer_version(latest, installed) => ComponentState::UpdateAvailable,
        Some(_) => ComponentState::UpToDate,
        None => ComponentState::NotChecked,
    };
    ComponentStatus {
        name: name.to_string(),
        installed_version: installed.to_string(),
        latest_version: latest,
        state,
        details: details.to_string(),
    }
}

fn github_latest_tag(client: &Client, owner_repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{owner_repo}/releases/latest");
    let release: serde_json::Value = client
        .get(url)
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .ok()?;
    release["tag_name"].as_str().map(str::to_string)
}

/// Whether `latest` is a newer version than `current` (e.g. "v0.9.5" vs "0.9.4"). Only the
/// leading dotted numbers are compared, so suffixes like "-final" or " (Addon)" are ignored, and
/// tags that are not version numbers never count as newer.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (version_numbers(latest), version_numbers(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn version_numbers(version: &str) -> Option<Vec<u64>> {
    let core: String = version
        .trim()
        .trim_start_matches(['v', 'V'])
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let mut parts = core
        .split('.')
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u64>>>()?;
    // "1.0.0" and "1.0" are the same version.
    while parts.len() > 1 && parts.last() == Some(&0) {
        parts.pop();
    }
    (!parts.is_empty()).then_some(parts)
}

/// Finds the newest ReShade add-on build linked from the reshade.me homepage.
pub fn parse_reshade_latest_version(html: &str) -> Option<String> {
    let link = Regex::new(r"ReShade_Setup_(\d+(?:\.\d+)+)_Addon\.exe").unwrap();
    link.captures_iter(html)
        .map(|cap| cap[1].to_string())
        .max_by_key(|version| version_numbers(version).unwrap_or_default())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Formats seconds since the Unix epoch as `YYYY-MM-DD HH:MM UTC`.
pub fn format_utc(unix_secs: u64) -> String {
    // Civil-from-days conversion (Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms").
    let days = (unix_secs / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    let seconds_of_day = unix_secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        seconds_of_day / 3_600,
        seconds_of_day % 3_600 / 60
    )
}
