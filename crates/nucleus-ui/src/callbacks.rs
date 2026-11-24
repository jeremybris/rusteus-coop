//! Callback builder struct for `MainWindow` events and engine actions.

use crate::models::{HandlerItem, HubPackageItem};
use nucleus_engine::session::ActiveSessionSnapshot;
use nucleus_profile::GameProfile;
use std::collections::HashMap;
use std::sync::Arc;

/// Callback type for installing a specific version of a Community Hub handler package.
pub type HubVersionInstallFn = Arc<
    dyn Fn(&str, &str, i32, Option<u64>, Box<dyn Fn(f32) + Send + Sync>) -> Option<HandlerItem>
        + Send
        + Sync,
>;

/// Messages dispatched from the background engine thread to the GUI overlay controller.
#[derive(Debug)]
pub enum EngineToGui {
    /// Toggle the in-game overlay, optionally specifying which player slot summoned it.
    ToggleInGameOsd { summoner_slot: Option<usize> },
    /// Interactive launch prompt requiring user confirmation or countdown proceed.
    LaunchPrompt(crate::models::LaunchPromptRequest),
}

/// Holds all external callbacks invoked by `MainWindow` when user actions,
/// background engine commands, or profile updates occur.
#[derive(Clone, Default)]
#[allow(clippy::type_complexity)]
pub struct MainWindowCallbacks {
    /// Optional callback invoked when toggling favorite state.
    pub on_toggle_favorite: Option<Arc<dyn Fn(&str, bool) + Send + Sync>>,
    /// Optional callback invoked when saving/updating profile properties.
    pub on_save_profile: Option<Arc<dyn Fn(&HandlerItem) + Send + Sync>>,
    /// Optional callback invoked to delete UserProfile save path.
    pub on_delete_save_path: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Optional callback invoked to remove a game from the list.
    pub on_remove_game: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Optional callback invoked when the user starts a game session from the UI.
    pub on_launch: Option<
        Arc<
            dyn Fn(&str, usize, &str, &HashMap<String, String>, &[String], &[usize], &[String])
                + Send
                + Sync,
        >,
    >,
    /// Optional callback invoked when a new game .exe is added via the file explorer dialog.
    pub on_add_game: Option<Arc<dyn Fn(&str) -> Option<HandlerItem> + Send + Sync>>,
    /// Optional callback invoked when searching the Community Hub.
    pub on_search_hub: Option<Arc<dyn Fn(&str) -> Vec<HubPackageItem> + Send + Sync>>,
    /// Optional callback invoked when downloading/installing a Community Hub package with progress.
    pub on_install_handler: Option<
        Arc<dyn Fn(&str, Box<dyn Fn(f32) + Send + Sync>) -> Option<HandlerItem> + Send + Sync>,
    >,
    /// Optional callback invoked when downloading/installing a specific Community Hub package version with progress.
    pub on_install_handler_version: Option<HubVersionInstallFn>,
    /// Optional callback invoked when a new player drops into an active session (`(device_assignment, layout_name) -> Ok(slot_idx)`).
    pub on_drop_in_player: Option<Arc<dyn Fn(&str, &str) -> Result<usize, String> + Send + Sync>>,
    /// Optional callback invoked when an active player drops out (`(slot_idx, layout_name) -> Ok(remaining_count)`).
    pub on_drop_out_player: Option<Arc<dyn Fn(usize, &str) -> Result<usize, String> + Send + Sync>>,
    /// Optional callback to query a live snapshot of the active game session for the In-Game OSD.
    pub on_query_active_session:
        Option<Arc<dyn Fn() -> Option<ActiveSessionSnapshot> + Send + Sync>>,
    /// Optional callback invoked when changing the handlers directory.
    pub on_handlers_dir_changed: Option<Arc<dyn Fn(&str) -> Vec<HandlerItem> + Send + Sync>>,
    /// Optional callback invoked when visual theme is changed.
    pub on_theme_changed: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Optional callback to scan local storage drives for installed games matching discovered handlers.
    pub on_scan_games: Option<Arc<dyn Fn() -> Vec<HandlerItem> + Send + Sync>>,
    /// Optional callback invoked when auto-scan toggle is changed.
    pub on_auto_scan_changed: Option<Arc<dyn Fn(bool) + Send + Sync>>,
    /// Optional callback to list saved numbered profiles for a game GUID: returns Vec<(profile_number, title)>.
    pub on_list_profiles: Option<Arc<dyn Fn(&str) -> Vec<(usize, String)> + Send + Sync>>,
    /// Optional callback to load a numbered profile: returns GameProfile if found.
    pub on_load_numbered_profile:
        Option<Arc<dyn Fn(&str, usize) -> Option<GameProfile> + Send + Sync>>,
    /// Optional callback to save a numbered profile: takes (guid, Option<profile_number>, &GameProfile) and returns assigned profile_number.
    pub on_save_numbered_profile:
        Option<Arc<dyn Fn(&str, Option<usize>, &GameProfile) -> Option<usize> + Send + Sync>>,
    /// Optional callback to delete a numbered profile.
    pub on_delete_numbered_profile: Option<Arc<dyn Fn(&str, usize) -> bool + Send + Sync>>,
    /// Optional callback to unload the currently loaded profile.
    pub on_unload_profile: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback invoked when toggling instance freeze mid-game.
    pub on_toggle_freeze: Option<Arc<dyn Fn() -> Option<bool> + Send + Sync>>,
    /// Optional callback invoked when reassigning a controller mid-game.
    pub on_reassign_controller: Option<Arc<dyn Fn(usize, usize) + Send + Sync>>,
    /// Optional callback invoked when re-enforcing window boundaries.
    pub on_re_enforce_windows: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback invoked when toggling input lock.
    pub on_toggle_input_lock: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    /// Optional callback invoked when stopping active session mid-game.
    pub on_stop_session: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback to check if the background engine has an active session.
    pub on_check_session_active: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    /// Optional callback to toggle the launcher window's topmost state when OSD menu opens/closes,
    /// targeting the monitor of the player who summoned the overlay.
    pub on_toggle_osd_topmost: Option<Arc<dyn Fn(bool, Option<usize>) + Send + Sync>>,
    /// Optional callback for Hub account login: `(username, password, remember)`.
    pub on_hub_login: Option<Arc<dyn Fn(&str, &str, bool) + Send + Sync>>,
    /// Optional callback for Hub account logout.
    pub on_hub_logout: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback to list save slots for a game GUID.
    pub on_list_save_slots:
        Option<Arc<dyn Fn(&str) -> Vec<nucleus_profile::save_slots::SaveSlot> + Send + Sync>>,
    /// Optional callback to create a save slot: `(guid, display_name, game_title, hint)`.
    pub on_create_save_slot: Option<
        Arc<
            dyn Fn(&str, &str, &str, &str) -> Option<nucleus_profile::save_slots::SaveSlot>
                + Send
                + Sync,
        >,
    >,
    /// Optional callback to clone a save slot: `(guid, source_id, new_name)`.
    pub on_clone_save_slot: Option<
        Arc<
            dyn Fn(&str, &str, &str) -> Option<nucleus_profile::save_slots::SaveSlot> + Send + Sync,
        >,
    >,
    /// Optional callback to delete a save slot: `(guid, slot_id)`.
    pub on_delete_save_slot: Option<Arc<dyn Fn(&str, &str) -> bool + Send + Sync>>,
    /// Optional callback to assign a save slot to a player slot: `(guid, player_idx, Option<slot_id>)`.
    pub on_set_player_save_slot: Option<Arc<dyn Fn(&str, usize, Option<String>) + Send + Sync>>,
    /// Optional callback to toggle an extension enable/disable state: `(id, enabled)`.
    pub on_toggle_extension: Option<Arc<dyn Fn(&str, bool) + Send + Sync>>,
    /// Optional callback to install an extension archive from path.
    pub on_install_extension: Option<Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
    /// Optional callback to rescan extensions from disk: returns updated list.
    pub on_rescan_extensions:
        Option<Arc<dyn Fn() -> Vec<crate::extensions_view::ExtensionItem> + Send + Sync>>,
    /// Optional callback to grant a permission to an extension: `(id, perm_name)`.
    pub on_grant_extension_permission: Option<Arc<dyn Fn(&str, &str) + Send + Sync>>,
    /// Optional callback to uninstall an extension: `(id)`.
    pub on_uninstall_extension: Option<Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>>,
    /// Optional callback for debugger resume.
    pub on_debug_resume: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback for debugger step over.
    pub on_debug_step_over: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback for debugger step into.
    pub on_debug_step_into: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback for debugger step out.
    pub on_debug_step_out: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback for debugger pause.
    pub on_debug_pause: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback for debugger stop.
    pub on_debug_stop: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional callback for debugger REPL evaluate: `(expr) -> Result<String, String>`.
    pub on_debug_eval: Option<Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
    /// Optional callback for toggling breakpoint: `(file_id, line)`.
    pub on_debug_toggle_breakpoint: Option<Arc<dyn Fn(&str, u32) + Send + Sync>>,
    /// Optional callback to read handler script source code: `(guid_or_name) -> Result<String, String>`.
    pub on_read_handler_script: Option<Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
    /// Optional callback to save handler script source code: `(guid_or_name, code) -> Result<(), String>`.
    pub on_save_handler_script: Option<Arc<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>>,
}

