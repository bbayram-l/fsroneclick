//! Official AMD FidelityFX Super Resolution (FSR) game catalog and detection.
//! Based on AMD's official FidelityFX supported games:
//! - FSR 3 with Frame Generation (#fsr4-item-6514045aac-tab)
//! - FSR 2 (#fsr4-item-7a3bbedc7f-tab)
//! - FSR 1 (#fsr4-item-dae8c7ecb4-tab)
//! - Upcoming / FSR 3.1 / FSR 4 (#fsr4-item-60178a43f6-tab)

use crate::renodx::normalize_title;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityFxTier {
    /// FSR 3 / 3.1 with AMD Fluid Motion Frames / Frame Generation
    Fsr3WithFrameGen,
    /// FSR 2 (Temporal Upscaling) - OptiScaler can upgrade this to FSR 3.1 & Frame Gen!
    Fsr2,
    /// FSR 1 (Spatial Upscaling)
    Fsr1,
    /// Upcoming official FSR 3 / 4 title
    FsrUpcoming,
}

impl FidelityFxTier {
    pub fn label(&self) -> &'static str {
        match self {
            FidelityFxTier::Fsr3WithFrameGen => "FSR 3 + Frame Generation (Native)",
            FidelityFxTier::Fsr2 => "FSR 2 (Native - Upgradable to FSR 3.1 & FG)",
            FidelityFxTier::Fsr1 => "FSR 1 (Native Spatial)",
            FidelityFxTier::FsrUpcoming => "FSR 3 / FSR 4 (Upcoming)",
        }
    }
}

pub struct FidelityFxCatalog {
    games: HashMap<String, FidelityFxTier>,
}

impl Default for FidelityFxCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl FidelityFxCatalog {
    pub fn new() -> Self {
        let mut map = HashMap::new();

        // 1. FSR 3 with Frame Generation (Official AMD Catalog)
        let fsr3_games = [
            "Avatar: Frontiers of Pandora",
            "Starfield",
            "Ghost of Tsushima",
            "Black Myth: Wukong",
            "Call of Duty: Modern Warfare III",
            "Call of Duty: Warzone",
            "Cyberpunk 2077",
            "God of War Ragnarok",
            "The Last of Us Part I",
            "RoboCop: Rogue City",
            "Star Wars Outlaws",
            "Immortals of Aveum",
            "Forspoken",
            "Like a Dragon: Infinite Wealth",
            "Like a Dragon Gaiden: The Man Who Erased His Name",
            "The Crew Motorfest",
            "Remnant II",
            "Senua's Saga: Hellblade II",
            "Frostpunk 2",
            "Warhammer 40,000: Space Marine 2",
            "Until Dawn",
            "Horizon Forbidden West",
            "Dragon's Dogma 2",
            "The First Descendant",
            "Palworld",
            "Still Wakes the Deep",
            "Flintlock: The Siege of Dawn",
            "Farming Simulator 25",
            "Nobody Wants to Die",
            "Kingdom Come: Deliverance II",
            "Marvel's Spider-Man Remastered",
            "Marvel's Spider-Man: Miles Morales",
            "Ratchet & Clank: Rift Apart",
            "Ghostrunner 2",
            "Talos Principle 2",
            "Estencel",
            "Sons of the Forest",
            "Pax Dei",
        ];

        for title in fsr3_games {
            map.insert(normalize_title(title), FidelityFxTier::Fsr3WithFrameGen);
        }

        // 2. FSR 2 (Official AMD Catalog)
        let fsr2_games = [
            "Far Cry 6",
            "Deathloop",
            "God of War",
            "Red Dead Redemption 2",
            "The Witcher 3: Wild Hunt",
            "The Witcher 3",
            "Dead Space",
            "Dead Island 2",
            "Star Wars Jedi: Survivor",
            "Dying Light 2 Stay Human",
            "Resident Evil 4",
            "Returnal",
            "Gotham Knights",
            "Scorn",
            "UNCHARTED: Legacy of Thieves Collection",
            "Tiny Tina's Wonderlands",
            "F1 22",
            "F1 23",
            "F1 24",
            "Hitman World of Assassination",
            "No Man's Sky",
            "Chernobylite",
            "Genshin Impact",
            "Microsoft Flight Simulator",
            "The Callisto Protocol",
            "A Plague Tale: Requiem",
            "Lies of P",
            "Lords of the Fallen",
            "The Talos Principle 2",
            "Pacific Drive",
            "Banishers: Ghosts of New Eden",
            "Tekken 8",
            "Suicide Squad: Kill the Justice League",
            "Helldivers 2",
        ];

        for title in fsr2_games {
            map.insert(normalize_title(title), FidelityFxTier::Fsr2);
        }

        // 3. FSR 1 (Official AMD Catalog)
        let fsr1_games = [
            "Resident Evil Village",
            "Dota 2",
            "Anno 1800",
            "Godfall",
            "Horizon Zero Dawn",
            "Terminator: Resistance",
            "Kingshunt",
            "Myst",
            "The Riftbreaker",
            "Chernobylite",
            "Necromunda: Hired Gun",
            "Edge of Eternity",
            "Evil Genius 2: World Domination",
            "Vampire: The Masquerade - Bloodhunt",
            "Grounded",
            "World War Z: Aftermath",
            "Warhammer: Vermintide 2",
            "Baldur's Gate 3",
        ];

        for title in fsr1_games {
            // Don't overwrite higher tier if already present
            map.entry(normalize_title(title))
                .or_insert(FidelityFxTier::Fsr1);
        }

        Self { games: map }
    }

    /// Queries whether the title has official AMD FidelityFX support or on-disk FSR DLLs.
    pub fn check_game(&self, title: &str, game_dir: &Path) -> Option<FidelityFxTier> {
        // Check for on-disk FidelityFX / FSR DLLs in game directory
        if game_dir.join("amd_fidelityfx_dx12.dll").is_file()
            || game_dir.join("amd_fidelityfx_vk.dll").is_file()
            || game_dir.join("ffx_backend_dx12_x64.dll").is_file()
        {
            return Some(FidelityFxTier::Fsr3WithFrameGen);
        }

        if game_dir.join("ffx_fsr2_api_dx12_x64.dll").is_file()
            || game_dir.join("amd_ags_x64.dll").is_file() && game_dir.join("fsr2.dll").is_file()
        {
            return Some(FidelityFxTier::Fsr2);
        }

        let norm = normalize_title(title);
        if let Some(&tier) = self.games.get(&norm) {
            return Some(tier);
        }

        // Fuzzy match
        for (k, &tier) in &self.games {
            if (norm.contains(k) || k.contains(&norm)) && (k.len() >= 5 && norm.len() >= 5) {
                return Some(tier);
            }
        }

        None
    }
}
