use fsroneclick::game;
use fsroneclick::gpu;
use fsroneclick::gui;
use fsroneclick::installer;
use fsroneclick::net;
use fsroneclick::renodx;

use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--check-updates") {
        run_check_updates_cli();
        return Ok(());
    }

    if args.iter().any(|a| a == "--update-compat") {
        run_update_compat_cli();
        return Ok(());
    }

    if args.iter().any(|a| a == "--list-games") {
        run_list_games();
        return Ok(());
    }

    if args.iter().any(|a| a == "--check") {
        let path = args.iter().skip(1).find(|a| *a != "--check");
        if let Some(p) = path {
            run_check(p);
            return Ok(());
        } else {
            eprintln!("Error: Missing game directory path.\nUsage: fsroneclick --check <game_path>");
            std::process::exit(1);
        }
    }

    if args.iter().any(|a| a == "--install") {
        let path = args.iter().skip(1).find(|a| *a != "--install");
        if let Some(p) = path {
            run_install_cli(p);
            return Ok(());
        } else {
            eprintln!("Error: Missing game directory path.\nUsage: fsroneclick --install <game_path>");
            std::process::exit(1);
        }
    }

    if args.iter().any(|a| a == "--remove" || a == "--uninstall") {
        let path = args.iter().skip(1).find(|a| *a != "--remove" && *a != "--uninstall");
        if let Some(p) = path {
            run_uninstall_cli(p);
            return Ok(());
        } else {
            eprintln!("Error: Missing game directory path.\nUsage: fsroneclick --remove <game_path>");
            std::process::exit(1);
        }
    }

    if args.len() > 1 && !args[1].starts_with("--") {
        let p = Path::new(&args[1]);
        if p.exists() {
            run_check(&args[1]);
            return Ok(());
        } else {
            eprintln!("Error: Specified path does not exist: {}", args[1]);
            std::process::exit(1);
        }
    }

    // Launch GUI
    run_gui();
    Ok(())
}

fn print_help() {
    println!(
        r#"FSRoneclick - 1-Click Game Enhancement Suite for AMD Radeon & Universal GPUs

USAGE:
    fsroneclick.exe [OPTIONS] [GAME_PATH]

OPTIONS:
    --list-games            Scan and list installed games across Steam, Epic, etc.
    --check <path>          Inspect a game directory or executable (API, DLSS, RenoDX)
    --install <path>        Headless 1-click install (OptiScaler + ReShade + RenoDX)
    --remove <path>         Headless 1-click clean uninstall / rollback
    --check-updates         Check online versions of OptiScaler, ReShade, and databases
    --update-compat         Fetch and cache latest compatibility list from GitHub wiki
    -h, --help              Print this help menu

Without arguments, FSRoneclick launches the interactive graphical interface.
"#
    );
}

fn run_check_updates_cli() {
    let client = match net::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Network error: {e}");
            return;
        }
    };
    let report = fsroneclick::updater::check_all_updates(&client);
    println!("\n{:<30} {:<18} {:<18} {:<8}", "Component", "Installed", "Latest Online", "Status");
    println!("{}", "-".repeat(78));
    for c in &report.components {
        let status_str = if c.up_to_date { "Up to date" } else { "Update Available" };
        let latest = c.latest_version.as_deref().unwrap_or("Unknown");
        println!("{:<30} {:<18} {:<18} {:<8}", c.name, c.installed_version, latest, status_str);
    }
    println!("\nTip: Run with --update-compat to fetch and cache the latest live compatibility database.");
}

fn run_update_compat_cli() {
    println!("Fetching latest compatibility list from OptiScaler GitHub Wiki...");
    let client = match net::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Network error: {e}");
            return;
        }
    };
    match fsroneclick::updater::fetch_and_cache_compatibility_db(&client) {
        Ok(db) => {
            println!("SUCCESS: Successfully updated compatibility database!");
            println!("Total games indexed: {}", db.total_entries);
            println!("Cache location:      {}", fsroneclick::updater::cache_file_path().display());
        }
        Err(e) => {
            eprintln!("Failed to update compatibility database: {e:#}");
        }
    }
}

fn run_list_games() {
    println!("Scanning installed games across Steam, Epic Games, and Standalone (D:\\Games, C:\\Games)...");
    let games = game::scan_all_games();
    println!("Found {} games:\n", games.len());
    println!("{:<30} {:<10} {:<11} {:<14} {:<14} {:<8}", "Game Title", "Store", "API", "OptiScaler", "AMD Redstone", "Status");
    println!("{}", "-".repeat(92));

    for g in games {
        let status = g.check_mod_status();
        let status_str = if status.is_optiscaler_installed {
            "Modded"
        } else {
            "Stock"
        };
        let opti_str = g.compat.opti_status.map_or("Untested", |s| s.as_str());
        let redstone_str = if g.compat.fsr_redstone_supported {
            if g.compat.fsr_redstone_native_fg {
                "Redstone+FG"
            } else {
                "Redstone"
            }
        } else {
            "-"
        };
        println!(
            "{:<30} {:<10} {:<11} {:<14} {:<14} {:<8}",
            truncate_str(&g.title, 28),
            g.store.label(),
            g.api.label(),
            opti_str,
            redstone_str,
            status_str
        );
    }
}

