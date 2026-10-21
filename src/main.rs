use fsroneclick::game;
use fsroneclick::gpu;
use fsroneclick::gui;
use fsroneclick::installer;
use fsroneclick::net;
use fsroneclick::renodx;
use fsroneclick::updater::ComponentState;

use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};

/// `println!` that drops output nobody can receive (for example after the reader of
/// `fsroneclick --list-games | head` exits) instead of panicking, so the command still finishes
/// its work and reports its real exit code.
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

/// `eprintln!` counterpart of [`outln!`].
macro_rules! errln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

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
            errln!("Error: Missing game directory path.\nUsage: fsroneclick --check <game_path>");
            std::process::exit(1);
        }
    }

    if args.iter().any(|a| a == "--install") {
        let path = args.iter().skip(1).find(|a| *a != "--install");
        if let Some(p) = path {
            run_install_cli(p);
            return Ok(());
        } else {
            errln!("Error: Missing game directory path.\nUsage: fsroneclick --install <game_path>");
            std::process::exit(1);
        }
    }

    if args.iter().any(|a| a == "--remove" || a == "--uninstall") {
        let path = args
            .iter()
            .skip(1)
            .find(|a| *a != "--remove" && *a != "--uninstall");
        if let Some(p) = path {
            run_uninstall_cli(p);
            return Ok(());
        } else {
            errln!("Error: Missing game directory path.\nUsage: fsroneclick --remove <game_path>");
            std::process::exit(1);
        }
    }

    if args.len() > 1 && !args[1].starts_with("--") {
        let p = Path::new(&args[1]);
        if p.exists() {
            run_check(&args[1]);
            return Ok(());
        } else {
            errln!("Error: Specified path does not exist: {}", args[1]);
            std::process::exit(1);
        }
    }

    // Launch GUI
    run_gui();
    Ok(())
}

fn print_help() {
    outln!(
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
            errln!("Network error: {e}");
            std::process::exit(1);
        }
    };
    let report = fsroneclick::updater::check_all_updates(&client);
    outln!(
        "\n{:<30} {:<18} {:<18} {:<8}",
        "Component",
        "Installed",
        "Latest Online",
        "Status"
    );
    outln!("{}", "-".repeat(78));
    for c in &report.components {
        let status_str = match c.state {
            ComponentState::UpToDate => "Up to date",
            ComponentState::UpdateAvailable => "Update Available",
            ComponentState::NotChecked => "Not checked",
        };
        let latest = c.latest_version.as_deref().unwrap_or("-");
        outln!(
            "{:<30} {:<18} {:<18} {:<8}",
            c.name,
            c.installed_version,
            latest,
            status_str
        );
    }
    outln!("\nTip: Run with --update-compat to fetch and cache the latest live compatibility database.");
}

fn run_update_compat_cli() {
    outln!("Fetching latest compatibility list from OptiScaler GitHub Wiki...");
    let client = match net::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            errln!("Network error: {e}");
            std::process::exit(1);
        }
    };
    match fsroneclick::updater::fetch_and_cache_compatibility_db(&client) {
        Ok(db) => {
            outln!("SUCCESS: Successfully updated compatibility database!");
            outln!("Total games indexed: {}", db.total_entries);
            outln!(
                "Cache location:      {}",
                fsroneclick::updater::cache_file_path().display()
            );
        }
        Err(e) => {
            errln!("Failed to update compatibility database: {e:#}");
            std::process::exit(1);
        }
    }
}

fn run_list_games() {
    outln!("Scanning installed games across Steam, Epic Games, and Standalone (D:\\Games, C:\\Games)...");
    let games = game::scan_all_games();
    outln!("Found {} games:\n", games.len());
    outln!(
        "{:<30} {:<10} {:<11} {:<14} {:<14} {:<8}",
        "Game Title",
        "Store",
        "API",
        "OptiScaler",
        "AMD Redstone",
        "Status"
    );
    outln!("{}", "-".repeat(92));

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
        outln!(
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
        let catalog = net::create_http_client()
            .map(|client| renodx::fetch_mods_catalog(&client))
            .unwrap_or_default();
        let renodx_mod = renodx::match_game(&game.title, &catalog);
        let mod_status = game.check_mod_status();

        outln!("=== Game Inspection Report ===");
        outln!("Title:             {}", game.title);
        outln!("Store:             {}", game.store.label());
        outln!("Location:          {}", game.path.display());
        outln!(
            "Executable:        {}",
            game.exe_path
                .as_ref()
                .map_or("None found".to_string(), |e| e.display().to_string())
        );
        outln!("Graphics API:      {}", game.api.label());
        outln!(
            "64-bit Binary:     {}",
            if game.is_64bit { "Yes" } else { "No (32-bit)" }
        );
        outln!(
            "AMD FidelityFX:    {}",
            game.fidelityfx_tier
                .as_ref()
                .map_or("None (Pre-FSR / OptiScaler recommended)".to_string(), |t| t
                    .label()
                    .to_string())
        );
        outln!(
            "OptiScaler Wiki:   {}",
            game.compat
                .opti_status
                .map_or("Untested (Generic Proxy Mode)".to_string(), |s| {
                    format!(
                        "{} (Inputs: {}){}",
                        s.as_str(),
                        game.compat.opti_inputs.as_deref().unwrap_or("N/A"),
                        if game.compat.optipatcher_supported {
                            " [OptiPatcher ✨]"
                        } else {
                            ""
                        }
                    )
                })
        );
        outln!(
            "AMD FSR Redstone:  {}",
            if game.compat.fsr_redstone_supported {
                format!(
                    "Supported (RX 7000 Series Driver 26.6.2+ ready){}",
                    if game.compat.fsr_redstone_native_fg {
                        " [Native FG on RX 9000]"
                    } else {
                        ""
                    }
                )
            } else {
                "Standard FSR 3.1 / 2.x".to_string()
            }
        );
        if let Some(ref notes) = game.compat.opti_notes {
            outln!("Community Notes:   {}", notes);
        }
        if let Some(hook) = game.compat.recommended_hook {
            outln!("Recommended Hook:  {}", hook);
        }
        outln!(
            "Native DLSS:       {}",
            if game.has_native_dlss {
                "Detected"
            } else {
                "Not Present"
            }
        );
        outln!(
            "Capcom RE Engine:  {}",
            if game.is_re_engine {
                "Yes (REFramework required)"
            } else {
                "No"
            }
        );
        outln!(
            "Anti-Cheat:        {}",
            game.anticheat.unwrap_or("Clean / None detected")
        );
        outln!(
            "RenoDX HDR Mod:    {}",
            renodx_mod.map_or("None (generic HDR)".to_string(), |m| format!(
                "Available ({})",
                m.filename
            ))
        );
        outln!(
            "Mod Status:        {}",
            if mod_status.is_optiscaler_installed {
                "Installed"
            } else {
                "Stock"
            }
        );

        if let Some(g) = gpu_info {
            outln!("\n=== System GPU ===");
            outln!("GPU:               {}", g.name);
            outln!("Architecture:      {}", g.arch.label());
        }
    } else {
        errln!("Could not analyze game at path: {path_str}");
        std::process::exit(1);
    }
}

