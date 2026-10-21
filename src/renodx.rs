//! RenoDX HDR mod directory, online synchronization, and game matching.

use regex::Regex;
use reqwest::blocking::Client;
use reqwest::Url;
use std::collections::HashMap;

pub const RENODX_WIKI_MODS: &str =
    "https://raw.githubusercontent.com/wiki/clshortfuse/renodx/Mods.md";

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
/// Returns an empty catalog when the wiki cannot be reached.
pub fn fetch_mods_catalog(client: &Client) -> HashMap<String, RenoDxMod> {
    client
        .get(RENODX_WIKI_MODS)
        .send()
        .and_then(|resp| resp.error_for_status())
        .and_then(|resp| resp.text())
        .map(|text| parse_mods_catalog(&text))
        .unwrap_or_default()
}

/// Parses the RenoDX wiki `Mods.md` table into a catalog keyed by normalized title.
///
/// The wiki is community-maintained and the add-ons are loaded into the game process, so rows
/// are only accepted when the download is HTTPS from GitHub (where every RenoDX mod is hosted)
/// and the file name is a plain `*.addon64` name that cannot point outside the game folder.
pub fn parse_mods_catalog(markdown: &str) -> HashMap<String, RenoDxMod> {
    let mut catalog = HashMap::new();
    let re_row = Regex::new(r"\|\s*([^|]+)\s*\|\s*([^|]+)\s*\|\s*\[!\[[^\]]*\]\([^)]*\)\]\(([^)]+\.addon64)\)\s*\|\s*([^|]*)\|").unwrap();

    for cap in re_row.captures_iter(markdown) {
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
        let Some(filename) = github_addon_file_name(&url) else {
            continue;
        };
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

/// Returns the add-on's file name when `url` is an HTTPS download from GitHub: a release asset
/// on github.com (`/<owner>/<repo>/releases/download/<tag>/<file>`) or a file on a GitHub Pages
/// site. Other github.com paths are refused because anyone can upload issue attachments there.
///
/// The URL is checked with the parser the HTTP client uses, so the host that is checked is the
/// host that gets contacted. Userinfo, ports, queries, fragments, and backslashes (which URL
/// parsers disagree about) are rejected outright.
fn github_addon_file_name(url: &str) -> Option<String> {
    if url.contains('\\') {
        return None;
    }
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let segments: Vec<&str> = parsed.path_segments()?.collect();
    let is_release_asset = segments.len() == 6 && segments[2..4] == ["releases", "download"];
    if !((host == "github.com" && is_release_asset) || host.ends_with(".github.io")) {
        return None;
    }
    let name = *segments.last()?;
    is_plain_addon_name(name).then(|| name.to_string())
}

fn is_plain_addon_name(name: &str) -> bool {
    name.ends_with(".addon64")
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
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
