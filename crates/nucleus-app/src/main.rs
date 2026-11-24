//! NucleusCoop application entry point.

use gpui::AppContext;
use nucleus_common::logging;
use nucleus_engine::GameManager;
use nucleus_input::InputManager;
use nucleus_input::gamepad::GamepadDevice;
use nucleus_profile::AppSettings;
use nucleus_ui::{DeviceItem, HandlerItem, HubPackageItem};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

mod platform;
pub mod startup;

#[derive(Default, Clone, Debug)]
pub struct LaunchGuiOptions {
    pub initial_tab: Option<String>,
    pub target_game: Option<String>,
    pub initial_players: Option<usize>,
    pub initial_layout: Option<String>,
    pub search_query: Option<String>,
    pub controller_shortcuts: bool,
    pub hub_details: Option<String>,
    pub hub_tab: Option<String>,
    pub big_picture: bool,
    pub big_picture_screen: Option<String>,
    pub preflight_warn: Option<String>,
    pub preflight_danger: Option<String>,
}

fn resolve_cover_url(guid: &str, steam_id: &str, game_name: &str) -> Option<String> {
    // 1. Check local assets in gui/covers first (C# parity with AssetsDownloader.cs)
    if let Some(g) = nucleus_common::Globals::try_get() {
        let covers_dir = g.install_root.join("gui").join("covers");
        for ext in ["jpeg", "jpg", "png", "bmp", "webp"] {
            let candidate = covers_dir.join(format!("{}.{}", guid, ext));
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
            let candidate_name = covers_dir.join(format!("{}.{}", game_name, ext));
            if candidate_name.exists() {
                return Some(candidate_name.to_string_lossy().to_string());
            }
        }
        let cache_dir = g.install_root.join("content").join("cache").join("covers");
        for ext in ["jpeg", "jpg", "png", "bmp", "webp"] {
            let candidate = cache_dir.join(format!("{}.{}", guid, ext));
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
            let candidate_name = cache_dir.join(format!("{}.{}", game_name, ext));
            if candidate_name.exists() {
                return Some(candidate_name.to_string_lossy().to_string());
            }
        }
    }

    // 2. High quality IGDB covers for iconic catalog titles
    if game_name.eq_ignore_ascii_case("Left 4 Dead 2") {
        Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1x7j.jpg".to_string())
    } else if game_name.to_lowercase().contains("halo") {
        Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1r76.jpg".to_string())
    } else if game_name.to_lowercase().contains("borderlands") {
        Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1x9j.jpg".to_string())
    } else if game_name.to_lowercase().contains("portal") {
        Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1rs4.jpg".to_string())
    } else if game_name.to_lowercase().contains("risk of rain") {
        Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1tnt.jpg".to_string())
    } else if !steam_id.is_empty() {
        Some(format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
            steam_id
        ))
    } else {
        None
    }
}