impl MainWindowCallbacks {
    /// Creates an empty callbacks builder with all callbacks unset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the launch callback for starting games from the GUI.
    pub fn with_launcher<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize, &str, &HashMap<String, String>, &[String], &[usize], &[String])
            + Send
            + Sync
            + 'static,
    {
        self.on_launch = Some(Arc::new(callback));
        self
    }

    /// Sets callback for toggling favorite status.
    pub fn with_favorite_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, bool) + Send + Sync + 'static,
    {
        self.on_toggle_favorite = Some(Arc::new(callback));
        self
    }

    /// Sets callback for saving profile properties.
    pub fn with_profile_saver<F>(mut self, callback: F) -> Self
    where
        F: Fn(&HandlerItem) + Send + Sync + 'static,
    {
        self.on_save_profile = Some(Arc::new(callback));
        self
    }

    /// Sets callback for deleting UserProfile save directory.
    pub fn with_save_path_deleter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_delete_save_path = Some(Arc::new(callback));
        self
    }

    /// Sets callback for removing a game from the library.
    pub fn with_game_remover<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_remove_game = Some(Arc::new(callback));
        self
    }

    /// Sets callback for adding a game by executable path.
    pub fn with_game_adder<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Option<HandlerItem> + Send + Sync + 'static,
    {
        self.on_add_game = Some(Arc::new(callback));
        self
    }

    /// Sets callback for searching the community hub.
    pub fn with_hub_searcher<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<HubPackageItem> + Send + Sync + 'static,
    {
        self.on_search_hub = Some(Arc::new(callback));
        self
    }

    /// Sets callback for installing a hub handler package with progress reporting.
    pub fn with_hub_installer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, Box<dyn Fn(f32) + Send + Sync>) -> Option<HandlerItem> + Send + Sync + 'static,
    {
        self.on_install_handler = Some(Arc::new(callback));
        self
    }

    /// Sets callback for updating the handlers directory.
    pub fn with_handlers_dir_changer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<HandlerItem> + Send + Sync + 'static,
    {
        self.on_handlers_dir_changed = Some(Arc::new(callback));
        self
    }

    /// Sets callback for saving theme changes.
    pub fn with_theme_changer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_theme_changed = Some(Arc::new(callback));
        self
    }

    /// Sets callback for auto scan toggles.
    pub fn with_auto_scan_changer<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.on_auto_scan_changed = Some(Arc::new(callback));
        self
    }

    /// Sets callback for scanning local storage drives for supported game installations.
    pub fn with_game_scanner<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Vec<HandlerItem> + Send + Sync + 'static,
    {
        self.on_scan_games = Some(Arc::new(callback));
        self
    }

    /// Sets callback for listing saved numbered profiles for a game.
    pub fn with_numbered_profile_lister<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<(usize, String)> + Send + Sync + 'static,
    {
        self.on_list_profiles = Some(Arc::new(callback));
        self
    }

    /// Sets callback for loading a numbered profile.
    pub fn with_numbered_profile_loader<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize) -> Option<GameProfile> + Send + Sync + 'static,
    {
        self.on_load_numbered_profile = Some(Arc::new(callback));
        self
    }

    /// Sets callback for saving a numbered profile.
    pub fn with_numbered_profile_saver<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, Option<usize>, &GameProfile) -> Option<usize> + Send + Sync + 'static,
    {
        self.on_save_numbered_profile = Some(Arc::new(callback));
        self
    }

    /// Sets callback for deleting a numbered profile.
    pub fn with_numbered_profile_deleter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize) -> bool + Send + Sync + 'static,
    {
        self.on_delete_numbered_profile = Some(Arc::new(callback));
        self
    }

    /// Sets callback for unloading the active profile.
    pub fn with_profile_unloader<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_unload_profile = Some(Arc::new(callback));
        self
    }

    /// Sets callback for toggling session freeze.
    pub fn with_freeze_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Option<bool> + Send + Sync + 'static,
    {
        self.on_toggle_freeze = Some(Arc::new(callback));
        self
    }

    /// Sets callback for hot-swapping controller assignment mid-game.
    pub fn with_controller_reassigner<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        self.on_reassign_controller = Some(Arc::new(callback));
        self
    }

    /// Sets callback for re-enforcing window bounds.
    pub fn with_window_re_enforcer<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_re_enforce_windows = Some(Arc::new(callback));
        self
    }

    /// Sets callback for toggling raw input lock.
    pub fn with_input_lock_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.on_toggle_input_lock = Some(Arc::new(callback));
        self
    }

    /// Sets callback for stopping an active session.
    pub fn with_session_stopper<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_stop_session = Some(Arc::new(callback));
        self
    }

    /// Sets callback to check if a session is actively running in the engine.
    pub fn with_session_active_checker<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.on_check_session_active = Some(Arc::new(callback));
        self
    }

    /// Sets callback to toggle the window's TOPMOST status when in-game OSD opens or closes.
    pub fn with_osd_topmost_handler<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool, Option<usize>) + Send + Sync + 'static,
    {
        self.on_toggle_osd_topmost = Some(Arc::new(callback));
        self
    }

    /// Sets callback for installing a specific Hub handler package version with progress.
    pub fn with_hub_version_installer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, i32, Option<u64>, Box<dyn Fn(f32) + Send + Sync>) -> Option<HandlerItem>
            + Send
            + Sync
            + 'static,
    {
        self.on_install_handler_version = Some(Arc::new(callback));
        self
    }

    /// Sets callback for hot-adding a Drop-In player into an active split-screen session.
    pub fn with_drop_in_handler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) -> Result<usize, String> + Send + Sync + 'static,
    {
        self.on_drop_in_player = Some(Arc::new(callback));
        self
    }

    /// Sets callback for cleanly dropping out an active player slot mid-session.
    pub fn with_drop_out_handler<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, &str) -> Result<usize, String> + Send + Sync + 'static,
    {
        self.on_drop_out_player = Some(Arc::new(callback));
        self
    }

    /// Sets callback to query a live `ActiveSessionSnapshot` from the background engine.
    pub fn with_active_session_querier<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Option<ActiveSessionSnapshot> + Send + Sync + 'static,
    {
        self.on_query_active_session = Some(Arc::new(callback));
        self
    }

    /// Sets callback for Hub account login.
    pub fn with_hub_login<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, bool) + Send + Sync + 'static,
    {
        self.on_hub_login = Some(Arc::new(callback));
        self
    }

    /// Sets callback for Hub account logout.
    pub fn with_hub_logout<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_hub_logout = Some(Arc::new(callback));
        self
    }

    /// Sets callback for listing save slots.
    pub fn with_save_slots_lister<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<nucleus_profile::save_slots::SaveSlot> + Send + Sync + 'static,
    {
        self.on_list_save_slots = Some(Arc::new(callback));
        self
    }

    /// Sets callback for creating a save slot.
    pub fn with_save_slot_creator<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, &str, &str) -> Option<nucleus_profile::save_slots::SaveSlot>
            + Send
            + Sync
            + 'static,
    {
        self.on_create_save_slot = Some(Arc::new(callback));
        self
    }

    /// Sets callback for cloning a save slot.
    pub fn with_save_slot_cloner<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, &str) -> Option<nucleus_profile::save_slots::SaveSlot>
            + Send
            + Sync
            + 'static,
    {
        self.on_clone_save_slot = Some(Arc::new(callback));
        self
    }

    /// Sets callback for deleting a save slot.
    pub fn with_save_slot_deleter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        self.on_delete_save_slot = Some(Arc::new(callback));
        self
    }

    /// Sets callback for assigning a player save slot.
    pub fn with_player_save_slot_setter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize, Option<String>) + Send + Sync + 'static,
    {
        self.on_set_player_save_slot = Some(Arc::new(callback));
        self
    }

    /// Sets callback for toggling an extension.
    pub fn with_extension_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, bool) + Send + Sync + 'static,
    {
        self.on_toggle_extension = Some(Arc::new(callback));
        self
    }

    /// Sets callback for installing an extension.
    pub fn with_extension_installer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.on_install_extension = Some(Arc::new(callback));
        self
    }

    /// Sets callback for rescanning extensions.
    pub fn with_extension_rescanner<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Vec<crate::extensions_view::ExtensionItem> + Send + Sync + 'static,
    {
        self.on_rescan_extensions = Some(Arc::new(callback));
        self
    }

    /// Sets callback for granting an extension permission.
    pub fn with_extension_permission_granter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) + Send + Sync + 'static,
    {
        self.on_grant_extension_permission = Some(Arc::new(callback));
        self
    }

    /// Sets callback for uninstalling an extension.
    pub fn with_extension_uninstaller<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.on_uninstall_extension = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger resume.
    pub fn with_debugger_resume<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_debug_resume = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger step over.
    pub fn with_debugger_step_over<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_debug_step_over = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger step into.
    pub fn with_debugger_step_into<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_debug_step_into = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger step out.
    pub fn with_debugger_step_out<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_debug_step_out = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger pause.
    pub fn with_debugger_pause<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_debug_pause = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger stop.
    pub fn with_debugger_stop<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_debug_stop = Some(Arc::new(callback));
        self
    }

    /// Sets callback for debugger expression evaluation.
    pub fn with_debugger_eval<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.on_debug_eval = Some(Arc::new(callback));
        self
    }

    /// Sets callback for toggling breakpoint.
    pub fn with_debugger_breakpoint_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, u32) + Send + Sync + 'static,
    {
        self.on_debug_toggle_breakpoint = Some(Arc::new(callback));
        self
    }

    /// Sets callback for reading handler script source code.
    pub fn with_handler_script_reader<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.on_read_handler_script = Some(Arc::new(callback));
        self
    }

    /// Sets callback for saving handler script source code.
    pub fn with_handler_script_saver<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.on_save_handler_script = Some(Arc::new(callback));
        self
    }
}
