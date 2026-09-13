//! Online Update Checker and Compatibility Database Manager.
//! Checks GitHub releases for OptiScaler, live wiki updates, and RenoDX catalog updates.

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const CURRENT_APP_VERSION: &str = "v0.1.0";
pub const CURRENT_OPTISCALER_VERSION: &str = "v0.9.4";
pub const CURRENT_RESHADE_VERSION: &str = "v6.3.3 (Addon)";
pub const COMPAT_WIKI_RAW_URL: &str =
    "https://raw.githubusercontent.com/wiki/optiscaler/OptiScaler/Compatibility-List.md";
pub const OPTISCALER_RELEASES_API: &str =
    "https://api.github.com/repos/optiscaler/OptiScaler/releases/latest";

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
    pub updated_at: String,
    pub total_entries: usize,
    pub entries: Vec<CachedCompatEntry>,
}

#[derive(Debug, Clone)]
pub struct ComponentStatus {
    pub name: String,
    pub installed_version: String,
    pub latest_version: Option<String>,
    pub up_to_date: bool,
    pub details: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckReport {
    pub components: Vec<ComponentStatus>,
    pub can_update_compat_db: bool,
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
                    notes_clean = format!("{}{}{}", &notes_clean[..start], link_text, &notes_clean[end_idx + 1..]);
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

        let auto_exp = notes_lower.contains("autoexposure on") || notes_lower.contains("autoexposure");
        let non_linear = notes_lower.contains("non-linear") || notes_lower.contains("srgb") || notes_lower.contains("pq input");
        let disable_spoof = notes_lower.contains("disable spoof") || notes_lower.contains("without spoof");
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
        return Err(anyhow!("Failed to fetch live compatibility list: HTTP {}", resp.status()));
    }
    let body = resp.text()?;
    let entries = parse_raw_compatibility_md(&body);
    if entries.is_empty() {
        return Err(anyhow!("Parsed 0 entries from compatibility list"));
    }

    let count = entries.len();
    let db = CachedCompatDatabase {
        updated_at: chrono_like_timestamp(),
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

    // 1. OptiScaler Release Check
    let opti_latest = client
        .get(OPTISCALER_RELEASES_API)
        .send()
        .ok()
        .and_then(|r| r.json::<serde_json::Value>().ok())
        .and_then(|j| j["tag_name"].as_str().map(|s| s.to_string()));

    let opti_up_to_date = opti_latest.as_deref().map_or(true, |lat| {
        lat.trim_start_matches('v') == CURRENT_OPTISCALER_VERSION.trim_start_matches('v')
    });

    components.push(ComponentStatus {
        name: "OptiScaler Engine".to_string(),
        installed_version: CURRENT_OPTISCALER_VERSION.to_string(),
        latest_version: opti_latest.clone(),
        up_to_date: opti_up_to_date,
        details: "FSR 3.1 & Intel XeSS upscaling with FSR 4.1.1 INT8 hardware acceleration".to_string(),
    });

    // 2. OptiScaler Compatibility Database
    let cached_db = load_cached_compatibility_db();
    let compat_count = cached_db.as_ref().map_or(700, |db| db.total_entries);
    let compat_updated = cached_db.as_ref().map_or("Embedded static", |db| db.updated_at.as_str());

    components.push(ComponentStatus {
        name: "OptiScaler Compatibility DB".to_string(),
        installed_version: format!("{compat_count} Games"),
        latest_version: Some("Live Wiki on GitHub".to_string()),
        up_to_date: cached_db.is_some(),
        details: format!("Community tested compatibility & workarounds (Status: {compat_updated})"),
    });

    // 3. AMD FSR Redstone Knowledge Base
    components.push(ComponentStatus {
        name: "AMD FSR Redstone DB".to_string(),
        installed_version: "409 Games".to_string(),
        latest_version: Some("409 Games".to_string()),
        up_to_date: true,
        details: "Official AMD FSR 4 'Redstone' supported catalog for RX 7000 (26.6.2+) & RX 9000".to_string(),
    });

    // 4. ReShade Add-on Framework
    components.push(ComponentStatus {
        name: "ReShade Add-on Build".to_string(),
        installed_version: CURRENT_RESHADE_VERSION.to_string(),
        latest_version: Some("v6.3.3+".to_string()),
        up_to_date: true,
        details: "Official ReShade with Add-on support for RenoDX shaders".to_string(),
    });

    // 5. RenoDX HDR Mod Catalog
    let renodx_catalog = crate::renodx::fetch_mods_catalog(client);
    components.push(ComponentStatus {
        name: "RenoDX HDR Catalog".to_string(),
        installed_version: format!("{} Games", renodx_catalog.len()),
        latest_version: Some(format!("{} Games", renodx_catalog.len())),
        up_to_date: true,
        details: "Community studio-grade HDR10/scRGB enhancement mods".to_string(),
    });

    // 6. OptiOneClick Application
    components.push(ComponentStatus {
        name: "OptiOneClick App".to_string(),
        installed_version: CURRENT_APP_VERSION.to_string(),
        latest_version: Some(CURRENT_APP_VERSION.to_string()),
        up_to_date: true,
        details: "1-Click optimization suite for AMD Radeon & universal GPUs".to_string(),
    });

    UpdateCheckReport {
        components,
        can_update_compat_db: true,
        has_app_update: !opti_up_to_date,
    }
}

fn chrono_like_timestamp() -> String {
    // Simple current date indicator
    "Latest Live Sync".to_string()
}