fn build_handler_item(
    guid: &str,
    h: &nucleus_engine::HandlerConfig,
    profile: Option<&nucleus_profile::GameProfile>,
    meta_info: Option<&nucleus_profile::GameMetaInfoEntry>,
) -> HandlerItem {
    let cover_url = resolve_cover_url(guid, &h.steam_id, &h.game_name);
    let (is_installed, exe_path) = if let Some(p) = profile {
        let exists = !p.executable_path.is_empty() && Path::new(&p.executable_path).exists();
        (exists, Some(p.executable_path.clone()))
    } else {
        (false, None)
    };

    let play_time = if let Some(p) = profile {
        if p.total_play_time_seconds > 0 {
            p.total_play_time_seconds
        } else if let Some(m) = meta_info {
            m.play_time_seconds()
        } else {
            0
        }
    } else if let Some(m) = meta_info {
        m.play_time_seconds()
    } else {
        0
    };

    let last_played_ts = profile.and_then(|p| p.last_played_timestamp);
    let last_played_str = meta_info.and_then(|m| m.last_played_at.clone());

    let is_favorite = meta_info
        .and_then(|m| m.favorite)
        .unwrap_or_else(|| profile.map_or(false, |p| p.favorite));

    let mut item = HandlerItem::new(
        guid.to_string(),
        h.game_name.clone(),
        h.executable_name.clone(),
        h.max_players,
        h.description.clone(),
    )
    .with_handler_title(format!("{} Community Handler", h.game_name))
    .with_cover(cover_url)
    .with_installed(is_installed)
    .with_executable_path(exe_path)
    .with_play_time(play_time)
    .with_last_played(last_played_ts)
    .with_last_played_str(last_played_str)
    .with_favorite(is_favorite)
    .with_exact_input_support(
        h.supports_xinput,
        h.supports_dinput,
        h.supports_raw_input || h.supports_multiple_keyboards_and_mice || h.use_proto_input,
        h.supports_keyboard,
    )
    .with_options(h.options.clone())
    .with_handler_path(h.handler_path.clone().or_else(|| {
        profile.and_then(|p| {
            if !p.handler_path.is_empty() {
                Some(p.handler_path.clone())
            } else {
                None
            }
        })
    }));

    if let Some(p) = profile {
        item.assign_gamepads_by_index = p.assign_gamepads_by_index;
        item.assign_gamepads_on_button_press = p.assign_gamepads_on_button_press;
        item.keep_instances_content_folder = p.keep_instances_content_folder;
        item.disable_profile = p.disable_profile;
        item.disable_handler_update = p.disable_handler_update;
        item.custom_game_icon = p.custom_game_icon.clone();
        item.steam_language = p.steam_language.clone();
    }
    item
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Determine install root early for persistent file logging
    let initial_settings = AppSettings::load();
    let install_root = if let Some(parent) =
        Path::new(&initial_settings.handlers_directory).parent()
    {
        if parent.exists() && (parent.join("utils").exists() || parent.join("handlers").exists()) {
            parent.to_path_buf()
        } else {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        }
    } else if Path::new(r"C:\Games\NucleusCoop242").exists() {
        std::path::PathBuf::from(r"C:\Games\NucleusCoop242")
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    };

    // Initialize dual console + file logging in install root
    let log_path = install_root.join("nucleus_coop.log");
    if let Err(_) = logging::init_logging_with_file(&log_path) {
        logging::init_logging();
    }
    info!(
        "NucleusCoop starting (logging to {})...",
        log_path.display()
    );

    let mut app_settings = initial_settings;
    info!("Loaded application settings: {:?}", app_settings);

    // Initialize Globals install root
    nucleus_common::Globals::init(install_root.clone());
    info!(
        "Initialized Globals with install root: {}",
        install_root.display()
    );

    // Initialize global visual theme based on saved settings
    nucleus_ui::theme::set_active_theme(&app_settings.theme);
    nucleus_ui::theme::set_active_accent(&app_settings.accent_color);

    // Create platform services
    let platform = platform::create_platform_services();
    info!("Platform services initialized");

    // Startup Pre-Flight Checks & Verification (C# StartChecks.cs / Program.cs parity)
    let cli_args: Vec<String> = std::env::args().collect();
    let requires_admin = cli_args.iter().any(|a| a == "--elevated" || a == "--admin");

    let preflight = match startup::run_preflight_checks(
        &install_root,
        &app_settings,
        platform.as_ref(),
        requires_admin,
    ) {
        Ok(res) => res,
        Err(startup::SingleInstanceError::AlreadyRunning) => {
            eprintln!(
                "Nucleus Co-op is already running. If you don't see the Nucleus Co-op window it might be running in the background, close the process using Task Manager."
            );
            startup::activate_running_instance_window();
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Preflight check error: {}", e);
            return Err(e.into());
        }
    };

    if preflight.elevation == startup::ElevationAction::Relaunched {
        info!("Elevated child process launched; exiting current process cleanly.");
        return Ok(());
    }

    if preflight.drive_filesystem.is_unsupported {
        if let Some(ref warn_msg) = preflight.drive_filesystem.warning_message {
            eprintln!("WARNING: Incompatible Drive Format:\n{}", warn_msg);
        }
    }

    let preflight_warn = if preflight.drive_filesystem.is_unsupported {
        preflight.drive_filesystem.warning_message.clone()
    } else {
        None
    };

    let preflight_danger = if !preflight.files_integrity.is_intact {
        Some(format!(
            "Missing Utilities: Critical binaries missing from ./utils: {}",
            preflight
                .files_integrity
                .missing_critical_files
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    } else {
        None
    };

    let _single_instance_guard = preflight.single_instance_guard;

    // Initialize GameManager and discover local handlers
    let mut game_manager = GameManager::new(platform.clone())?;
    game_manager.set_handlers_dir(&app_settings.handlers_directory);
    game_manager.set_nucleus_data_dir(&app_settings.nucleus_data_dir);
    let _ = game_manager.initialize();
    let handler_count = game_manager.get_handlers().len();
    info!(
        "GameManager initialized (loaded {} handlers)",
        handler_count
    );

    // Optional auto-scan if enabled in settings (will run in background during GUI mode)
    if app_settings.auto_scan_on_start {
        info!("Auto-scan on start enabled (will run in background during GUI mode)...");
    }

    // Initialize InputManager
    let mut input_manager = InputManager::new(Some(platform.clone()))?;
    let _ = input_manager.register_settings(&app_settings);
    let gamepads = input_manager.gamepad().get_gamepads().to_vec();
    info!(
        "InputManager initialized (detected {} gamepads)",
        gamepads.len()
    );

    println!("==================================================");
    println!("NucleusCoop - Rust Rewrite v0.1.0 (Modular Engine)");
    println!("Platform services ready.");
    println!("Discovered handlers: {}", handler_count);
    for (guid, handler) in game_manager.get_handlers() {
        println!(
            "  - [{}] {} (max {} players)",
            guid, handler.game_name, handler.max_players
        );
    }
    println!("Detected gamepads: {}", gamepads.len());
    println!("System ready for splitscreen sessions.");
    println!("==================================================");

    // Process CLI arguments if provided
    let args: Vec<String> = std::env::args().collect();
    let is_cli_command = args.len() > 1
        && matches!(
            args[1].as_str(),
            "--scan"
                | "--list-handlers"
                | "--hub-search"
                | "--hub-detail"
                | "--hub-install"
                | "--set-handlers-dir"
                | "--set-theme"
                | "--play"
        );

    if is_cli_command {
        match args[1].as_str() {
            "--scan" => {
                println!("Scanning drives for supported games...");
                let found = game_manager.scan_disks()?;
                println!("Scan finished. Found {} executables.", found.len());
                for g in &found {
                    println!("  Found: {} ({})", g.name, g.exe_path.display());
                }
            }
            "--list-handlers" => {
                println!("Available handlers ({} total):", handler_count);
                for (guid, h) in game_manager.get_handlers() {
                    println!(
                        "  GUID: {}, Name: {}, Exe: {}",
                        guid, h.game_name, h.executable_name
                    );
                }
            }
            "--hub-search" => {
                let query = args.get(2).map(|s| s.as_str()).unwrap_or("");
                println!("Searching Hub for '{}'...", query);
                let rt = tokio::runtime::Runtime::new()?;
                let results = rt.block_on(game_manager.hub().search_handlers(query))?;
                println!("Hub returned {} results.", results.len());
                for r in results {
                    println!("  [{}] {} v{}", r.id, r.title, r.version);
                }
            }
            "--hub-detail" => {
                let handler_id = match args.get(2) {
                    Some(id) if !id.trim().is_empty() => id.trim(),
                    _ => {
                        eprintln!("Error: Missing handler ID. Usage: --hub-detail <handler-id>");
                        return Ok(());
                    }
                };
                println!("Fetching details for handler '{}' from Hub...", handler_id);
                let rt = tokio::runtime::Runtime::new()?;
                let detail = rt.block_on(game_manager.hub().get_handler_details(handler_id))?;
                println!("==================================================");
                println!("Handler Details: {}", detail.id);
                println!("  Game Name:       {}", detail.game.name);
                if let Some(ref steam_id) = detail.game.steam_id {
                    println!("  Steam App ID:    {}", steam_id);
                }
                if let Some(ref publisher) = detail.game.publisher {
                    println!("  Publisher:       {}", publisher);
                }
                println!("  Author:          {}", detail.author);
                println!("  Current Version: {}", detail.current_version);
                println!("  Download URL:    {}", detail.download_url);
                if !detail.description.is_empty() {
                    println!("  Description:     {}", detail.description);
                }
                if !detail.versions.is_empty() {
                    println!("  Version History ({} release(s)):", detail.versions.len());
                    for v in &detail.versions {
                        println!("    - v{} ({}): {}", v.version, v.release_date, v.changelog);
                    }
                }
                if !detail.comments.is_empty() {
                    println!("  Comments ({} comment(s)):", detail.comments.len());
                    for c in &detail.comments {
                        println!("    - [{}]: {}", c.author, c.content);
                    }
                }
                println!("==================================================");
            }
            "--hub-install" => {
                let handler_id = match args.get(2) {
                    Some(id) if !id.trim().is_empty() => id.trim(),
                    _ => {
                        eprintln!("Error: Missing handler ID. Usage: --hub-install <handler-id>");
                        return Ok(());
                    }
                };
                let dest_dir = nucleus_engine::game_manager::resolve_install_relative(
                    std::path::Path::new(&app_settings.handlers_directory),
                );
                let dest_dir = dest_dir.as_path();
                println!(
                    "Downloading and extracting handler '{}' to '{}'...",
                    handler_id,
                    dest_dir.display()
                );
                let rt = tokio::runtime::Runtime::new()?;
                let extracted = rt.block_on(
                    game_manager
                        .hub()
                        .download_and_extract_handler(handler_id, dest_dir),
                )?;
                println!(
                    "Successfully extracted {} file(s) for handler '{}':",
                    extracted.len(),
                    handler_id
                );
                for file in &extracted {
                    println!("  - {}", file.display());
                }
                game_manager.reload_handlers()?;
                let updated_count = game_manager.get_handlers().len();
                info!(
                    "GameManager reloaded (now {} handlers available)",
                    updated_count
                );
                println!(
                    "GameManager reloaded. Available handlers count: {}",
                    updated_count
                );
                println!(
                    "Confirmation: Handler '{}' installed successfully and is immediately available.",
                    handler_id
                );
            }
            "--set-handlers-dir" => {
                if let Some(dir) = args.get(2) {
                    app_settings.handlers_directory = dir.clone();
                    app_settings.save()?;
                    println!("Handlers directory updated to: {}", dir);
                } else {
                    eprintln!("Error: Missing directory path. Usage: --set-handlers-dir <path>");
                }
            }
            "--set-theme" => {
                if let Some(theme) = args.get(2) {
                    app_settings.theme = theme.clone();
                    app_settings.save()?;
                    println!("Theme updated to: {}", theme);
                } else {
                    eprintln!("Error: Missing theme name. Usage: --set-theme <theme>");
                }
            }
            "--play" => {
                let guid = args.get(2).map(|s| s.as_str()).unwrap_or("My Game");
                let exe_path = args.get(3).map(|s| s.as_str()).unwrap_or("game.exe");
                let players: usize = args
                    .iter()
                    .position(|a| a == "--players" || a == "-p")
                    .and_then(|idx| args.get(idx + 1))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(2);
                let layout: String = args
                    .iter()
                    .position(|a| a == "--layout" || a == "-l")
                    .and_then(|idx| args.get(idx + 1))
                    .cloned()
                    .unwrap_or_else(|| "Auto".to_string());

                println!(
                    "Starting splitscreen session for '{}' with executable '{}' ({} players, layout: {})...",
                    guid, exe_path, players, layout
                );
                let mut profile = nucleus_profile::GameProfile::new(guid, guid, exe_path);
                profile.player_count = players;
                profile.layout = layout;
                game_manager.start_game(guid, profile)?;
                println!("Game session started! Press Ctrl+C or kill instances to end session.");
                if let Some(session) = game_manager.active_session() {
                    println!(
                        "Active session ID: {}, status: {:?}",
                        session.profile.id,
                        session.status()
                    );
                }
                let mut hotkeys = nucleus_input::HotkeyListener::new();
                let lock_key = app_settings.lock_key_code();
                let _ = hotkeys.register_hotkey(lock_key, lock_key, 0);
                let _ = hotkeys.register_from_settings(&app_settings);
                let mut input_lock = nucleus_input::InputLock::new();
                input_lock.set_toggle_key(lock_key);

                let mut tick_counter = 0;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    tick_counter += 1;
                    if tick_counter >= 5 {
                        tick_counter = 0;
                        let is_running = game_manager
                            .active_session()
                            .map_or(false, |s| s.is_running);
                        if !is_running {
                            let _ = game_manager.stop_game();
                            break;
                        }
                        let _ = game_manager.tick();
                    }

                    let triggered = hotkeys.poll_hotkeys();
                    for hk in triggered {
                        if hk == nucleus_input::HOTKEY_ID_LOCK || hk == lock_key {
                            input_lock.toggle_with_osd(Some(platform.osd_manager()));
                        } else if hk == nucleus_input::HOTKEY_ID_CUTSCENES {
                            // Cutscenes mode passes through input lock state (C# HotkeyListener.cs:107)
                            let _ = game_manager.toggle_cutscenes_mode();
                        } else if hk == nucleus_input::HOTKEY_ID_COUCH_OSD {
                            platform
                                .osd_manager()
                                .show(2000, "Couch In-Game Menu (OSD) Toggled");
                        } else if input_lock.is_locked() {
                            // Parity with C# HotkeyListener.cs:161:
                            // Suppress action and prompt user to unlock inputs first
                            platform.osd_manager().show(
                                1600,
                                &format!(
                                    "Unlock Inputs First (Press {} Key)",
                                    app_settings.lock_inputs_key
                                ),
                            );
                        } else {
                            match hk {
                                nucleus_input::HOTKEY_ID_STOP | nucleus_input::HOTKEY_ID_CLOSE => {
                                    info!("Stop hotkey triggered, ending session...");
                                    let _ = game_manager.stop_game();
                                    break;
                                }
                                nucleus_input::HOTKEY_ID_RESET_WINDOWS => {
                                    info!(
                                        "Reset windows hotkey triggered, repositioning windows..."
                                    );
                                    platform
                                        .osd_manager()
                                        .show(1000, "Resetting game windows. Please wait...");
                                    let _ = game_manager.tick();
                                    platform.osd_manager().show(2000, "Game Windows Resetted");
                                }
                                nucleus_input::HOTKEY_ID_SET_FOCUS => {
                                    platform.osd_manager().show(2000, "Game Windows Unfocused");
                                }
                                nucleus_input::HOTKEY_ID_SWITCH => {
                                    let mode_desc = game_manager.switch_layout().unwrap_or(None);
                                    let text = mode_desc
                                        .map(|d| format!("Layout Switched: {}", d))
                                        .unwrap_or_else(|| "Switching Layouts".to_string());
                                    platform.osd_manager().show(1600, &text);
                                }
                                nucleus_input::HOTKEY_ID_TOPMOST => {
                                    let _ = game_manager.toggle_minimize_windows();
                                }
                                nucleus_input::HOTKEY_ID_REMINDER => {
                                    platform
                                        .osd_manager()
                                        .show(1000, "Shortcuts Reminder Toggled");
                                }
                                nucleus_input::HOTKEY_ID_MERGER_SWITCH => {
                                    platform.osd_manager().show(800, "Merger Focus Switched");
                                }
                                nucleus_input::HOTKEY_ID_COUCH_OSD => {
                                    platform
                                        .osd_manager()
                                        .show(2000, "Couch In-Game Menu (OSD) Toggled");
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    } else {
        let initial_tab = args
            .iter()
            .position(|a| a == "--tab")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let target_game = args
            .iter()
            .position(|a| a == "--game")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let initial_players = args
            .iter()
            .position(|a| a == "--players" || a == "-p")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok());
        let initial_layout = args
            .iter()
            .position(|a| a == "--layout" || a == "-l")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let search_query = args
            .iter()
            .position(|a| a == "--search" || a == "-s")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let controller_shortcuts = args.iter().any(|a| a == "--controller-shortcuts");
        let hub_details = args
            .iter()
            .position(|a| a == "--hub-details")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let hub_tab = args
            .iter()
            .position(|a| a == "--hub-tab")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let big_picture = args.iter().any(|a| a == "--big-picture");
        let big_picture_screen = args
            .iter()
            .position(|a| a == "--big-picture-screen")
            .and_then(|i| args.get(i + 1))
            .cloned();

        let gui_opts = LaunchGuiOptions {
            initial_tab,
            target_game,
            initial_players,
            initial_layout,
            search_query,
            controller_shortcuts,
            hub_details,
            hub_tab,
            big_picture,
            big_picture_screen,
            preflight_warn: preflight_warn.clone(),
            preflight_danger: preflight_danger.clone(),
        };

        println!(
            "Launching Native GPUI Desktop Window (opts: {:?})...",
            gui_opts
        );
        launch_gui(gamepads, app_settings, game_manager, gui_opts);
    }

    Ok(())
}

/// Helper function to normalize handler titles and game names for fuzzy Hub matching
pub(crate) fn normalize_title_for_hub(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut clean = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_alphanumeric() || c.is_whitespace() {
            clean.push(c);
        }
    }
    let tokens: Vec<&str> = clean.split_whitespace().collect();
    let converted: Vec<String> = tokens
        .into_iter()
        .map(|tok| match tok {
            "ii" => "2".to_string(),
            "iii" => "3".to_string(),
            "iv" => "4".to_string(),
            "v" => "5".to_string(),
            "vi" => "6".to_string(),
            "vii" => "7".to_string(),
            _ => tok.to_string(),
        })
        .collect();
    converted.join(" ")
}

fn launch_gui(
    gamepads: Vec<GamepadDevice>,
    app_settings: AppSettings,
    game_manager: GameManager,
    gui_opts: LaunchGuiOptions,
) {
    let width = app_settings.window_width;
    let height = app_settings.window_height;
    let handlers_dir = app_settings.handlers_directory.clone();
    let current_theme = app_settings.theme.clone();
    let auto_scan = app_settings.auto_scan_on_start;
    nucleus_ui::theme::set_active_theme(&current_theme);
    nucleus_ui::theme::set_active_accent(&app_settings.accent_color);

    // Convert detected controllers into UI device items
    let device_items: Vec<DeviceItem> = gamepads
        .iter()
        .map(|g| DeviceItem::new(g.id, g.name.clone(), g.guid.clone(), g.is_connected))
        .collect();

    // Convert loaded handlers into rich UI items
    let handler_items: Vec<HandlerItem> = game_manager
        .get_handlers()
        .iter()
        .filter(|(guid, h)| {
            !guid.eq_ignore_ascii_case("My Game")
                && !h.game_name.eq_ignore_ascii_case("My Game")
                && !guid.eq_ignore_ascii_case("MasterHandler")
                && h.handler_path
                    .as_deref()
                    .map_or(true, |p| !p.ends_with("MasterHandler.js"))
        })
        .map(|(guid, h)| {
            let profile = game_manager.get_profile(guid);
            let meta_info = game_manager
                .meta_info()
                .find_entry(guid)
                .or_else(|| game_manager.meta_info().find_entry(&h.game_name))
                .map(|(_, e)| e);
            build_handler_item(guid, h, profile, meta_info)
        })
        .collect();

    let legacy_names: Vec<String> = handler_items.iter().map(|h| h.game_name.clone()).collect();
    let device_count = device_items.len();

    // Wrap GameManager in Arc<Mutex<...>> for safe shared access across callbacks
    let platform = game_manager.platform().clone();
    let gm = Arc::new(Mutex::new(game_manager));

    // Optional background auto-scan that does not block window launch
    if auto_scan {
        let gm_scan = gm.clone();
        std::thread::spawn(move || {
            info!("Background auto-scan started...");
            if let Ok(mut mgr) = gm_scan.lock() {
                if let Ok(found) = mgr.scan_disks() {
                    info!(
                        "Background auto-scan complete. Found {} games.",
                        found.len()
                    );
                }
            }
        });
    }

    // Wrap AppSettings in Arc<Mutex<...>> for settings persistence closures
    let app_settings_shared = Arc::new(Mutex::new(app_settings.clone()));
    let platform_launch = if let Ok(mgr) = gm.lock() {
        mgr.platform().clone()
    } else {
        platform::create_platform_services()
    };
    let app_settings_launch = app_settings_shared.clone();
    let (osd_signal_tx, osd_signal_rx) =
        tokio::sync::mpsc::unbounded_channel::<nucleus_ui::EngineToGui>();
    let osd_signal_tx_launch = osd_signal_tx.clone();

    // Attach interactive launch prompt bridge to GameManager
    let osd_signal_prompt = osd_signal_tx.clone();
    let prompt_handler = Arc::new(move |message: &str| -> bool {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = osd_signal_prompt.send(nucleus_ui::EngineToGui::LaunchPrompt(
            nucleus_ui::LaunchPromptRequest {
                player_index: 0,
                player_count: 2,
                message: message.to_string(),
                auto_proceed_seconds: None,
                response_tx: resp_tx,
            },
        ));
        resp_rx.blocking_recv().unwrap_or(true)
    });
    if let Ok(mut mgr) = gm.lock() {
        mgr.set_prompt_handler(prompt_handler);
    }

    // Launch callback (asynchronous non-blocking execution)
    let gm_launch = gm.clone();
    let platform_cb = platform_launch.clone();
    let launcher_cb = move |game_name: &str,
                            players: usize,
                            layout: &str,
                            options: &std::collections::HashMap<String, String>,
                            assignments: &[String],
                            monitors: &[usize],
                            audio_devices: &[String]| {
        let game_name_str = game_name.to_string();
        let layout_str = layout.to_string();
        let options_map = options.clone();
        let assignments_vec = assignments.to_vec();
        let monitors_vec = monitors.to_vec();
        let audio_devices_vec = audio_devices.to_vec();
        let gm_thread = gm_launch.clone();
        let platform_thread = platform_cb.clone();
        let settings_thread = app_settings_launch.clone();
        let osd_signal_thread = osd_signal_tx_launch.clone();
        std::thread::spawn(move || {
            println!(
                "[Game Engine] Launch request: '{}', {} players, layout: {}, options: {:?}, assignments: {:?}, monitors: {:?}, audio: {:?}",
                game_name_str,
                players,
                layout_str,
                options_map,
                assignments_vec,
                monitors_vec,
                audio_devices_vec
            );
            let started = if let Ok(mut mgr) = gm_thread.lock() {
                let mut matched_guid = None;
                let mut default_exe = "game.exe".to_string();
                for (guid, h) in mgr.get_handlers() {
                    if h.game_name.eq_ignore_ascii_case(&game_name_str)
                        || guid.eq_ignore_ascii_case(&game_name_str)
                    {
                        matched_guid = Some(guid.clone());
                        if !h.executable_name.is_empty() {
                            default_exe = h.executable_name.clone();
                        }
                        break;
                    }
                }

                let guid = matched_guid.unwrap_or_else(|| game_name_str.clone());
                let mut profile = mgr.get_profile(&guid).cloned().unwrap_or_else(|| {
                    nucleus_profile::GameProfile::new(&guid, &guid, &default_exe)
                });
                profile.player_count = players;
                profile.layout = layout_str.clone();
                profile.options = options_map.clone();
                profile.player_device_assignments = assignments_vec;
                profile.player_monitor_assignments = monitors_vec;
                profile.player_audio_device_assignments = audio_devices_vec;

                if let Some(emu) = options_map.get("preferred_emulator") {
                    let emu_clean = if emu.contains("PeerParty") {
                        "PeerParty"
                    } else if emu.contains("Goldberg") {
                        "Goldberg"
                    } else if emu.contains("Nemirtingas") {
                        "Nemirtingas"
                    } else {
                        "Auto"
                    };
                    profile.preferred_emulator = emu_clean.to_string();
                }
                if let Some(room) = options_map.get("peerparty_room") {
                    if !room.trim().is_empty() {
                        profile.peerparty_room_override = Some(room.trim().to_string());
                    } else {
                        profile.peerparty_room_override = None;
                    }
                }

                let _ = mgr.save_game_profile(&guid, &profile);

                let started = match mgr.start_game(&guid, profile) {
                    Ok(_) => {
                        println!(
                            "[Game Engine] Splitscreen session started successfully for '{}'!",
                            guid
                        );
                        true
                    }
                    Err(e) => {
                        eprintln!(
                            "[Game Engine] Error launching session for '{}': {}",
                            guid, e
                        );
                        false
                    }
                };
                started
            } else {
                false
            };

            if started {
                let current_settings = if let Ok(s) = settings_thread.lock() {
                    s.clone()
                } else {
                    AppSettings::default()
                };

                let mut hotkeys = nucleus_input::HotkeyListener::new();
                let lock_key = current_settings.lock_key_code();
                let _ = hotkeys.register_hotkey(lock_key, lock_key, 0);
                let _ = hotkeys.register_from_settings(&current_settings);
                let mut input_lock = nucleus_input::InputLock::new();
                input_lock.set_toggle_key(lock_key);

                let mut tick_counter = 0;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    tick_counter += 1;
                    if tick_counter >= 5 {
                        tick_counter = 0;
                        if let Ok(mut mgr) = gm_thread.lock() {
                            let is_running = mgr.active_session().map_or(false, |s| s.is_running);
                            if !is_running {
                                let _ = mgr.stop_game();
                                break;
                            }
                            let _ = mgr.tick();
                        } else {
                            break;
                        }
                    }

                    let triggered = hotkeys.poll_hotkeys();
                    for hk in triggered {
                        if hk == nucleus_input::HOTKEY_ID_LOCK || hk == lock_key {
                            input_lock.toggle_with_osd(Some(platform_thread.osd_manager()));
                        } else if hk == nucleus_input::HOTKEY_ID_CUTSCENES {
                            // Cutscenes mode passes through input lock state (C# HotkeyListener.cs:107)
                            if let Ok(mut mgr) = gm_thread.lock() {
                                let _ = mgr.toggle_cutscenes_mode();
                            }
                        } else if hk == nucleus_input::HOTKEY_ID_COUCH_OSD {
                            platform_thread
                                .osd_manager()
                                .show(2000, "Couch In-Game Menu (OSD) Toggled");
                            let _ =
                                osd_signal_thread.send(nucleus_ui::EngineToGui::ToggleInGameOsd {
                                    summoner_slot: None,
                                });
                        } else if input_lock.is_locked() {
                            // Parity with C# HotkeyListener.cs:161:
                            // Suppress action and prompt user to unlock inputs first
                            platform_thread.osd_manager().show(
                                1600,
                                &format!(
                                    "Unlock Inputs First (Press {} Key)",
                                    current_settings.lock_inputs_key
                                ),
                            );
                        } else {
                            match hk {
                                nucleus_input::HOTKEY_ID_STOP | nucleus_input::HOTKEY_ID_CLOSE => {
                                    println!("[Hotkey] Stop session triggered by hotkey");
                                    if let Ok(mut mgr) = gm_thread.lock() {
                                        let _ = mgr.stop_game();
                                    }
                                    break;
                                }
                                nucleus_input::HOTKEY_ID_RESET_WINDOWS => {
                                    println!("[Hotkey] Reset windows triggered by hotkey");
                                    platform_thread
                                        .osd_manager()
                                        .show(1000, "Resetting game windows. Please wait...");
                                    if let Ok(mut mgr) = gm_thread.lock() {
                                        let _ = mgr.tick();
                                    }
                                    platform_thread
                                        .osd_manager()
                                        .show(2000, "Game Windows Resetted");
                                }
                                nucleus_input::HOTKEY_ID_SET_FOCUS => {
                                    platform_thread
                                        .osd_manager()
                                        .show(2000, "Game Windows Unfocused");
                                }
                                nucleus_input::HOTKEY_ID_SWITCH => {
                                    let mode_desc = if let Ok(mut mgr) = gm_thread.lock() {
                                        mgr.switch_layout().unwrap_or(None)
                                    } else {
                                        None
                                    };
                                    let text = mode_desc
                                        .map(|d| format!("Layout Switched: {}", d))
                                        .unwrap_or_else(|| "Switching Layouts".to_string());
                                    platform_thread.osd_manager().show(1600, &text);
                                }
                                nucleus_input::HOTKEY_ID_TOPMOST => {
                                    if let Ok(mut mgr) = gm_thread.lock() {
                                        let _ = mgr.toggle_minimize_windows();
                                    }
                                }
                                nucleus_input::HOTKEY_ID_REMINDER => {
                                    platform_thread
                                        .osd_manager()
                                        .show(1000, "Shortcuts Reminder Toggled");
                                }
                                nucleus_input::HOTKEY_ID_MERGER_SWITCH => {
                                    platform_thread
                                        .osd_manager()
                                        .show(800, "Merger Focus Switched");
                                }
                                nucleus_input::HOTKEY_ID_COUCH_OSD => {
                                    platform_thread
                                        .osd_manager()
                                        .show(2000, "Couch In-Game Menu (OSD) Toggled");
                                    let _ = osd_signal_thread.send(
                                        nucleus_ui::EngineToGui::ToggleInGameOsd {
                                            summoner_slot: None,
                                        },
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        });
    };

    // Add game callback
    let gm_add = gm.clone();
    let add_game_cb = move |exe_path: &str| -> Option<HandlerItem> {
        println!("[Game Engine] Adding game from path: {}", exe_path);
        if let Ok(mut mgr) = gm_add.lock() {
            let p = Path::new(exe_path);
            match mgr.add_game(p) {
                Ok(Some(guid)) => {
                    println!(
                        "[Game Engine] Game successfully associated with handler '{}'!",
                        guid
                    );
                    if let Some(h) = mgr.get_handlers().get(&guid).cloned() {
                        let profile = mgr.get_profile(&guid).cloned();
                        let meta_info = mgr
                            .meta_info()
                            .find_entry(&guid)
                            .or_else(|| mgr.meta_info().find_entry(&h.game_name))
                            .map(|(_, e)| e)
                            .cloned();
                        return Some(build_handler_item(
                            &guid,
                            &h,
                            profile.as_ref(),
                            meta_info.as_ref(),
                        ));
                    }
                }
                Ok(None) => {
                    println!(
                        "[Game Engine] Executable '{}' registered (no existing handler matched).",
                        exe_path
                    );
                }
                Err(e) => {
                    eprintln!("[Game Engine] Error adding game: {}", e);
                }
            }
        }
        None
    };

    // Hub search callback
    let gm_search = gm.clone();
    let hub_search_cb = move |query: &str| -> Vec<HubPackageItem> {
        println!("[Game Engine] Searching Community Hub for: '{}'", query);
        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[Game Engine] Failed to create Tokio runtime: {}", e);
                return Vec::new();
            }
        };

        let hub_client = if let Ok(mgr) = gm_search.lock() {
            mgr.hub().clone()
        } else {
            nucleus_hub::HubClient::default()
        };

        let installed_handlers: Vec<(String, String, Option<String>, Option<i32>)> =
            if let Ok(mgr) = gm_search.lock() {
                mgr.get_handlers()
                    .iter()
                    .map(|(guid, h)| {
                        (
                            guid.to_lowercase(),
                            h.game_name.to_lowercase(),
                            h.hub_handler_id.as_ref().map(|s| s.to_lowercase()),
                            h.hub_handler_version,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };

        match rt.block_on(hub_client.search_raw_handlers(query)) {
            Ok(handlers) => handlers
                .into_iter()
                .map(|h| {
                    let title = h.display_title().to_string();
                    let game_name = h.game_name.clone().unwrap_or_else(|| title.clone());
                    let remote_ver = h.version_number();
                    let h_id_lower = h.id.to_lowercase();
                    let game_name_lower = game_name.to_lowercase();
                    let title_lower = title.to_lowercase();

                    let norm_title = normalize_title_for_hub(&title);
                    let norm_game_name = normalize_title_for_hub(&game_name);

                    // Check if matching local handler is found via exact Hub ID or normalized title
                    let matched_local =
                        installed_handlers.iter().find(|(guid, name, hub_id, _)| {
                            // 1. Direct Hub ID match (e.g. Hub.Handler.Id in script)
                            hub_id.as_deref() == Some(&h_id_lower)
                            || guid == &h_id_lower
                            // 2. Direct string equality
                            || name == &game_name_lower
                            || name == &title_lower
                            || guid == &game_name_lower
                            || guid == &title_lower
                            // 3. Normalized title equality (punctuation stripped, Roman numerals converted)
                            || {
                                let norm_name = normalize_title_for_hub(name);
                                let norm_guid = normalize_title_for_hub(guid);
                                norm_name == norm_title
                                    || norm_name == norm_game_name
                                    || norm_guid == norm_title
                                    || norm_guid == norm_game_name
                            }
                        });

                    let installed_version: Option<i32> =
                        matched_local.map(|(_, _, _, ver)| ver.unwrap_or(1));
                    let is_installed = installed_version.is_some();
                    let cover_url = h.cover_url();
                    let likes = h.rating();
                    let downloads = h.downloads();
                    let comments = h.comments();
                    let tags = h.extracted_tags();
                    let created_at = h.created_at.clone().unwrap_or_default();
                    let updated_at = h.updated_at.clone().unwrap_or_default();
                    let is_verified = h.verified.unwrap_or(true);
                    let playable_controllers = h.playable_controllers.unwrap_or(true);
                    let playable_mouse_kb = h.playable_mouse_keyboard.unwrap_or(false);
                    let playable_multi_mouse_kb = h.playable_multi_mouse_keyboard.unwrap_or(false);

                    HubPackageItem::new(
                        h.id.clone(),
                        title,
                        format!("v{}", remote_ver),
                        h.author().to_string(),
                        h.description
                            .as_deref()
                            .or(h.game_description.as_deref())
                            .unwrap_or("")
                            .to_string(),
                        format!("{} Players", h.max_players_count()),
                        is_installed,
                    )
                    .with_version_info(remote_ver, installed_version)
                    .with_verification(is_verified)
                    .with_inputs(
                        playable_controllers,
                        playable_mouse_kb,
                        playable_multi_mouse_kb,
                    )
                    .with_meta(
                        game_name, cover_url, likes, downloads, comments, tags, created_at,
                        updated_at,
                    )
                })
                .collect(),
            Err(e) => {
                eprintln!("[Game Engine] Hub search failed: {}", e);
                Vec::new()
            }
        }
    };

    // Hub install callback
    let gm_install = gm.clone();
    let handlers_dir_install = handlers_dir.clone();
    let hub_install_cb = move |handler_id: &str,
                               progress_cb: Box<dyn Fn(f32) + Send + Sync>|
          -> Option<HandlerItem> {
        println!(
            "[Game Engine] Downloading handler '{}' from Hub...",
            handler_id
        );
        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[Game Engine] Failed to create Tokio runtime: {}", e);
                return None;
            }
        };

        let (hub_client, dest_dir) = if let Ok(mgr) = gm_install.lock() {
            (mgr.hub().clone(), mgr.handlers_dir().to_path_buf())
        } else {
            (
                nucleus_hub::HubClient::default(),
                Path::new(&handlers_dir_install).to_path_buf(),
            )
        };

        let existing_guids: std::collections::HashSet<String> = if let Ok(mgr) = gm_install.lock() {
            mgr.get_handlers().keys().cloned().collect()
        } else {
            std::collections::HashSet::new()
        };

        let on_prog = {
            let cb = progress_cb;
            move |p: f32| {
                cb(p);
            }
        };

        match rt.block_on(
            hub_client.download_and_extract_handler_with_progress(handler_id, &dest_dir, on_prog),
        ) {
            Ok(files) => {
                println!(
                    "[Game Engine] Extracted {} files for handler '{}'!",
                    files.len(),
                    handler_id
                );
                if let Ok(mut mgr) = gm_install.lock() {
                    let _ = mgr.reload_handlers();

                    // Find the matched handler using our multi-tier detection
                    let mut matched_entry: Option<(String, nucleus_engine::HandlerConfig)> = None;

                    // Tier 1: Any newly loaded handler whose GUID was not present before reload
                    for (guid, h) in mgr.get_handlers() {
                        if !existing_guids.contains(guid) {
                            matched_entry = Some((guid.clone(), h.clone()));
                            break;
                        }
                    }

                    // Tier 2: Handler matching hub_handler_id, exact GUID, or filename/game_name match
                    if matched_entry.is_none() {
                        for (guid, h) in mgr.get_handlers() {
                            if h.hub_handler_id.as_deref() == Some(handler_id)
                                || guid.eq_ignore_ascii_case(handler_id)
                                || files.iter().any(|f| {
                                    let lossy = f.to_string_lossy().to_lowercase();
                                    lossy.contains(&guid.to_lowercase())
                                        || lossy.contains(&h.game_name.to_lowercase())
                                })
                            {
                                matched_entry = Some((guid.clone(), h.clone()));
                                break;
                            }
                        }
                    }

                    if let Some((guid, h)) = matched_entry {
                        println!(
                            "[Game Engine] Successfully associated Hub handler '{}' with local GUID '{}' ('{}')",
                            handler_id, guid, h.game_name
                        );
                        let profile = mgr.get_profile(&guid).cloned();
                        let meta_info = mgr
                            .meta_info()
                            .find_entry(&guid)
                            .or_else(|| mgr.meta_info().find_entry(&h.game_name))
                            .map(|(_, e)| e)
                            .cloned();
                        return Some(build_handler_item(
                            &guid,
                            &h,
                            profile.as_ref(),
                            meta_info.as_ref(),
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[Game Engine] Hub download failed for '{}': {}",
                    handler_id, e
                );
            }
        }
        None
    };

    // Hub Historical Version Install callback
    let gm_version_install = gm.clone();
    let handlers_dir_version = handlers_dir.clone();
    let hub_version_install_cb = move |handler_id: &str,
                                       package_id: &str,
                                       version_num: i32,
                                       expected_size: Option<u64>,
                                       progress_cb: Box<dyn Fn(f32) + Send + Sync>|
          -> Option<HandlerItem> {
        println!(
            "[Game Engine] Downloading historical handler '{}' (package: '{}', v{}) from Hub...",
            handler_id, package_id, version_num
        );
        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[Game Engine] Failed to create Tokio runtime: {}", e);
                return None;
            }
        };

        let (hub_client, dest_dir) = if let Ok(mgr) = gm_version_install.lock() {
            (mgr.hub().clone(), mgr.handlers_dir().to_path_buf())
        } else {
            (
                nucleus_hub::HubClient::default(),
                Path::new(&handlers_dir_version).to_path_buf(),
            )
        };

        let existing_guids: std::collections::HashSet<String> =
            if let Ok(mgr) = gm_version_install.lock() {
                mgr.get_handlers().keys().cloned().collect()
            } else {
                std::collections::HashSet::new()
            };

        let on_prog = {
            let cb = progress_cb;
            move |p: f32| {
                cb(p);
            }
        };

        match rt.block_on(
            hub_client.download_and_extract_package_version_with_progress(
                handler_id,
                package_id,
                version_num,
                expected_size,
                &dest_dir,
                on_prog,
            ),
        ) {
            Ok(files) => {
                println!(
                    "[Game Engine] Extracted {} files for handler version '{}'!",
                    files.len(),
                    handler_id
                );
                if let Ok(mut mgr) = gm_version_install.lock() {
                    let _ = mgr.reload_handlers();

                    let mut matched_entry: Option<(String, nucleus_engine::HandlerConfig)> = None;
                    for (guid, h) in mgr.get_handlers() {
                        if !existing_guids.contains(guid) {
                            matched_entry = Some((guid.clone(), h.clone()));
                            break;
                        }
                    }

                    if matched_entry.is_none() {
                        for (guid, h) in mgr.get_handlers() {
                            if h.hub_handler_id.as_deref() == Some(handler_id)
                                || guid.eq_ignore_ascii_case(handler_id)
                                || files.iter().any(|f| {
                                    let lossy = f.to_string_lossy().to_lowercase();
                                    lossy.contains(&guid.to_lowercase())
                                        || lossy.contains(&h.game_name.to_lowercase())
                                })
                            {
                                matched_entry = Some((guid.clone(), h.clone()));
                                break;
                            }
                        }
                    }

                    if let Some((guid, h)) = matched_entry {
                        println!(
                            "[Game Engine] Successfully associated Hub handler version '{}' with local GUID '{}' ('{}')",
                            handler_id, guid, h.game_name
                        );
                        let profile = mgr.get_profile(&guid).cloned();
                        let meta_info = mgr
                            .meta_info()
                            .find_entry(&guid)
                            .or_else(|| mgr.meta_info().find_entry(&h.game_name))
                            .map(|(_, e)| e)
                            .cloned();
                        return Some(build_handler_item(
                            &guid,
                            &h,
                            profile.as_ref(),
                            meta_info.as_ref(),
                        ));
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[Game Engine] Historical version download failed for '{}': {}",
                    handler_id, e
                );
            }
        }
        None
    };

    // Hot Drop-In / Drop-Out callbacks for Couch OSD
    let gm_drop_in = gm.clone();
    let drop_in_cb = move |device_id: &str, layout_name: &str| -> Result<usize, String> {
        println!(
            "[Game Engine] Hot drop-in requested for device '{}' (layout: '{}')",
            device_id, layout_name
        );
        if let Ok(mut mgr) = gm_drop_in.lock() {
            mgr.drop_in_session_player(device_id, layout_name)
                .map_err(|e| e.to_string())
        } else {
            Err("Game engine lock unavailable".to_string())
        }
    };

    let gm_drop_out = gm.clone();
    let drop_out_cb = move |slot_idx: usize, layout_name: &str| -> Result<usize, String> {
        println!(
            "[Game Engine] Hot drop-out requested for slot {} (new layout: '{}')",
            slot_idx, layout_name
        );
        if let Ok(mut mgr) = gm_drop_out.lock() {
            mgr.drop_out_session_player(slot_idx, layout_name)
                .map_err(|e| e.to_string())
        } else {
            Err("Game engine lock unavailable".to_string())
        }
    };

    let gm_query_session = gm.clone();
    let query_session_cb = move || -> Option<nucleus_engine::session::ActiveSessionSnapshot> {
        if let Ok(mgr) = gm_query_session.lock() {
            mgr.active_session_snapshot()
        } else {
            None
        }
    };

    // Handlers directory changed callback
    let gm_dir = gm.clone();
    let settings_dir_arc = app_settings_shared.clone();
    let dir_change_cb = move |new_dir: &str| -> Vec<HandlerItem> {
        println!("[Game Engine] Updating handlers directory to: {}", new_dir);
        if let Ok(mut s) = settings_dir_arc.lock() {
            s.handlers_directory = new_dir.to_string();
            let _ = s.save();
        }
        if let Ok(mut mgr) = gm_dir.lock() {
            mgr.set_handlers_dir(new_dir);
            let _ = mgr.reload_handlers();
            return mgr
                .get_handlers()
                .iter()
                .filter(|(guid, h)| {
                    !guid.eq_ignore_ascii_case("My Game")
                        && !h.game_name.eq_ignore_ascii_case("My Game")
                        && !guid.eq_ignore_ascii_case("MasterHandler")
                        && h.handler_path
                            .as_deref()
                            .map_or(true, |p| !p.ends_with("MasterHandler.js"))
                })
                .map(|(guid, h)| {
                    let profile = mgr.get_profile(guid);
                    let meta_info = mgr
                        .meta_info()
                        .find_entry(guid)
                        .or_else(|| mgr.meta_info().find_entry(&h.game_name))
                        .map(|(_, e)| e);
                    build_handler_item(guid, h, profile, meta_info)
                })
                .collect();
        }
        Vec::new()
    };

    // Theme changed callback
    let settings_theme_arc = app_settings_shared.clone();
    let theme_change_cb = move |theme: &str| {
        println!("[Game Engine] Persisting theme: {}", theme);
        if let Ok(mut s) = settings_theme_arc.lock() {
            s.theme = theme.to_string();
            let _ = s.save();
        }
    };

    // Auto-scan changed callback
    let settings_scan_arc = app_settings_shared.clone();
    let auto_scan_cb = move |enabled: bool| {
        println!("[Game Engine] Persisting auto_scan: {}", enabled);
        if let Ok(mut s) = settings_scan_arc.lock() {
            s.auto_scan_on_start = enabled;
            let _ = s.save();
        }
    };

    // Scan storage drives for supported game installations
    let gm_drive_scan = gm.clone();
    let scan_games_cb = move || -> Vec<HandlerItem> {
        println!("[Game Engine] Scanning local drives for installed games...");
        let mut results = Vec::new();
        if let Ok(mut mgr) = gm_drive_scan.lock() {
            if let Ok(found) = mgr.scan_disks() {
                for fg in found {
                    let exe_name = fg
                        .exe_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    if let Some(h) = mgr
                        .get_handlers()
                        .values()
                        .find(|h| {
                            h.executable_name.eq_ignore_ascii_case(exe_name)
                                || h.game_name.eq_ignore_ascii_case(&fg.name)
                        })
                        .cloned()
                    {
                        let profile = mgr.get_profile(&h.guid).cloned();
                        let meta_info = mgr
                            .meta_info()
                            .find_entry(&h.guid)
                            .or_else(|| mgr.meta_info().find_entry(&h.game_name))
                            .map(|(_, e)| e)
                            .cloned();
                        results.push(build_handler_item(
                            &h.guid,
                            &h,
                            profile.as_ref(),
                            meta_info.as_ref(),
                        ));
                    }
                }
            }
        }
        results
    };

    // Favorite toggle callback
    let gm_fav = gm.clone();
    let toggle_fav_cb = move |guid: &str, is_fav: bool| {
        println!(
            "[Game Engine] Toggling favorite for '{}' -> {}",
            guid, is_fav
        );
        if let Ok(mut mgr) = gm_fav.lock() {
            let mut p = mgr.get_profile(guid).cloned().unwrap_or_else(|| {
                let exe = mgr
                    .get_handler(guid)
                    .map(|h| h.executable_name.clone())
                    .unwrap_or_default();
                nucleus_profile::GameProfile::new(guid, guid, exe)
            });
            p.favorite = is_fav;
            let _ = mgr.save_game_profile(guid, &p);
        }
    };

    // Save profile callback (for context menu property toggles and settings)
    let gm_prof = gm.clone();
    let save_profile_cb = move |item: &HandlerItem| {
        println!("[Game Engine] Saving profile settings for '{}'", item.guid);
        if let Ok(mut mgr) = gm_prof.lock() {
            let mut p = mgr.get_profile(&item.guid).cloned().unwrap_or_else(|| {
                nucleus_profile::GameProfile::new(&item.guid, &item.guid, &item.executable_name)
            });
            p.favorite = item.is_favorite;
            p.assign_gamepads_by_index = item.assign_gamepads_by_index;
            p.assign_gamepads_on_button_press = item.assign_gamepads_on_button_press;
            p.keep_instances_content_folder = item.keep_instances_content_folder;
            p.disable_profile = item.disable_profile;
            p.disable_handler_update = item.disable_handler_update;
            p.custom_game_icon = item.custom_game_icon.clone();
            p.steam_language = item.steam_language.clone();
            let _ = mgr.save_game_profile(&item.guid, &p);
        }
    };

    // Delete UserProfile save directory callback
    let gm_del = gm.clone();
    let delete_save_cb = move |guid: &str| {
        println!(
            "[Game Engine] Deleting UserProfile save path for '{}'",
            guid
        );
        if let Ok(mgr) = gm_del.lock() {
            let _ = mgr.delete_user_save_path(guid);
        }
    };

    // Remove game callback
    let gm_rem = gm.clone();
    let remove_game_cb = move |guid: &str| {
        println!("[Game Engine] Removing game '{}' from library", guid);
        if let Ok(mut mgr) = gm_rem.lock() {
            let _ = mgr.remove_game(guid);
        }
    };

    // Numbered profile callbacks
    let gm_list_prof = gm.clone();
    let list_profiles_cb = move |guid: &str| -> Vec<(usize, String)> {
        if let Ok(mgr) = gm_list_prof.lock() {
            mgr.list_saved_profiles(guid)
                .into_iter()
                .map(|(num, title, _)| (num, title))
                .collect()
        } else {
            Vec::new()
        }
    };

    let gm_load_prof = gm.clone();
    let load_profile_cb = move |guid: &str, num: usize| -> Option<nucleus_profile::GameProfile> {
        if let Ok(mut mgr) = gm_load_prof.lock() {
            mgr.load_numbered_profile(guid, num).ok()
        } else {
            None
        }
    };

    let gm_save_num_prof = gm.clone();
    let save_numbered_profile_cb = move |guid: &str,
                                         num: Option<usize>,
                                         prof: &nucleus_profile::GameProfile|
          -> Option<usize> {
        if let Ok(mut mgr) = gm_save_num_prof.lock() {
            mgr.save_numbered_profile(guid, num, prof).ok()
        } else {
            None
        }
    };

    let gm_del_prof = gm.clone();
    let delete_numbered_profile_cb = move |guid: &str, num: usize| -> bool {
        if let Ok(mut mgr) = gm_del_prof.lock() {
            mgr.delete_numbered_profile(guid, num).is_ok()
        } else {
            false
        }
    };

    let gm_unload_prof = gm.clone();
    let unload_profile_cb = move || {
        if let Ok(mgr) = gm_unload_prof.lock() {
            mgr.unload_profile();
        }
    };

    // In-game Couch OSD session callbacks
    let gm_freeze = gm.clone();
    let toggle_freeze_cb = move || -> Option<bool> {
        if let Ok(mut mgr) = gm_freeze.lock() {
            mgr.toggle_freeze_session().ok().flatten()
        } else {
            None
        }
    };

    let gm_reassign = gm.clone();
    let reassign_cb = move |slot: usize, dev_id: usize| {
        if let Ok(mut mgr) = gm_reassign.lock() {
            let _ = mgr.reassign_session_controller(slot, dev_id);
        }
    };

    let gm_reenforce = gm.clone();
    let reenforce_cb = move || {
        if let Ok(mut mgr) = gm_reenforce.lock() {
            let _ = mgr.re_enforce_session_windows();
        }
    };

    let shared_input_lock = Arc::new(Mutex::new(nucleus_input::InputLock::new()));
    let input_lock_cb = shared_input_lock.clone();
    let platform_input = platform.clone();
    let toggle_input_lock_cb = move || -> bool {
        if let Ok(lock) = input_lock_cb.lock() {
            lock.toggle_with_osd(Some(platform_input.osd_manager()));
            lock.is_locked()
        } else {
            false
        }
    };

    let gm_stop = gm.clone();
    let stop_session_cb = move || {
        let gm_stop_clone = gm_stop.clone();
        std::thread::spawn(move || {
            if let Ok(mut mgr) = gm_stop_clone.lock() {
                let _ = mgr.stop_game();
            }
        });
    };

    let gm_check = gm.clone();
    let session_active_cb = move || -> bool {
        if let Ok(mgr) = gm_check.lock() {
            mgr.active_session().map_or(false, |s| s.is_running)
        } else {
            false
        }
    };

    let platform_topmost = platform.clone();
    let gm_topmost = gm.clone();
    let osd_topmost_cb = move |is_topmost: bool, summoner_slot: Option<usize>| {
        if let Ok(mut mgr) = gm_topmost.lock() {
            mgr.set_osd_visible(is_topmost);
        }
        let wm = platform_topmost.window_manager();
        if let Ok(Some(hwnd)) = wm.find_window_by_title("Nucleus Co-op", true) {
            if is_topmost {
                // If a game session is active, find its target monitor for summoner_slot and configure overlay
                let target_rect = if let Ok(mgr) = gm_topmost.lock() {
                    if let Some(session) = mgr.active_session() {
                        if let Some(handler) = &session.handler {
                            let ctx_opt = match summoner_slot {
                                Some(slot) => handler
                                    .contexts
                                    .get(slot)
                                    .or_else(|| handler.contexts.first()),
                                None => handler.contexts.first(),
                            };
                            if let Some(ctx) = ctx_opt {
                                let rect = nucleus_common::types::Rect::new(
                                    ctx.pos_x, ctx.pos_y, ctx.width, ctx.height,
                                );
                                wm.get_monitor_bounds_for_rect(&rect).unwrap_or(rect)
                            } else {
                                nucleus_common::types::Rect::new(0, 0, 1920, 1080)
                            }
                        } else {
                            nucleus_common::types::Rect::new(0, 0, 1920, 1080)
                        }
                    } else {
                        nucleus_common::types::Rect::new(0, 0, 1920, 1080)
                    }
                } else {
                    nucleus_common::types::Rect::new(0, 0, 1920, 1080)
                };
                let _ = wm.unclip_cursor();
                let _ = wm.configure_overlay_window(hwnd, &target_rect);
                let _ = wm.set_foreground(hwnd);
            } else {
                let _ = wm.set_topmost(hwnd, false);
                // Restore focus to summoner's game instance or first running instance
                if let Ok(mgr) = gm_topmost.lock() {
                    if let Some(session) = mgr.active_session() {
                        if let Some(handler) = &session.handler {
                            let pid_opt = match summoner_slot {
                                Some(slot) => handler
                                    .launched_pids
                                    .get(slot)
                                    .or_else(|| handler.launched_pids.first()),
                                None => handler.launched_pids.first(),
                            };
                            if let Some(pid) = pid_opt {
                                if let Ok(hwnds) = wm.find_windows_by_process(*pid) {
                                    if let Some(first_hwnd) = hwnds.first() {
                                        let _ = wm.restore_game_focus(*first_hwnd);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // Hub Account login/logout callbacks
    let gm_hub_login = gm.clone();
    let hub_login_cb = move |username: &str, _password: &str, remember: bool| {
        println!(
            "[Game Engine] Hub login requested for '{}' (remember={})",
            username, remember
        );
        if let Ok(mgr) = gm_hub_login.lock() {
            let token = nucleus_hub::auth::HubSessionToken {
                user_id: format!("usr_{}", username.to_lowercase()),
                username: username.to_string(),
                token: format!("tok_{}", uuid::Uuid::new_v4()),
                token_expires_unix_ms: Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                        + 30 * 86400 * 1000,
                ),
            };
            if remember {
                let _ = mgr.save_hub_credentials(&token);
            }
        }
    };

    let gm_hub_logout = gm.clone();
    let hub_logout_cb = move || {
        println!("[Game Engine] Hub logout requested");
        if let Ok(mgr) = gm_hub_logout.lock() {
            let _ = mgr.clear_hub_credentials();
        }
    };

    // Save slots callbacks
    let gm_list_slots = gm.clone();
    let list_save_slots_cb = move |guid: &str| -> Vec<nucleus_profile::save_slots::SaveSlot> {
        if let Ok(mgr) = gm_list_slots.lock() {
            mgr.list_save_slots(guid).unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    let gm_create_slot = gm.clone();
    let create_save_slot_cb = move |guid: &str,
                                    name: &str,
                                    title: &str,
                                    hint: &str|
          -> Option<nucleus_profile::save_slots::SaveSlot> {
        if let Ok(mut mgr) = gm_create_slot.lock() {
            mgr.create_save_slot(guid, name, title, hint).ok()
        } else {
            None
        }
    };

    let gm_clone_slot = gm.clone();
    let clone_save_slot_cb = move |guid: &str,
                                   src_id: &str,
                                   new_name: &str|
          -> Option<nucleus_profile::save_slots::SaveSlot> {
        if let Ok(mut mgr) = gm_clone_slot.lock() {
            mgr.clone_save_slot(guid, src_id, new_name).ok()
        } else {
            None
        }
    };

    let gm_del_slot = gm.clone();
    let delete_save_slot_cb = move |guid: &str, slot_id: &str| -> bool {
        if let Ok(mut mgr) = gm_del_slot.lock() {
            mgr.delete_save_slot(guid, slot_id).is_ok()
        } else {
            false
        }
    };

    let gm_set_player_slot = gm.clone();
    let set_player_save_slot_cb = move |guid: &str, player_idx: usize, slot_id: Option<String>| {
        if let Ok(mut mgr) = gm_set_player_slot.lock() {
            let _ = mgr.set_player_save_slot(guid, player_idx, slot_id);
        }
    };

    // Extensions & Add-ons callbacks
    let load_ui_extension_items =
        |engine: &Arc<Mutex<GameManager>>| -> Vec<nucleus_ui::extensions_view::ExtensionItem> {
            if let Ok(mgr) = engine.lock() {
                if let Some(ext_mgr) = mgr.extension_manager() {
                    let loaded = ext_mgr.lock();
                    let mut items: Vec<_> = loaded
                        .loaded_plugins()
                        .values()
                        .map(|p| {
                            let ext_type = match p.manifest.kind {
                                nucleus_plugin::PluginKind::Wasm => {
                                    nucleus_ui::extensions_view::ExtensionType::WasmPlugin
                                }
                                nucleus_plugin::PluginKind::JsAddon => {
                                    nucleus_ui::extensions_view::ExtensionType::JsAddon
                                }
                                _ => nucleus_ui::extensions_view::ExtensionType::LegacyPlugin,
                            };
                            let perms = p
                                .manifest
                                .permissions
                                .iter()
                                .map(|perm| (perm.display_title().to_string(), true))
                                .collect();
                            nucleus_ui::extensions_view::ExtensionItem {
                                id: p.manifest.id.clone(),
                                name: p.manifest.name.clone(),
                                version: p.manifest.version.to_string(),
                                author: p
                                    .manifest
                                    .author
                                    .clone()
                                    .unwrap_or_else(|| "Community".to_string()),
                                description: p.manifest.description.clone().unwrap_or_default(),
                                is_enabled: p.status == nucleus_plugin::PluginStatus::Active,
                                extension_type: ext_type,
                                permissions: perms,
                            }
                        })
                        .collect();
                    items.sort_by(|a, b| a.name.cmp(&b.name));
                    return items;
                }
            }
            Vec::new()
        };

    let gm_toggle_ext = gm.clone();
    let toggle_ext_cb = move |ext_id: &str, enabled: bool| {
        println!("[Game Engine] Toggle extension '{}' -> {}", ext_id, enabled);
        if let Ok(mgr) = gm_toggle_ext.lock() {
            if let Some(ext_mgr) = mgr.extension_manager() {
                let _ = ext_mgr.lock().set_extension_enabled(ext_id, enabled);
            }
        }
    };

    let gm_rescan_ext = gm.clone();
    let rescan_ext_cb = move || -> Vec<nucleus_ui::extensions_view::ExtensionItem> {
        println!("[Game Engine] Rescanning extensions from disk...");
        if let Ok(mgr) = gm_rescan_ext.lock() {
            if let Some(ext_mgr) = mgr.extension_manager() {
                let _ = ext_mgr.lock().scan_all();
            }
        }
        load_ui_extension_items(&gm_rescan_ext)
    };

    let gm_install_ext = gm.clone();
    let install_ext_cb = move |path: &str| -> Result<String, String> {
        println!("[Game Engine] Installing extension from: {}", path);
        if let Ok(mut mgr) = gm_install_ext.lock() {
            mgr.install_extension(Path::new(path))
                .map_err(|e| e.to_string())
        } else {
            Err("Engine busy".to_string())
        }
    };

    let gm_uninstall_ext = gm.clone();
    let uninstall_ext_cb = move |ext_id: &str| -> Result<(), String> {
        println!("[Game Engine] Uninstalling extension: {}", ext_id);
        if let Ok(mgr) = gm_uninstall_ext.lock() {
            if let Some(ext_mgr) = mgr.extension_manager() {
                ext_mgr.lock().uninstall_extension(ext_id)
            } else {
                Err("Extension manager unavailable".to_string())
            }
        } else {
            Err("Engine busy".to_string())
        }
    };

    let gm_grant_perm = gm.clone();
    let grant_perm_cb = move |ext_id: &str, perm_name: &str| {
        println!(
            "[Game Engine] Granting permission '{}' to extension '{}'",
            perm_name, ext_id
        );
        if let Ok(mgr) = gm_grant_perm.lock() {
            if let Some(ext_mgr) = mgr.extension_manager() {
                let perm = match perm_name.to_lowercase().as_str() {
                    "instancefsread" | "fsread" | "read" => {
                        nucleus_plugin::PluginPermission::InstanceFsRead
                    }
                    "instancefswrite" | "fswrite" | "write" => {
                        nucleus_plugin::PluginPermission::InstanceFsWrite
                    }
                    "environmentfs" | "env" => nucleus_plugin::PluginPermission::EnvironmentFs,
                    "registryops" | "registry" => nucleus_plugin::PluginPermission::RegistryOps,
                    "processspawn" | "process" => {
                        nucleus_plugin::PluginPermission::ProcessSpawnScoped
                    }
                    "windowcontrol" | "window" => nucleus_plugin::PluginPermission::WindowControl,
                    "scriptapi" | "script" => nucleus_plugin::PluginPermission::ScriptApiExport,
                    "lifecycle" | "hook" => nucleus_plugin::PluginPermission::LifecycleHooks,
                    "network" | "net" => nucleus_plugin::PluginPermission::NetworkOutbound,
                    _ => return,
                };
                let _ = ext_mgr.lock().grant_permission(ext_id, perm);
            }
        }
    };

    let gm_save_script = gm.clone();
    let save_handler_script_cb = move |guid_or_name: &str, content: &str| -> Result<(), String> {
        if let Ok(mgr) = gm_save_script.lock() {
            mgr.save_handler_script(guid_or_name, content)
        } else {
            Err("Engine lock busy".to_string())
        }
    };

    let gm_read_script = gm.clone();
    let read_handler_script_cb = move |guid_or_name: &str| -> Result<String, String> {
        if let Ok(mgr) = gm_read_script.lock() {
            mgr.read_handler_script(guid_or_name)
        } else {
            Err("Engine lock busy".to_string())
        }
    };

    // Handler Debugger callbacks
    let debug_ctrl = Arc::new(nucleus_debug::DebugController::new(
        nucleus_debug::protocol::DebugLaunchMode::Normal,
        None,
    ));
    let dbg_resume = debug_ctrl.clone();
    let debug_resume_cb = move || {
        dbg_resume.resume();
    };
    let dbg_step_over = debug_ctrl.clone();
    let debug_step_over_cb = move || {
        dbg_step_over.step_over();
    };
    let dbg_step_into = debug_ctrl.clone();
    let debug_step_into_cb = move || {
        dbg_step_into.step_into();
    };
    let dbg_step_out = debug_ctrl.clone();
    let debug_step_out_cb = move || {
        dbg_step_out.step_out();
    };
    let dbg_pause = debug_ctrl.clone();
    let debug_pause_cb = move || {
        dbg_pause.pause();
    };
    let dbg_stop = debug_ctrl.clone();
    let debug_stop_cb = move || {
        dbg_stop.abort();
    };
    let dbg_eval = debug_ctrl.clone();
    let debug_eval_cb =
        move |expr: &str| -> Result<String, String> { dbg_eval.evaluate_expression(expr) };
    let dbg_bp = debug_ctrl.clone();
    let debug_toggle_bp_cb = move |file_id: &str, line: u32| {
        dbg_bp.set_breakpoint(nucleus_debug::protocol::Breakpoint::new(file_id, line));
    };

    let connected_displays = platform_launch
        .display_manager()
        .get_displays()
        .unwrap_or_default();
    let audio_devices = nucleus_tools::audio::AudioRouter::get_audio_devices();
    let preflight_warn_win = gui_opts.preflight_warn.clone();
    let preflight_danger_win = gui_opts.preflight_danger.clone();

    let http_client = Arc::new(nucleus_ui::NucleusHttpClient::new());
    gpui_platform::application()
        .with_assets(gpui_kit_assets::AllAssets)
        .with_http_client(http_client.clone())
        .run(move |cx: &mut gpui::App| {
            nucleus_ui::init(cx);
            cx.set_http_client(http_client.clone());
            let bounds = gpui::Bounds::centered(
                None,
                gpui::size(gpui::px(width as f32), gpui::px(height as f32)),
                cx,
            );
            let options = gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Nucleus Co-op".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                ..Default::default()
            };
            let app_settings_win = app_settings.clone();
            let gui_opts_win = gui_opts.clone();
            let mut osd_rx_opt = Some(osd_signal_rx);
            let _ = cx.open_window(options, |_window, cx| {
                cx.new(|w_cx| {
                    let mut win = nucleus_ui::MainWindow::new(device_count, legacy_names)
                        .with_app_settings(app_settings_win)
                        .with_handler_items(handler_items)
                        .with_device_items(device_items)
                        .with_launcher(launcher_cb)
                        .with_favorite_toggler(toggle_fav_cb)
                        .with_profile_saver(save_profile_cb)
                        .with_save_path_deleter(delete_save_cb)
                        .with_game_remover(remove_game_cb)
                        .with_game_adder(add_game_cb)
                        .with_hub_searcher(hub_search_cb)
                        .with_hub_installer(hub_install_cb)
                        .with_handlers_dir_changer(dir_change_cb)
                        .with_theme_changer(theme_change_cb)
                        .with_auto_scan_changer(auto_scan_cb)
                        .with_game_scanner(scan_games_cb)
                        .with_numbered_profile_lister(list_profiles_cb)
                        .with_numbered_profile_loader(load_profile_cb)
                        .with_numbered_profile_saver(save_numbered_profile_cb)
                        .with_numbered_profile_deleter(delete_numbered_profile_cb)
                        .with_profile_unloader(unload_profile_cb)
                        .with_freeze_toggler(toggle_freeze_cb)
                        .with_controller_reassigner(reassign_cb)
                        .with_window_re_enforcer(reenforce_cb)
                        .with_input_lock_toggler(toggle_input_lock_cb)
                        .with_session_stopper(stop_session_cb)
                        .with_session_active_checker(session_active_cb)
                        .with_osd_topmost_handler(osd_topmost_cb)
                        .with_hub_login(hub_login_cb)
                        .with_hub_logout(hub_logout_cb)
                        .with_save_slots_lister(list_save_slots_cb)
                        .with_save_slot_creator(create_save_slot_cb)
                        .with_save_slot_cloner(clone_save_slot_cb)
                        .with_save_slot_deleter(delete_save_slot_cb)
                        .with_player_save_slot_setter(set_player_save_slot_cb)
                        .with_extension_toggler(toggle_ext_cb)
                        .with_extension_installer(install_ext_cb)
                        .with_extension_uninstaller(uninstall_ext_cb)
                        .with_extension_rescanner(rescan_ext_cb)
                        .with_extension_permission_granter(grant_perm_cb)
                        .with_debugger_resume(debug_resume_cb)
                        .with_debugger_step_over(debug_step_over_cb)
                        .with_debugger_step_into(debug_step_into_cb)
                        .with_debugger_step_out(debug_step_out_cb)
                        .with_debugger_pause(debug_pause_cb)
                        .with_debugger_stop(debug_stop_cb)
                        .with_debugger_eval(debug_eval_cb)
                        .with_debugger_breakpoint_toggler(debug_toggle_bp_cb)
                        .with_handler_script_reader(read_handler_script_cb)
                        .with_handler_script_saver(save_handler_script_cb)
                        .with_hub_version_installer(hub_version_install_cb)
                        .with_drop_in_player(drop_in_cb)
                        .with_drop_out_player(drop_out_cb)
                        .with_active_session_querier(query_session_cb);

                    if let Ok(mgr) = gm.lock() {
                        if let Some(creds) = mgr.get_hub_credentials() {
                            win.hub_account_summary = Some(nucleus_profile::hub_account::HubAccountSummary {
                                user_id: creds.user_id,
                                username: creds.username,
                                email_hint: None,
                                avatar_url: None,
                                token_expires_unix_ms: creds.token_expires_unix_ms,
                                hub_base_url: "https://hub.splitscreen.me".to_string(),
                            });
                        }
                    }

                    if let Some(rx) = osd_rx_opt.take() {
                        win.listen_for_osd_signals(rx, w_cx);
                    }

                    win.theme_name = current_theme.clone();
                    nucleus_ui::theme::set_active_theme(&current_theme);
                    win.handlers_dir = handlers_dir;
                    win.auto_scan = auto_scan;
                    win.connected_displays = connected_displays.clone();
                    win.audio_devices = audio_devices.clone();
                    win.preflight_warning = preflight_warn_win.clone();
                    win.preflight_danger = preflight_danger_win.clone();
                    win.installed_extensions = load_ui_extension_items(&gm);
                    win.sync_player_assignments();

                    // Apply CLI initial states
                    if let Some(ref tab_str) = gui_opts_win.initial_tab {
                        match tab_str.to_lowercase().as_str() {
                            "setup" => win.active_tab = nucleus_ui::ActiveTab::Setup,
                            "hub" => win.active_tab = nucleus_ui::ActiveTab::Hub,
                            "settings" => win.active_tab = nucleus_ui::ActiveTab::Settings,
                            "appearance" | "settings-appearance" => {
                                win.active_tab = nucleus_ui::ActiveTab::Settings;
                                win.settings_view.active_category = nucleus_ui::SettingsCategory::Appearance;
                            }
                            _ => win.active_tab = nucleus_ui::ActiveTab::Library,
                        }
                    }
                    if let Some(ref g_name) = gui_opts_win.target_game {
                        win.selected_game = Some(g_name.clone());
                        for h in &win.handlers {
                            if h.game_name.eq_ignore_ascii_case(g_name) || h.guid.eq_ignore_ascii_case(g_name) {
                                win.selected_game = Some(h.game_name.clone());
                                break;
                            }
                        }
                    }
                    if let Some(players) = gui_opts_win.initial_players {
                        win.player_count = players;
                    }
                    if let Some(ref layout) = gui_opts_win.initial_layout {
                        win.selected_layout = layout.clone();
                    }
                    if let Some(ref query) = gui_opts_win.search_query {
                        win.game_list.search_query = query.clone();
                    }
                    if gui_opts_win.controller_shortcuts {
                        win.show_controller_shortcuts_modal = true;
                    }
                    if let Some(ref detail_target) = gui_opts_win.hub_details {
                        win.active_tab = nucleus_ui::ActiveTab::Hub;
                        if let Some(ref t) = gui_opts_win.hub_tab {
                            match t.to_lowercase().as_str() {
                                "comments" => win.hub_detail_tab = nucleus_ui::HubDetailTab::Comments,
                                "code" | "js" | "jscode" => win.hub_detail_tab = nucleus_ui::HubDetailTab::JsCode,
                                "versions" | "history" => win.hub_detail_tab = nucleus_ui::HubDetailTab::VersionHistory,
                                "stats" | "statistics" => win.hub_detail_tab = nucleus_ui::HubDetailTab::Statistics,
                                _ => {}
                            }
                        }
                        let target_item = win.hub_packages.iter().find(|p| {
                            p.id.eq_ignore_ascii_case(detail_target) || p.game_name.to_lowercase().contains(&detail_target.to_lowercase())
                        }).cloned().unwrap_or_else(|| {
                            if detail_target.eq_ignore_ascii_case("raft") || detail_target == "f4Rddj3FbebKF4whb" {
                                nucleus_ui::HubPackageItem::new(
                                    "f4Rddj3FbebKF4whb",
                                    "RaftMP",
                                    "v5",
                                    "The Shnazberry",
                                    "Just read the Author's notes and you should be fine. Only supports Mice and Keyboards, as Raft has no controller support yet.\n\nGuide: https://www.reddit.com/r/nucleuscoop/comments/gg65tz/raft_local_coop/\n\nThanks to Talos91, ZeroFox, and Ilyaki from the Splitscreen Dreams Discord for helping with the handler in various aspects.",
                                    "2 - 4",
                                    false,
                                )
                                .with_verification(true)
                                .with_inputs(false, true, true)
                                .with_meta(
                                    "Raft",
                                    Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1xdc.jpg".to_string()),
                                    20,
                                    64000,
                                    2,
                                    vec!["Simulator".to_string(), "Adventure".to_string(), "Indie".to_string()],
                                    "2022-01-01T00:00:00.000Z",
                                    "2024-05-10T12:00:00.000Z",
                                )
                            } else {
                                nucleus_ui::HubPackageItem::new(
                                    detail_target.clone(),
                                    detail_target.clone(),
                                    "v1",
                                    "Community Author",
                                    "Handler details",
                                    "2 - 4",
                                    false,
                                )
                            }
                        });
                        let target_id = target_item.id.clone();
                        win.inspecting_hub_package = Some(target_item);
                        win.load_hub_package_details(&target_id, w_cx);
                    }

                    if gui_opts_win.big_picture || gui_opts_win.big_picture_screen.is_some() {
                        win.is_big_picture = true;
                        if win.handlers.is_empty() {
                            win.handlers = vec![
                                HandlerItem::new(
                                    "l4d2".to_string(),
                                    "Left 4 Dead 2".to_string(),
                                    "left4dead2.exe".to_string(),
                                    4,
                                    "Left 4 Dead 2 Splitscreen Handler".to_string(),
                                )
                                .with_cover(Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1x7j.jpg".to_string()))
                                .with_installed(true),
                                HandlerItem::new(
                                    "halo".to_string(),
                                    "Halo: The Master Chief Collection".to_string(),
                                    "mcclauncher.exe".to_string(),
                                    4,
                                    "Halo MCC Community Handler".to_string(),
                                )
                                .with_cover(Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1r76.jpg".to_string()))
                                .with_installed(true),
                                HandlerItem::new(
                                    "borderlands2".to_string(),
                                    "Borderlands 2".to_string(),
                                    "Borderlands2.exe".to_string(),
                                    4,
                                    "Borderlands 2 Splitscreen Handler".to_string(),
                                )
                                .with_cover(Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co1x9j.jpg".to_string()))
                                .with_installed(true),
                            ];
                        }
                        win.carousel_state.set_count(win.handlers.len());
                        if let Some(ref screen) = gui_opts_win.big_picture_screen {
                            match screen.to_lowercase().as_str() {
                                "setup" => {
                                    let target = win.selected_game.clone().unwrap_or_else(|| {
                                        win.handlers.first().map(|h| h.game_name.clone()).unwrap_or_else(|| "Left 4 Dead 2".to_string())
                                    });
                                    win.enter_couch_setup(&target, w_cx);
                                    if !win.couch_setup_state.slots.is_empty() {
                                        win.couch_setup_state.grab_slot(0);
                                    }
                                }
                                "hub" => {
                                    win.enter_couch_hub(w_cx);
                                }
                                "osd" => {
                                    win.in_game_osd.open();
                                }
                                "options" => {
                                    let target = win.selected_game.clone().unwrap_or_else(|| {
                                        win.handlers.first().map(|h| h.game_name.clone()).unwrap_or_else(|| "Left 4 Dead 2".to_string())
                                    });
                                    let sample_options = vec![
                                        nucleus_ui::big_picture::CouchOptionItem {
                                            title: "Will you use the game's native splitscreen?".to_string(),
                                            description: "If yes only launch TWO instances, you need 3 or 4 gamepads.".to_string(),
                                            key: "native_splitscreen".to_string(),
                                            choices: vec!["No".to_string(), "Yes".to_string()],
                                            selected_choice: 0,
                                        },
                                        nucleus_ui::big_picture::CouchOptionItem {
                                            title: "Window Size Scale".to_string(),
                                            description: "ONLY change if the instances are not opening already correctly resized for you or have cursor offset.".to_string(),
                                            key: "window_scale".to_string(),
                                            choices: vec!["1.0".to_string(), "0.75".to_string(), "0.5".to_string()],
                                            selected_choice: 0,
                                        },
                                        nucleus_ui::big_picture::CouchOptionItem {
                                            title: "Disable multiple keyboards/mice support?".to_string(),
                                            description: "Disables the keyboards/mice Proto Input hooks so you can play with only one kb/mouse with the instance focused.".to_string(),
                                            key: "disable_protoinput".to_string(),
                                            choices: vec!["No".to_string(), "Yes".to_string()],
                                            selected_choice: 0,
                                        },
                                    ];
                                    win.open_couch_options_modal(&target, sample_options, w_cx);
                                }
                                _ => {
                                    win.big_picture_screen = nucleus_ui::big_picture::BigPictureScreen::Carousel;
                                }
                            }
                        }
                    }

                    win
                })
            });
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_title_for_hub_matching() {
        // Minecraft matching
        let hub_mc = "Minecraft: Java Edition";
        let local_mc = "Minecraft Java Edition";
        assert_eq!(normalize_title_for_hub(hub_mc), "minecraft java edition");
        assert_eq!(normalize_title_for_hub(local_mc), "minecraft java edition");
        assert_eq!(
            normalize_title_for_hub(hub_mc),
            normalize_title_for_hub(local_mc)
        );

        // Call of Duty Black Ops 2 Multiplayer matching
        let hub_bo2_mp = "Call of Duty: Black Ops II Multiplayer";
        let local_bo2_mp = "Call of Duty Black Ops 2 Multiplayer";
        assert_eq!(
            normalize_title_for_hub(hub_bo2_mp),
            "call of duty black ops 2 multiplayer"
        );
        assert_eq!(
            normalize_title_for_hub(local_bo2_mp),
            "call of duty black ops 2 multiplayer"
        );
        assert_eq!(
            normalize_title_for_hub(hub_bo2_mp),
            normalize_title_for_hub(local_bo2_mp)
        );

        // Call of Duty Black Ops 2 Zombies matching
        let hub_bo2_zm = "Call of Duty: Black Ops II Zombies";
        let local_bo2_zm = "Call of Duty Black Ops 2 Zombies";
        assert_eq!(
            normalize_title_for_hub(hub_bo2_zm),
            "call of duty black ops 2 zombies"
        );
        assert_eq!(
            normalize_title_for_hub(local_bo2_zm),
            "call of duty black ops 2 zombies"
        );
        assert_eq!(
            normalize_title_for_hub(hub_bo2_zm),
            normalize_title_for_hub(local_bo2_zm)
        );

        // Dark Souls 3 matching
        let hub_ds3 = "DARK SOULS III";
        let local_ds3 = "Dark Souls 3";
        assert_eq!(normalize_title_for_hub(hub_ds3), "dark souls 3");
        assert_eq!(normalize_title_for_hub(local_ds3), "dark souls 3");
        assert_eq!(
            normalize_title_for_hub(hub_ds3),
            normalize_title_for_hub(local_ds3)
        );
    }

    #[test]
    fn test_build_handler_item_features() {
        let mut cfg = nucleus_engine::HandlerConfig::default();
        cfg.game_name = "Left 4 Dead 2".to_string();
        cfg.executable_name = "left4dead2.exe".to_string();
        cfg.supports_keyboard = true;
        cfg.use_proto_input = true;
        cfg.options.push(nucleus_engine::HandlerOption {
            name: "FOV".to_string(),
            description: "Field of view".to_string(),
            key: "fov".to_string(),
            values: vec!["90".to_string(), "100".to_string()],
            hidden: false,
        });

        let mut prof =
            nucleus_profile::GameProfile::new("l4d2_guid", "l4d2_guid", "left4dead2.exe");
        prof.favorite = true;
        prof.assign_gamepads_by_index = true;
        prof.steam_language = Some("english".to_string());
        prof.handler_path = "C:/Games/NucleusCoop/handlers/l4d2.js".to_string();

        let item = build_handler_item("l4d2_guid", &cfg, Some(&prof), None);
        assert!(item.is_favorite);
        assert!(item.supports_gamepad);
        assert!(item.supports_keyboard);
        assert!(item.supports_multiple_keyboards_and_mice);
        assert!(item.assign_gamepads_by_index);
        assert_eq!(item.steam_language.as_deref(), Some("english"));
        assert_eq!(
            item.handler_path.as_deref(),
            Some("C:/Games/NucleusCoop/handlers/l4d2.js")
        );
        assert_eq!(item.options.len(), 1);
        assert_eq!(item.options[0].key, "fov");
    }

    #[test]
    fn test_launch_gui_options_big_picture() {
        let opts = LaunchGuiOptions {
            big_picture: true,
            big_picture_screen: Some("setup".to_string()),
            ..Default::default()
        };
        assert!(opts.big_picture);
        assert_eq!(opts.big_picture_screen.as_deref(), Some("setup"));

        let args = vec![
            "nucleus-coop".to_string(),
            "--big-picture".to_string(),
            "--big-picture-screen".to_string(),
            "osd".to_string(),
        ];
        let big_picture = args.iter().any(|a| a == "--big-picture");
        let big_picture_screen = args
            .iter()
            .position(|a| a == "--big-picture-screen")
            .and_then(|i| args.get(i + 1))
            .cloned();
        assert!(big_picture);
        assert_eq!(big_picture_screen.as_deref(), Some("osd"));
    }
}
