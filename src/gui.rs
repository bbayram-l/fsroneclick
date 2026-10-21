//! Native egui graphical interface for FSRoneclick.

use crate::game::{self, GameInfo, ModStatus, Store};
use crate::gpu::{self, GpuInfo};
use crate::installer::{self, InstallConfig};
use crate::net;
use crate::renodx::{self, RenoDxMod};
use crate::updater::ComponentState;
use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

pub struct FSRoneclickApp {
    ctx: egui::Context,
    client: Client,
    gpu: Option<GpuInfo>,
    games: Vec<GameInfo>,
    mod_status_cache: Vec<ModStatus>,
    is_scanning: bool,
    /// Another scan was requested while one was running.
    rescan_pending: bool,
    /// Bumped whenever an install or uninstall finishes, so a scan that started earlier can
    /// tell that its mod statuses are out of date.
    status_generation: u64,
    filtered_games: Vec<usize>,
    selected_game_idx: Option<usize>,
    renodx_catalog: HashMap<String, RenoDxMod>,
    catalog_loaded: bool,
    search_query: String,
    config: InstallConfig,
    custom_path_input: String,

    // Background task channel, shared by every worker thread so no task's messages are dropped
    task_tx: Sender<TaskMessage>,
    task_rx: Receiver<TaskMessage>,
    task_running: bool,
    progress_val: f32,
    progress_msg: String,
    log_messages: Vec<String>,

    // Guide & Updates state
    show_guide_window: bool,
    guide_tab: usize,
    show_updates_window: bool,
    update_report: Option<crate::updater::UpdateCheckReport>,
    is_checking_updates: bool,
    is_updating_compat_db: bool,
    update_status_text: Option<String>,
}

enum TaskMessage {
    Progress(f32, String),
    Log(String),
    Done(Result<String, String>),
    UpdateReport(crate::updater::UpdateCheckReport),
    CompatDbUpdated(Result<usize, String>),
    /// Scanned games, their mod statuses, and the `status_generation` the scan started at.
    GamesScanned(Vec<GameInfo>, Vec<ModStatus>, u64),
    RenoDxCatalog(HashMap<String, RenoDxMod>),
}