fn run_check(path_str: &str) {
    let p = PathBuf::from(path_str);
    if let Some(game) = game::from_custom_path(&p) {
        let gpu_info = gpu::detect_gpu();
        let client = net::create_http_client().unwrap();
        let catalog = renodx::fetch_mods_catalog(&client);
        let renodx_mod = renodx::match_game(&game.title, &catalog);
        let mod_status = game.check_mod_status();

        println!("=== Game Inspection Report ===");
        println!("Title:             {}", game.title);
        println!("Store:             {}", game.store.label());
        println!("Location:          {}", game.path.display());
        println!(
            "Executable:        {}",
            game.exe_path
                .as_ref()
                .map_or("None found".to_string(), |e| e.display().to_string())
        );
        println!("Graphics API:      {}", game.api.label());
        println!("64-bit Binary:     {}", if game.is_64bit { "Yes" } else { "No (32-bit)" });
        println!(
            "AMD FidelityFX:    {}",
            game.fidelityfx_tier
                .as_ref()
                .map_or("None (Pre-FSR / OptiScaler recommended)".to_string(), |t| t.label().to_string())
        );
        println!(
            "OptiScaler Wiki:   {}",
            game.compat.opti_status.map_or("Untested (Generic Proxy Mode)".to_string(), |s| {
                format!(
                    "{} (Inputs: {}){}",
                    s.as_str(),
                    game.compat.opti_inputs.as_deref().unwrap_or("N/A"),
                    if game.compat.optipatcher_supported { " [OptiPatcher ✨]" } else { "" }
                )
            })
        );
        println!(
            "AMD FSR Redstone:  {}",
            if game.compat.fsr_redstone_supported {
                format!(
                    "Supported (RX 7000 Series Driver 26.6.2+ ready){}",
                    if game.compat.fsr_redstone_native_fg { " [Native FG on RX 9000]" } else { "" }
                )
            } else {
                "Standard FSR 3.1 / 2.x".to_string()
            }
        );
        if let Some(ref notes) = game.compat.opti_notes {
            println!("Community Notes:   {}", notes);
        }
        if let Some(hook) = game.compat.recommended_hook {
            println!("Recommended Hook:  {}", hook);
        }
        println!(
            "Native DLSS:       {}",
            if game.has_native_dlss { "Detected" } else { "Not Present" }
        );
        println!(
            "Capcom RE Engine:  {}",
            if game.is_re_engine { "Yes (REFramework required)" } else { "No" }
        );
        println!(
            "Anti-Cheat:        {}",
            game.anticheat.unwrap_or("Clean / None detected")
        );
        println!(
            "RenoDX HDR Mod:    {}",
            renodx_mod.map_or("None (generic HDR)".to_string(), |m| format!(
                "Available ({})",
                m.filename
            ))
        );
        println!(
            "Mod Status:        {}",
            if mod_status.is_optiscaler_installed { "Installed" } else { "Stock" }
        );

        if let Some(g) = gpu_info {
            println!("\n=== System GPU ===");
            println!("GPU:               {}", g.name);
            println!("Architecture:      {}", g.arch.label());
        }
    } else {
        eprintln!("Could not analyze game at path: {path_str}");
    }
}

fn run_install_cli(path_str: &str) {
    let p = PathBuf::from(path_str);
    let Some(game) = game::from_custom_path(&p) else {
        eprintln!("Error: Target path {} is not a valid game directory.", path_str);
        return;
    };

    let client = match net::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Network error: {e}");
            return;
        }
    };

    let gpu_info = gpu::detect_gpu();
    let gpu_arch = gpu_info.as_ref().map(|g| g.arch.clone()).unwrap_or(gpu::Arch::Unknown);
    let catalog = renodx::fetch_mods_catalog(&client);
    let config = installer::InstallConfig::default();

    println!("Installing enhancements for {}...", game.title);
    let progress_fn = Box::new(|pct: f32, msg: &str| {
        println!("[{:>3.0}%] {}", pct * 100.0, msg);
    });

    match installer::install(&client, &game, &config, &gpu_arch, &catalog, progress_fn) {
        Ok(files) => {
            println!("\nSUCCESS: Staged {} components in {}:", files.len(), game.path.display());
            for f in files {
                println!("  + {}", f);
            }
        }
        Err(e) => {
            eprintln!("\nInstallation failed: {e:#}");
        }
    }
}

fn run_uninstall_cli(path_str: &str) {
    let p = PathBuf::from(path_str);
    let Some(game) = game::from_custom_path(&p) else {
        eprintln!("Error: Target path {} is not a valid game directory.", path_str);
        return;
    };

    println!("Rolling back enhancements for {}...", game.title);
    match installer::uninstall(&game) {
        Ok(files) => {
            println!("\nSUCCESS: Cleanly removed {} files:", files.len());
            for f in files {
                println!("  - {}", f);
            }
            println!("Game restored to stock state.");
        }
        Err(e) => {
            eprintln!("\nRollback failed: {e:#}");
        }
    }
}

fn run_gui() {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([880.0, 560.0])
            .with_title("FSRoneclick - AMD FSR & Frame Generation Manager"),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "FSRoneclick",
        native_options,
        Box::new(|cc| Ok(Box::new(gui::FSRoneclickApp::new(cc)))),
    );
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let mut truncated: String = s.chars().take(max_chars - 3).collect();
        truncated.push_str("...");
        truncated
    } else {
        s.to_string()
    }
}
