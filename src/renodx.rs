//! RenoDX HDR mod directory, online synchronization, and game matching.

use regex::Regex;
use reqwest::blocking::Client;
use std::collections::HashMap;

pub const RENODX_WIKI_MODS: &str =
    "https://raw.githubusercontent.com/wiki/clshortfuse/renodx/Mods.md";
pub const RENODX_SNAPSHOT_BASE: &str =
    "https://github.com/clshortfuse/renodx/releases/download/snapshot/";

#[derive(Debug, Clone)]
pub struct RenoDxMod {
    pub game_title: String,
    pub filename: String,
    pub download_url: String,
    pub notes: Option<String>,
}

/// Normalizes a game title for fuzzy matching (lowercased, alphanumeric only).
pub fn normalize_title(title: &str) -> String {
    let lower = title.to_lowercase();
    let re = Regex::new(r"[^a-z0-9]").unwrap();
    re.replace_all(&lower, "").to_string()
}

/// Fetches the latest RenoDX game mods catalog from the official wiki.
pub fn fetch_mods_catalog(client: &Client) -> HashMap<String, RenoDxMod> {
    let mut catalog = get_fallback_catalog();

    let Ok(resp) = client.get(RENODX_WIKI_MODS).send() else {
        return catalog;
    };
    let Ok(text) = resp.text() else {
        return catalog;
    };

    let re_row = Regex::new(r"\|\s*([^|]+)\s*\|\s*([^|]+)\s*\|\s*\[!\[[^\]]*\]\([^)]*\)\]\(([^)]+\.addon64)\)\s*\|\s*([^|]*)\|").unwrap();

    for cap in re_row.captures_iter(&text) {
        let raw_title = cap[1].trim();
        // Strip markdown links if present in title [Title](url)
        let clean_title = if let Some(start) = raw_title.find('[') {
            if let Some(end) = raw_title.find(']') {
                &raw_title[start + 1..end]
            } else {
                raw_title
            }
        } else {
            raw_title
        };

        let url = cap[3].trim().to_string();
        let filename = url
            .split('/')
            .last()
            .unwrap_or("renodx.addon64")
            .to_string();
        let notes = cap.get(4).map(|m| m.as_str().trim().to_string());

        let norm = normalize_title(clean_title);
        catalog.insert(
            norm,
            RenoDxMod {
                game_title: clean_title.to_string(),
                filename,
                download_url: url,
                notes,
            },
        );
    }

    catalog
}

/// Matches a game title to a RenoDX mod if one exists.
pub fn match_game<'a>(
    game_title: &str,
    catalog: &'a HashMap<String, RenoDxMod>,
) -> Option<&'a RenoDxMod> {
    let norm = normalize_title(game_title);

    // Direct match
    if let Some(m) = catalog.get(&norm) {
        return Some(m);
    }

    // Substring match
    for (k, v) in catalog {
        if norm.contains(k) || k.contains(&norm) {
            // Avoid matching overly short names trivially
            if k.len() >= 4 && norm.len() >= 4 {
                return Some(v);
            }
        }
    }

    None
}

/// Provides a curated offline fallback catalog of popular RenoDX supported games.
fn get_fallback_catalog() -> HashMap<String, RenoDxMod> {
    let mut map = HashMap::new();

    let list = [
        ("Cyberpunk 2077", "renodx-cyberpunk2077.addon64"),
        ("The Witcher 3", "renodx-witcher3.addon64"),
        ("Alan Wake 2", "renodx-alanwake2.addon64"),
        ("Dragon's Dogma 2", "renodx-dragonsdogma2.addon64"),
        ("Baldur's Gate 3", "renodx-baldursgate3.addon64"),
        ("Starfield", "renodx-starfield.addon64"),
        ("Black Myth: Wukong", "renodx-blackmythwukong.addon64"),
        ("Hogwarts Legacy", "renodx-hogwartslegacy.addon64"),
        ("Elden Ring", "renodx-eldenring.addon64"),
        ("Armored Core VI", "renodx-armoredcore6.addon64"),
        ("Ghost of Tsushima", "renodx-ghostoftsushima.addon64"),
        ("Horizon Forbidden West", "renodx-horizonforbiddenwest.addon64"),
        ("God of War Ragnarok", "renodx-godofwarragnarok.addon64"),
        ("Lies of P", "renodx-liesofp.addon64"),
        ("Final Fantasy VII Rebirth", "renodx-ff7rebirth.addon64"),
        ("Final Fantasy XVI", "renodx-ff16.addon64"),
        ("Palworld", "renodx-palworld.addon64"),
        ("Helldivers 2", "renodx-helldivers2.addon64"),
        ("Resident Evil 4", "renodx-re4.addon64"),
        ("Resident Evil Village", "renodx-re8.addon64"),
    ];

    for (title, file) in list {
        let norm = normalize_title(title);
        map.insert(
            norm,
            RenoDxMod {
                game_title: title.to_string(),
                filename: file.to_string(),
                download_url: format!("{RENODX_SNAPSHOT_BASE}{file}"),
                notes: Some("Official RenoDX HDR mod".to_string()),
            },
        );
    }

    map
}