fn run_install_cli(path_str: &str) {
    let p = PathBuf::from(path_str);
    let Some(game) = game::from_custom_path(&p) else {
        errln!(
            "Error: Target path {} is not a valid game directory.",
            path_str
        );
        std::process::exit(1);
    };

    let client = match net::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            errln!("Network error: {e}");
            std::process::exit(1);
        }
    };

    let gpu_info = gpu::detect_gpu();
    let gpu_arch = gpu_info
        .as_ref()
        .map(|g| g.arch.clone())
        .unwrap_or(gpu::Arch::Unknown);
    let catalog = renodx::fetch_mods_catalog(&client);
    let config = installer::InstallConfig::default();

    if let Some(anticheat) = game.anticheat {
        errln!(
            "Warning: {anticheat} detected. Only use these mods offline or in single-player; anti-cheat can flag injected DLLs and ban accounts."
        );
    }
    outln!("Installing enhancements for {}...", game.title);
    let progress_fn = Box::new(|pct: f32, msg: &str| {
        outln!("[{:>3.0}%] {}", pct * 100.0, msg);
    });

    match installer::install(&client, &game, &config, &gpu_arch, &catalog, progress_fn) {
        Ok(files) => {
            outln!(
                "\nSUCCESS: Staged {} components in {}:",
                files.len(),
                game.path.display()
            );
            for f in files {
                outln!("  + {}", f);
            }
        }
        Err(e) => {
            errln!("\nInstallation failed: {e:#}");
            std::process::exit(1);
        }
    }
}

fn run_uninstall_cli(path_str: &str) {
    let p = PathBuf::from(path_str);
    let Some(game) = game::from_custom_path(&p) else {
        errln!(
            "Error: Target path {} is not a valid game directory.",
            path_str
        );
        std::process::exit(1);
    };

    outln!("Rolling back enhancements for {}...", game.title);
    match installer::uninstall(&game) {
        Ok(files) => {
            outln!("\nSUCCESS: Cleanly removed {} files:", files.len());
            for f in files {
                outln!("  - {}", f);
            }
            outln!("Game restored to stock state.");
        }
        Err(e) => {
            errln!("\nRollback failed: {e:#}");
            std::process::exit(1);
        }
    }
}

/// A double-click in Explorer makes Windows create a console window just for this process.
/// Release it so only the GUI window remains. When started from a terminal, the terminal shares
/// the console and nothing changes, so CLI output, piping, and exit codes keep working.
#[cfg(windows)]
fn release_console_if_owned() {
    use windows_sys::Win32::System::Console::{
        FreeConsole, GetConsoleProcessList, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
        STD_OUTPUT_HANDLE,
    };

    let mut process_ids = [0u32; 2];
    // SAFETY: `process_ids` has room for the number of entries passed as its length.
    let attached =
        unsafe { GetConsoleProcessList(process_ids.as_mut_ptr(), process_ids.len() as u32) };
    if attached == 1 {
        // SAFETY: FreeConsole has no preconditions; it only detaches this process from its console.
        unsafe { FreeConsole() };
        // The old console handle values would otherwise be reused for unrelated files, and a
        // later print would write into them; with null handles, output is discarded instead.
        for handle in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: setting a standard handle to null is always allowed.
            unsafe { SetStdHandle(handle, std::ptr::null_mut()) };
        }
    }
}

#[cfg(not(windows))]
fn release_console_if_owned() {}

/// Reports an error with a message box, since a GUI launch may have no console to print to.
#[cfg(windows)]
fn show_error_dialog(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let (text, caption) = (wide(message), wide("FSRoneclick"));
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        )
    };
}

#[cfg(not(windows))]
fn show_error_dialog(_message: &str) {}

fn run_gui() {
    release_console_if_owned();
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([880.0, 560.0])
            .with_title("FSRoneclick - AMD FSR & Frame Generation Manager"),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "FSRoneclick",
        native_options,
        Box::new(|cc| Ok(Box::new(gui::FSRoneclickApp::new(cc)))),
    ) {
        let message = format!(
            "FSRoneclick could not open its window: {e}\n\n\
             The command-line interface still works; run `fsroneclick.exe --help` in a terminal."
        );
        errln!("{message}");
        show_error_dialog(&message);
    }
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