impl FSRoneclickApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let client = net::create_http_client().unwrap_or_else(|_| Client::new());
        let gpu = gpu::detect_gpu();
        let (task_tx, task_rx) = channel();

        // Scanning libraries and fetching the RenoDX catalog can take many seconds on a cold disk
        // or slow network, so both run in the background and the window opens immediately.
        let catalog_tx = task_tx.clone();
        let catalog_client = client.clone();
        let catalog_ctx = cc.egui_ctx.clone();
        thread::spawn(move || {
            let catalog = renodx::fetch_mods_catalog(&catalog_client);
            let _ = catalog_tx.send(TaskMessage::RenoDxCatalog(catalog));
            catalog_ctx.request_repaint();
        });

        let mut app = Self {
            ctx: cc.egui_ctx.clone(),
            client,
            gpu,
            games: Vec::new(),
            mod_status_cache: Vec::new(),
            is_scanning: false,
            rescan_pending: false,
            status_generation: 0,
            filtered_games: Vec::new(),
            selected_game_idx: None,
            renodx_catalog: HashMap::new(),
            catalog_loaded: false,
            search_query: String::new(),
            config: InstallConfig::default(),
            custom_path_input: String::new(),
            task_tx,
            task_rx,
            task_running: false,
            progress_val: 0.0,
            progress_msg: "Ready".to_string(),
            log_messages: Vec::new(),
            show_guide_window: false,
            guide_tab: 0,
            show_updates_window: false,
            update_report: None,
            is_checking_updates: false,
            is_updating_compat_db: false,
            update_status_text: None,
        };
        app.refresh_games();
        app
    }

    fn update_filter(&mut self) {
        let q = self.search_query.trim().to_lowercase();
        self.filtered_games = self
            .games
            .iter()
            .enumerate()
            .filter(|(_, g)| q.is_empty() || g.title.to_lowercase().contains(&q))
            .map(|(idx, _)| idx)
            .collect();
    }

    /// Rescans the game libraries on a background thread; the result arrives as `GamesScanned`.
    fn refresh_games(&mut self) {
        if self.is_scanning {
            self.rescan_pending = true;
            return;
        }
        self.is_scanning = true;
        let generation = self.status_generation;
        let tx = self.task_tx.clone();
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            // Cached live compatibility data must be loaded before games are matched against it.
            let _ = crate::updater::load_cached_compatibility_db();
            let games = game::scan_all_games();
            let statuses = games.iter().map(|g| g.check_mod_status()).collect();
            let _ = tx.send(TaskMessage::GamesScanned(games, statuses, generation));
            ctx.request_repaint();
        });
    }

    fn apply_scan(&mut self, scanned: Vec<GameInfo>, statuses: Vec<ModStatus>, generation: u64) {
        let selected_path = self
            .selected_game_idx
            .and_then(|idx| self.games.get(idx))
            .map(|g| g.path.clone());

        // Keep folders the user added by hand; a library scan never finds them.
        let mut games: Vec<GameInfo> = self
            .games
            .iter()
            .filter(|g| {
                g.store == Store::Custom
                    && !scanned
                        .iter()
                        .any(|found| game::path_key(&found.path) == game::path_key(&g.path))
            })
            .cloned()
            .collect();
        let mut all_statuses: Vec<ModStatus> = games.iter().map(|g| g.check_mod_status()).collect();
        games.extend(scanned);
        all_statuses.extend(statuses);

        // An install or uninstall that finished during the scan made its statuses stale.
        if generation != self.status_generation {
            all_statuses = games.iter().map(|g| g.check_mod_status()).collect();
        }

        self.games = games;
        self.mod_status_cache = all_statuses;
        self.update_filter();
        self.selected_game_idx = selected_path
            .and_then(|path| self.games.iter().position(|g| g.path == path))
            .or_else(|| self.filtered_games.first().copied());
    }

    fn run_install(&mut self, game_idx: usize) {
        let game = self.games[game_idx].clone();
        let client = self.client.clone();
        let config = self.config.clone();
        let gpu_arch = self
            .gpu
            .as_ref()
            .map(|g| g.arch.clone())
            .unwrap_or(gpu::Arch::Unknown);
        // Before the startup catalog fetch has finished, the install fetches it itself rather
        // than silently leaving out the game's HDR mod.
        let catalog = self.catalog_loaded.then(|| self.renodx_catalog.clone());

        let tx = self.task_tx.clone();
        self.task_running = true;
        self.progress_val = 0.0;
        self.progress_msg = "Starting install...".to_string();

        thread::spawn(move || {
            let tx_clone = tx.clone();
            let progress_fn = Box::new(move |pct: f32, msg: &str| {
                let _ = tx_clone.send(TaskMessage::Progress(pct, msg.to_string()));
            });

            let catalog = catalog.unwrap_or_else(|| renodx::fetch_mods_catalog(&client));
            match installer::install(&client, &game, &config, &gpu_arch, &catalog, progress_fn) {
                Ok(files) => {
                    let _ = tx.send(TaskMessage::Log(format!(
                        "Installed {} files successfully:\n{}",
                        files.len(),
                        files.join("\n")
                    )));
                    let _ = tx.send(TaskMessage::Done(Ok(format!(
                        "Installed {} components for {}",
                        files.len(),
                        game.title
                    ))));
                }
                Err(e) => {
                    let _ = tx.send(TaskMessage::Done(Err(format!(
                        "Installation failed: {e:#}"
                    ))));
                }
            }
        });
    }

    fn run_uninstall(&mut self, game_idx: usize) {
        let game = self.games[game_idx].clone();
        let tx = self.task_tx.clone();
        self.task_running = true;
        self.progress_val = 0.5;
        self.progress_msg = "Rolling back enhancements...".to_string();

        thread::spawn(move || match installer::uninstall(&game) {
            Ok(files) => {
                let _ = tx.send(TaskMessage::Log(format!(
                    "Cleanly removed {} files:\n{}",
                    files.len(),
                    files.join("\n")
                )));
                let _ = tx.send(TaskMessage::Done(Ok(format!(
                    "Restored {} to stock cleanly",
                    game.title
                ))));
            }
            Err(e) => {
                let _ = tx.send(TaskMessage::Done(Err(format!("Rollback failed: {e:#}"))));
            }
        });
    }

    fn run_check_updates(&mut self) {
        let client = self.client.clone();
        let tx = self.task_tx.clone();
        self.is_checking_updates = true;
        self.update_status_text = Some("Querying GitHub for latest releases...".to_string());

        thread::spawn(move || {
            let report = crate::updater::check_all_updates(&client);
            let _ = tx.send(TaskMessage::UpdateReport(report));
        });
    }

    fn run_update_compat_db(&mut self) {
        let client = self.client.clone();
        let tx = self.task_tx.clone();
        self.is_updating_compat_db = true;
        self.update_status_text =
            Some("Downloading latest OptiScaler compatibility wiki from GitHub...".to_string());

        thread::spawn(
            move || match crate::updater::fetch_and_cache_compatibility_db(&client) {
                Ok(db) => {
                    let _ = tx.send(TaskMessage::CompatDbUpdated(Ok(db.total_entries)));
                }
                Err(e) => {
                    let _ = tx.send(TaskMessage::CompatDbUpdated(Err(e.to_string())));
                }
            },
        );
    }

    fn poll_task(&mut self) {
        let mut messages = Vec::new();
        while let Ok(msg) = self.task_rx.try_recv() {
            messages.push(msg);
        }

        let mut needs_refresh = false;

        for msg in messages {
            match msg {
                TaskMessage::Progress(pct, s) => {
                    // Skipped components would otherwise vanish with the next progress update.
                    if s.starts_with(installer::SKIPPED_PREFIX) {
                        self.log_messages.push(format!("WARNING: {s}"));
                    }
                    self.progress_val = pct;
                    self.progress_msg = s;
                }
                TaskMessage::Log(s) => {
                    self.log_messages.push(s);
                }
                TaskMessage::Done(res) => {
                    self.task_running = false;
                    match res {
                        Ok(msg) => {
                            self.progress_val = 1.0;
                            self.progress_msg = msg.clone();
                            self.log_messages.push(format!("SUCCESS: {msg}"));
                        }
                        Err(err) => {
                            self.progress_msg = err.clone();
                            self.log_messages.push(format!("ERROR: {err}"));
                        }
                    }
                    self.status_generation += 1;
                    self.mod_status_cache =
                        self.games.iter().map(|g| g.check_mod_status()).collect();
                }
                TaskMessage::UpdateReport(report) => {
                    self.is_checking_updates = false;
                    self.update_status_text = Some("All component checks completed.".to_string());
                    self.update_report = Some(report);
                }
                TaskMessage::CompatDbUpdated(res) => {
                    self.is_updating_compat_db = false;
                    match res {
                        Ok(count) => {
                            self.update_status_text = Some(format!(
                                "Successfully updated & cached {count} games from live wiki!"
                            ));
                            needs_refresh = true;
                        }
                        Err(e) => {
                            self.update_status_text = Some(format!("Wiki update failed: {e}"));
                        }
                    }
                }
                TaskMessage::GamesScanned(games, statuses, generation) => {
                    self.is_scanning = false;
                    self.apply_scan(games, statuses, generation);
                    if std::mem::take(&mut self.rescan_pending) {
                        needs_refresh = true;
                    }
                }
                TaskMessage::RenoDxCatalog(catalog) => {
                    self.renodx_catalog = catalog;
                    self.catalog_loaded = true;
                }
            }
        }

        if needs_refresh {
            self.refresh_games();
        }
    }
}

impl eframe::App for FSRoneclickApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_task();

        // Dark modern gaming style
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(225, 230, 240));
        visuals.panel_fill = Color32::from_rgb(18, 20, 26);
        visuals.window_fill = Color32::from_rgb(24, 27, 36);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 32, 42);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(34, 40, 54);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(45, 54, 74);
        visuals.widgets.active.bg_fill = Color32::from_rgb(60, 72, 100);
        ctx.set_visuals(visuals);

        egui::TopBottomPanel::top("header_panel").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("FSRoneclick")
                        .size(22.0)
                        .strong()
                        .color(Color32::from_rgb(70, 180, 255)),
                );
                ui.label(
                    RichText::new("· AMD FSR & Frame Generation Manager")
                        .size(14.0)
                        .color(Color32::GRAY),
                );

                ui.add_space(16.0);
                if ui
                    .button(RichText::new("Documentation").size(12.0).strong())
                    .clicked()
                {
                    self.show_guide_window = !self.show_guide_window;
                }
                if ui
                    .button(RichText::new("Component Status").size(12.0).strong())
                    .clicked()
                {
                    self.show_updates_window = !self.show_updates_window;
                    if self.update_report.is_none() && !self.is_checking_updates {
                        self.run_check_updates();
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ref gpu) = self.gpu {
                        let badge_color = if gpu.arch.supports_fsr4() {
                            Color32::from_rgb(255, 60, 80) // AMD Red
                        } else {
                            Color32::from_rgb(70, 150, 230)
                        };
                        ui.label(RichText::new(&gpu.name).strong().color(badge_color));
                        ui.label(RichText::new("GPU:").color(Color32::GRAY));
                    } else {
                        ui.label(
                            RichText::new("GPU: Universal Direct3D / Vulkan").color(Color32::GRAY),
                        );
                    }
                });
            });

            if let Some(ref gpu) = self.gpu {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Hardware Architecture: {}", gpu.arch.label()))
                        .size(12.0)
                        .color(Color32::from_rgb(140, 200, 140)),
                );
            }
            ui.add_space(8.0);
        });

        // Bottom status & progress bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if self.task_running {
                    ui.spinner();
                }
                ui.label(RichText::new(&self.progress_msg).size(13.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.task_running {
                        ui.add(
                            egui::ProgressBar::new(self.progress_val)
                                .desired_width(200.0)
                                .show_percentage(),
                        );
                    }
                });
            });
            ui.add_space(6.0);
        });

        // Left sidebar: Games List & Search
        egui::SidePanel::left("games_sidebar")
            .min_width(320.0)
            .max_width(360.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Filter installed games...")
                            .desired_width(220.0),
                    );
                    if search_resp.changed() {
                        self.update_filter();
                    }

                    if ui
                        .add_enabled(!self.is_scanning, egui::Button::new("Rescan"))
                        .clicked()
                    {
                        self.refresh_games();
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Custom Folder Adder
                ui.collapsing("Add Custom Game Directory", |ui| {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.custom_path_input);
                        if ui.button("Add").clicked() && !self.custom_path_input.trim().is_empty() {
                            let p = PathBuf::from(self.custom_path_input.trim());
                            if let Some(g) = game::from_custom_path(&p) {
                                let status = g.check_mod_status();
                                self.games.insert(0, g);
                                self.mod_status_cache.insert(0, status);
                                self.custom_path_input.clear();
                                self.update_filter();
                                self.selected_game_idx = Some(0);
                            }
                        }
                    });
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Installed Games ({})", self.filtered_games.len()))
                            .small()
                            .color(Color32::GRAY),
                    );
                    if self.is_scanning {
                        ui.spinner();
                        ui.label(
                            RichText::new("Scanning game libraries...")
                                .small()
                                .color(Color32::GRAY),
                        );
                    }
                });
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("games_sidebar_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for &idx in &self.filtered_games {
                            let g = &self.games[idx];
                            let is_selected = self.selected_game_idx == Some(idx);
                            let mod_status = if idx < self.mod_status_cache.len() {
                                &self.mod_status_cache[idx]
                            } else {
                                continue;
                            };

                            let btn_resp = ui.selectable_label(
                                is_selected,
                                format!("{} [{}]", g.title, g.store.label()),
                            );

                            if btn_resp.clicked() {
                                self.selected_game_idx = Some(idx);
                            }

                            // Small indicator badge under each item
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(g.api.label())
                                        .size(10.0)
                                        .color(Color32::from_rgb(100, 160, 220)),
                                );

                                if g.compat.fsr_redstone_supported {
                                    ui.label(
                                        RichText::new("[Redstone]")
                                            .size(10.0)
                                            .color(Color32::from_rgb(255, 120, 120)),
                                    );
                                } else if g.compat.opti_status.is_some() {
                                    ui.label(
                                        RichText::new("[OptiScaler]")
                                            .size(10.0)
                                            .color(Color32::from_rgb(100, 220, 150)),
                                    );
                                }

                                if mod_status.is_optiscaler_installed {
                                    ui.label(
                                        RichText::new("[Installed]")
                                            .size(10.0)
                                            .color(Color32::from_rgb(80, 220, 120)),
                                    );
                                }
                                if mod_status.installed_renodx_mod.is_some() {
                                    ui.label(
                                        RichText::new("[HDR]")
                                            .size(10.0)
                                            .color(Color32::from_rgb(255, 180, 50)),
                                    );
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
            });

        // Main Center Panel: Selected Game Detail & Setup Actions
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(idx) = self.selected_game_idx else {
                ui.centered_and_justified(|ui| {
                    ui.heading(if self.is_scanning {
                        "Scanning your game libraries..."
                    } else {
                        "Select a game from the sidebar to inspect and enhance"
                    });
                });
                return;
            };

            let game = self.games[idx].clone();
            let mod_status = if idx < self.mod_status_cache.len() {
                self.mod_status_cache[idx].clone()
            } else {
                game.check_mod_status()
            };
            let renodx_mod = renodx::match_game(&game.title, &self.renodx_catalog).cloned();

            egui::ScrollArea::vertical()
                .id_salt("main_center_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new(&game.title).size(24.0).strong());
                        ui.label(
                            RichText::new(format!("[{}]", game.store.label()))
                                .size(14.0)
                                .color(Color32::LIGHT_BLUE),
                        );
                    });

            ui.label(
                RichText::new(format!("Location: {}", game.path.display()))
                    .size(11.0)
                    .color(Color32::GRAY),
            );

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Status Grid
            ui.group(|ui| {
                ui.label(RichText::new("Engine & System Analysis").strong().size(15.0));
                ui.add_space(6.0);
                egui::Grid::new("game_info_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
                    ui.label("Graphics API:");
                    ui.label(RichText::new(game.api.label()).strong().color(Color32::from_rgb(100, 180, 255)));
                    ui.end_row();

                    ui.label("Architecture:");
                    ui.label(if game.is_64bit { "64-bit (x86_64)" } else { "32-bit (x86)" });
                    ui.end_row();

                    ui.label("AMD FidelityFX:");
                    ui.label(if let Some(ref tier) = game.fidelityfx_tier {
                        RichText::new(tier.label()).color(Color32::from_rgb(255, 140, 60))
                    } else {
                        RichText::new("None native (OptiScaler & ReShade upgrade)").color(Color32::GRAY)
                    });
                    ui.end_row();

                    ui.label("Native DLSS / Streamline:");
                    ui.label(if game.has_native_dlss {
                        RichText::new("Detected (OptiScaler converts to FSR 3.1 & FG)").color(Color32::from_rgb(120, 220, 120))
                    } else {
                        RichText::new("Not present (OptiScaler proxy + ReShade mode)").color(Color32::GRAY)
                    });
                    ui.end_row();

                    ui.label("Capcom RE Engine:");
                    ui.label(if game.is_re_engine {
                        RichText::new("Yes (REFramework dinput8.dll will be staged)").color(Color32::from_rgb(255, 180, 80))
                    } else {
                        RichText::new("No").color(Color32::GRAY)
                    });
                    ui.end_row();

                    ui.label("RenoDX HDR Mod:");
                    ui.label(if let Some(ref m) = renodx_mod {
                        RichText::new(format!("Available ({})", m.filename)).color(Color32::from_rgb(80, 220, 180))
                    } else {
                        RichText::new("Generic ReShade HDR mode").color(Color32::GRAY)
                    });
                    ui.end_row();

                    ui.label("Anti-Cheat System:");
                    ui.label(if let Some(ac) = game.anticheat {
                        RichText::new(format!("{ac} detected (avoid in online multiplayer)")).color(Color32::from_rgb(255, 90, 90))
                    } else {
                        RichText::new("None detected (safe for runtime injection)").color(Color32::from_rgb(110, 210, 110))
                    });
                    ui.end_row();

                    ui.label("OptiScaler Profile:");
                    ui.horizontal(|ui| {
                        if let Some(status) = game.compat.opti_status {
                            let (status_text, color) = match status {
                                crate::compatibility::OptiStatus::Working => ("Verified Working", Color32::from_rgb(100, 220, 120)),
                                crate::compatibility::OptiStatus::Partial => ("Partial Support / Caveats", Color32::from_rgb(255, 190, 60)),
                                crate::compatibility::OptiStatus::NotWorking => ("Unsupported / Incompatible", Color32::from_rgb(255, 80, 80)),
                            };
                            ui.label(RichText::new(status_text).strong().color(color));
                            if let Some(ref inputs) = game.compat.opti_inputs {
                                ui.label(RichText::new(format!("[{inputs}]")).size(11.0).color(Color32::LIGHT_BLUE));
                            }
                            if game.compat.optipatcher_supported {
                                ui.label(RichText::new("[OptiPatcher]").size(11.0).color(Color32::from_rgb(255, 215, 0)));
                            }
                        } else {
                            ui.label(RichText::new("Generic Proxy Mode (Untested)").color(Color32::GRAY));
                        }
                    });
                    ui.end_row();

                    ui.label("AMD FSR Redstone:");
                    ui.horizontal(|ui| {
                        if game.compat.fsr_redstone_supported {
                            ui.label(RichText::new("Supported").strong().color(Color32::from_rgb(255, 120, 180)));
                            ui.label(RichText::new("(RX 7000 Series / Driver 26.6.2+)").size(11.0).color(Color32::from_rgb(255, 160, 200)));
                            if game.compat.fsr_redstone_native_fg {
                                ui.label(RichText::new("[Native FG]").size(11.0).color(Color32::from_rgb(255, 215, 0)));
                            }
                        } else {
                            ui.label(RichText::new("Standard FSR 3.1 / 2.x").color(Color32::GRAY));
                        }
                    });
                    ui.end_row();
                });
            });

            ui.add_space(12.0);

            // Enhancement Toggles
            ui.group(|ui| {
                ui.label(RichText::new("Enhancement Options").strong().size(15.0));
                ui.add_space(6.0);

                ui.checkbox(&mut self.config.enable_optiscaler, "OptiScaler (FSR 3.1 & Intel XeSS 1.3)");
                ui.label(RichText::new("   Replaces in-game DLSS 2/3 calls with AMD FSR 3.1 upscaling.").size(11.0).color(Color32::GRAY));

                ui.checkbox(&mut self.config.enable_frame_generation, "AMD FSR 3.1 Frame Generation");
                ui.label(RichText::new("   Enables Nukem9's DLSS-G to FSR 3 Frame Generation bridge.").size(11.0).color(Color32::GRAY));

                let arch = self.gpu.as_ref().map(|g| &g.arch);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.config.enable_fsr4, "FSR 4.1.1 ML Upscaling");
                    let supported_color = Color32::from_rgb(100, 220, 100);
                    let (note, color) = match arch {
                        Some(gpu::Arch::Rdna4) => ("(Native FP8 model on your RDNA 4 GPU)", supported_color),
                        Some(gpu::Arch::Rdna3) => ("(INT8 model on your RDNA 3 GPU)", supported_color),
                        _ => ("(Requires an RDNA 3 or RDNA 4 GPU; FSR 3.1 is used otherwise)", Color32::GRAY),
                    };
                    ui.label(RichText::new(note).color(color).size(11.0));
                });

                ui.checkbox(&mut self.config.enable_reshade, "ReShade (Add-on support build)");
                ui.label(RichText::new("   Chained cleanly alongside OptiScaler as ReShade64.dll.").size(11.0).color(Color32::GRAY));

                if renodx_mod.is_some() {
                    ui.checkbox(&mut self.config.enable_renodx, "RenoDX Game-Specific HDR Mod");
                    ui.label(RichText::new("   Injects studio-grade HDR10/scRGB color grading into the game.").size(11.0).color(Color32::GRAY));
                }

            });

            ui.add_space(10.0);

            // Community Advice & Auto-Tuning Intelligence
            if game.compat.opti_notes.is_some() || game.compat.fsr_redstone_supported || game.compat.recommended_hook.is_some() {
                ui.group(|ui| {
                    ui.label(RichText::new("Community Recommendations & Auto-Tuning").strong().size(14.0).color(Color32::from_rgb(230, 235, 245)));
                    ui.add_space(4.0);
                    if let Some(ref notes) = game.compat.opti_notes {
                        ui.label(RichText::new(format!("Note: {notes}")).size(12.0).color(Color32::from_rgb(210, 215, 225)));
                        ui.add_space(2.0);
                    }
                    ui.horizontal_wrapped(|ui| {
                        if let Some(hook) = game.compat.recommended_hook {
                            ui.label(RichText::new(format!("[Hook: {hook}]")).size(11.0).color(Color32::LIGHT_BLUE));
                        }
                        if game.compat.auto_exposure {
                            ui.label(RichText::new("[AutoExposure: Enabled]").size(11.0).color(Color32::LIGHT_GREEN));
                        }
                        if game.compat.non_linear_color_space {
                            ui.label(RichText::new("[Color Space: Non-Linear]").size(11.0).color(Color32::LIGHT_GREEN));
                        }
                        if game.compat.disable_spoofing {
                            ui.label(RichText::new("[Spoofing: Disabled]").size(11.0).color(Color32::LIGHT_YELLOW));
                        }
                        if game.compat.fsr2_pattern {
                            ui.label(RichText::new("[FSR2 Pattern: Active]").size(11.0).color(Color32::LIGHT_GREEN));
                        }
                        if game.compat.force_dx12_param {
                            ui.label(RichText::new("[-dx12 flag recommended]").size(11.0).color(Color32::from_rgb(255, 140, 60)));
                        }
                        if game.compat.fsr_redstone_supported {
                            ui.label(RichText::new("[FSR Redstone: Compatible]").size(11.0).color(Color32::from_rgb(255, 120, 180)));
                        }
                    });
                });
                ui.add_space(8.0);
            }

            ui.add_space(6.0);

            // Action Buttons
            ui.horizontal(|ui| {
                if !mod_status.is_optiscaler_installed {
                    let install_btn = egui::Button::new(
                        RichText::new("Install Enhancements")
                            .size(15.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(16, 120, 70))
                    .min_size(Vec2::new(240.0, 40.0))
                    .rounding(Rounding::same(4.0));

                    if ui.add_enabled(!self.task_running, install_btn).clicked() {
                        self.run_install(idx);
                    }
                } else {
                    let update_btn = egui::Button::new(
                        RichText::new("Update Configuration")
                            .size(14.0)
                            .strong(),
                    )
                    .min_size(Vec2::new(180.0, 38.0))
                    .rounding(Rounding::same(4.0));

                    if ui.add_enabled(!self.task_running, update_btn).clicked() {
                        self.run_install(idx);
                    }

                    ui.add_space(10.0);

                    let uninstall_btn = egui::Button::new(
                        RichText::new("Uninstall Enhancements")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(255, 120, 120)),
                    )
                    .fill(Color32::from_rgb(60, 24, 28))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(140, 40, 50)))
                    .min_size(Vec2::new(180.0, 38.0))
                    .rounding(Rounding::same(4.0));

                    if ui.add_enabled(!self.task_running, uninstall_btn).clicked() {
                        self.run_uninstall(idx);
                    }
                }
            });

            ui.add_space(12.0);

            // Log output / Manifest summary
            if !self.log_messages.is_empty() {
                ui.add_space(8.0);
                ui.collapsing("Execution Log", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Copy Output").clicked() {
                            let all_logs = self.log_messages.join("\n");
                            ui.output_mut(|o| o.copied_text = all_logs);
                        }
                        if ui.button("Clear").clicked() {
                            self.log_messages.clear();
                        }
                    });
                    ui.add_space(4.0);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(14, 16, 22))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 64)))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("action_log_inner_scroll")
                                .max_height(180.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for line in &self.log_messages {
                                        for subline in line.lines() {
                                            let (color, text) = if subline.starts_with("SUCCESS:") || subline.starts_with("Installed") {
                                                (Color32::from_rgb(100, 220, 140), subline)
                                            } else if subline.starts_with("ERROR:") || subline.starts_with("Failed") {
                                                (Color32::from_rgb(255, 100, 100), subline)
                                            } else if subline.starts_with("WARNING:") {
                                                (Color32::from_rgb(255, 190, 60), subline)
                                            } else if subline.starts_with('+') || subline.starts_with('-') {
                                                (Color32::from_rgb(140, 200, 255), subline)
                                            } else {
                                                (Color32::from_rgb(210, 215, 225), subline)
                                            };
                                            ui.label(RichText::new(text).monospace().size(12.0).color(color));
                                        }
                                    }
                                });
                        });
                });
            }
            ui.add_space(16.0);
        });
    });

        if self.show_guide_window {
            self.render_guide_window(ctx);
        }
        if self.show_updates_window {
            self.render_updates_window(ctx);
        }

        if self.task_running
            || self.is_checking_updates
            || self.is_updating_compat_db
            || self.is_scanning
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}

impl FSRoneclickApp {
    fn render_guide_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_guide_window;
        egui::Window::new("Documentation & Tuning Guide")
            .open(&mut open)
            .min_width(640.0)
            .min_height(480.0)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.guide_tab, 0, "Quickstart");
                    ui.selectable_value(&mut self.guide_tab, 1, "Radeon RX 7000 / RDNA 3");
                    ui.selectable_value(&mut self.guide_tab, 2, "In-Game Keybindings");
                    ui.selectable_value(&mut self.guide_tab, 3, "Anti-Cheat Safety");
                    ui.selectable_value(&mut self.guide_tab, 4, "Credits & Licenses");
                });
                ui.separator();
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .id_salt("guide_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    match self.guide_tab {
                        0 => {
                            ui.heading("Quickstart: 1-Click Installation Workflow");
                            ui.add_space(6.0);
                            ui.label(RichText::new("1. Select Your Target Game").strong());
                            ui.label("• Select a detected game from the left sidebar.\n• For custom or non-store installations, open 'Add Custom Game Directory' and specify the binary path (e.g. D:\\Games\\Far Cry 5\\bin).");
                            ui.add_space(6.0);
                            ui.label(RichText::new("2. Review Engine & System Analysis").strong());
                            ui.label("• Verify the graphics API (DirectX 11, DirectX 12, or Vulkan).\n• Review OptiScaler Community Compatibility profile and AMD FSR Redstone tier.\n• FSRoneclick automatically configures recommended engine flags (AutoExposure, Non-Linear Color Space, Custom Proxy Hooks) matching verified upstream workarounds.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("3. Configure Enhancement Options").strong());
                            ui.label("• OptiScaler: Intercepts DLSS and XeSS API calls, routing them into AMD FSR 3.1.\n• FSR Frame Generation: Replaces DLSS-G calls with AMD FSR 3 frame generation.\n• FSR 4.1.1 INT8: Employs RDNA 3 WMMA tensor instructions for machine-learning super-resolution.\n• ReShade + RenoDX: Loads HDR10/scRGB color grading and Contrast Adaptive Sharpening (CAS).");
                            ui.add_space(6.0);
                            ui.label(RichText::new("4. Apply Enhancements").strong());
                            ui.label("• Click 'Install Enhancements'. All proxy libraries, Agility SDK runtimes, ASI patchers, and configuration files are deployed instantly.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("5. In-Game Configuration").strong());
                            ui.label("• Launch your game normally.\n• In Video / Display options, enable 'DLSS' (now powered by FSR 3.1 / FSR 4), or XeSS / FSR if DLSS isn't offered.\n• Enable 'DLSS Frame Generation' if the game has it (now running AMD FSR Frame Generation).\n• Press 'Insert' to open the OptiScaler overlay, and 'Home' to access ReShade/RenoDX.");
                        }
                        1 => {
                            ui.heading("Architecture Tuning: AMD Radeon RX 7000 / RDNA 3");
                            ui.add_space(6.0);
                            ui.label(RichText::new("AMD FSR Redstone (FSR 4) on RX 7000 Series").strong().color(Color32::from_rgb(255, 120, 180)));
                            ui.label("• AMD FSR 'Redstone' upscaling is supported on AMD Radeon RX 7000 Series GPUs using driver Adrenalin 26.6.2 or newer.\n• RDNA 3 WMMA matrix units execute INT8 tensor operations with zero emulation penalty.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Cutscene Flickering or Black Screen Resolution").strong().color(Color32::LIGHT_GREEN));
                            ui.label("• If cutscenes or menus flicker under FSR 4, open the OptiScaler overlay ('Insert') and set 'FsrNonLinearColorSpace = true' or enable 'sRGB Input'.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Over-Bright / Blown-Out Exposure").strong().color(Color32::LIGHT_GREEN));
                            ui.label("• If lighting appears over-exposed in dark areas, enable 'AutoExposure = true' in OptiScaler settings.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Unreal Engine DX11 Foliage Shimmering").strong().color(Color32::from_rgb(255, 180, 80)));
                            ui.label("• For Unreal Engine DX11 games experiencing motion vector jitter, add 'r.NGX.DLSS.DilateMotionVectors=0' to Engine.ini under '[SystemSettings]'.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Alternative Proxy Hook Types").strong().color(Color32::LIGHT_BLUE));
                            ui.label("• Most titles hook cleanly through dxgi.dll. For games with custom DXGI wrappers, FSRoneclick can deploy as winmm.dll, d3d12.dll, or version.dll.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Latency Reduction: Reflex to Anti-Lag 2").strong().color(Color32::LIGHT_YELLOW));
                            ui.label("• FakeNVAPI dynamically routes NVIDIA Reflex synchronization into AMD Anti-Lag 2 for low input latency.");
                        }
                        2 => {
                            ui.heading("In-Game Keybindings");
                            ui.add_space(6.0);
                            ui.group(|ui| {
                                egui::Grid::new("hotkeys_grid").num_columns(2).spacing([24.0, 10.0]).show(ui, |ui| {
                                    ui.label(RichText::new("Insert").strong().color(Color32::from_rgb(100, 200, 255)));
                                    ui.label("Toggle OptiScaler In-Game Menu (Upscaler backend, Sharpness, Frame Gen, Motion Vectors)");
                                    ui.end_row();

                                    ui.label(RichText::new("End").strong().color(Color32::from_rgb(100, 200, 255)));
                                    ui.label("Turn OptiScaler Frame Generation on / off");
                                    ui.end_row();

                                    ui.label(RichText::new("Page Up / Page Down").strong().color(Color32::from_rgb(100, 200, 255)));
                                    ui.label("Show the OptiScaler FPS overlay / cycle its style");
                                    ui.end_row();

                                    ui.label(RichText::new("Home").strong().color(Color32::from_rgb(255, 180, 80)));
                                    ui.label("Toggle ReShade Menu (Shader effects, RenoDX HDR calibration, Peak Nits, White Point)");
                                    ui.end_row();
                                });
                            });
                        }
                        3 => {
                            ui.heading("Anti-Cheat Safety Guidelines");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Single-Player & Offline Games:").strong().color(Color32::LIGHT_GREEN));
                            ui.label("• FSRoneclick is completely safe for single-player campaigns and offline play.");
                            ui.add_space(6.0);
                            ui.label(RichText::new("Multiplayer / Kernel Anti-Cheat Protected Titles:").strong().color(Color32::from_rgb(255, 90, 90)));
                            ui.label("• Titles protected by kernel-level anti-cheat (Easy Anti-Cheat, BattlEye, Vanguard, Ricochet) will block DLL proxy hooks (such as dxgi.dll) and may flag accounts.\n• When FSRoneclick detects an active anti-cheat system, it flags a warning badge in the System Analysis panel.\n• Always launch games in offline or single-player mode when using DLL mod proxies.");
                        }
                        4 => {
                            ui.heading("Credits & Third-Party Licenses");
                            ui.add_space(6.0);
                            ui.label("FSRoneclick automates, coordinates, and stages other creators' exceptional open-source work.");
                            ui.add_space(8.0);

                            ui.group(|ui| {
                                ui.label(RichText::new("Inspiration & Reference").strong().color(Color32::from_rgb(100, 200, 255)));
                                ui.label("• Faisal Al-Kindi (faisalkindi) — DLSS5oneclick (https://github.com/faisalkindi/DLSS5oneclick)\n  Inspiration for the UI layout, library scanning approach, and 1-click automation workflow.");
                            });
                            ui.add_space(8.0);

                            ui.group(|ui| {
                                ui.label(RichText::new("Core Runtimes & Components").strong().color(Color32::from_rgb(255, 180, 80)));
                                ui.label("• OptiScaler Team & cdozdil — OptiScaler (GPL-3.0 / MIT)\n  DLSS/XeSS to AMD FSR 3.1 & FSR 4 translation layer and frame generation runtime.");
                                ui.add_space(4.0);
                                ui.label("• AMD (GPUOpen) — FidelityFX SDK & FSR Redstone (MIT)\n  Temporal upscaling, frame generation, and RDNA 3 WMMA INT8 tensor acceleration.");
                                ui.add_space(4.0);
                                ui.label("• crosire — ReShade & reshade-shaders (BSD 3-Clause)\n  Generic post-processing injection framework and shader runtime.");
                                ui.add_space(4.0);
                                ui.label("• clshortfuse & RenoDX Community — RenoDX (MIT)\n  True native HDR reconstruction and tonemapping add-on.");
                                ui.add_space(4.0);
                                ui.label("• OptiScaler Team — OptiPatcher (MIT)\n  In-memory signature patches neutralizing proprietary RTX vendor locks to prevent crashes.");
                                ui.add_space(4.0);
                                ui.label("• praydog — REFramework (MIT)\n  Modding framework staged for Capcom RE Engine games.");
                                ui.add_space(4.0);
                                ui.label("• OopyDoopy — CAS-for-ReShade (MIT, based on AMD FidelityFX CAS)\n  Contrast Adaptive Sharpening shader for ReShade.");
                                ui.add_space(4.0);
                                ui.label("• Emil Ernerfeldt (emilk) — egui & eframe (MIT / Apache-2.0)\n  Immediate-mode GUI toolkit for Rust, including its bundled Hack, Ubuntu and Noto Emoji fonts.");
                            });
                        }
                        _ => {}
                    }
                });
            });
        self.show_guide_window = open;
    }

    fn render_updates_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_updates_window;
        egui::Window::new("Component Status & Updates")
            .open(&mut open)
            .min_width(680.0)
            .min_height(420.0)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.is_checking_updates, egui::Button::new("Check Upstream Versions")).clicked() {
                        self.run_check_updates();
                    }
                    if ui.add_enabled(!self.is_updating_compat_db, egui::Button::new("Sync Compatibility Wiki DB")).clicked() {
                        self.run_update_compat_db();
                    }
                });

                if let Some(ref status) = self.update_status_text {
                    ui.add_space(4.0);
                    ui.label(RichText::new(status).size(12.0).color(Color32::from_rgb(100, 200, 255)));
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(RichText::new("Package & Library Status").strong().size(15.0));
                ui.add_space(6.0);

                egui::ScrollArea::vertical()
                    .id_salt("updates_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    if let Some(ref report) = self.update_report {
                        egui::Grid::new("updates_grid").num_columns(4).spacing([24.0, 10.0]).show(ui, |ui| {
                            ui.label(RichText::new("Component").strong());
                            ui.label(RichText::new("Installed").strong());
                            ui.label(RichText::new("Upstream Release").strong());
                            ui.label(RichText::new("Status").strong());
                            ui.end_row();

                            for comp in &report.components {
                                ui.label(RichText::new(&comp.name).strong());
                                ui.label(&comp.installed_version);
                                ui.label(comp.latest_version.as_deref().unwrap_or("—"));
                                let (status, color) = match comp.state {
                                    ComponentState::UpToDate => ("Current", Color32::from_rgb(100, 220, 120)),
                                    ComponentState::UpdateAvailable => ("Update Available", Color32::from_rgb(255, 180, 50)),
                                    ComponentState::NotChecked => ("Not checked", Color32::GRAY),
                                };
                                ui.label(RichText::new(status).color(color)).on_hover_text(&comp.details);
                                ui.end_row();
                            }
                        });
                    } else {
                        ui.label("Click 'Check Upstream Versions' to query GitHub releases for latest components.");
                    }
                });
            });
        self.show_updates_window = open;
    }
}
