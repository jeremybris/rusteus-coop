//! Native GPUI application window component for NucleusCoop.

use crate::big_picture::{
    BigPictureScreen, CarouselState, CouchHubState, CouchOptionItem, CouchSetupState,
    PathResolution, resolve_game_path_or_couch_prompt,
};
use crate::callbacks::MainWindowCallbacks;
use crate::components::{
    CouchConfirmDialog, CouchContextMenu, CouchSidebarNav, CouchStatusBar, CouchTitleBar,
};
use crate::models::{DeviceItem, HandlerItem, HubPackageItem};
use crate::theme;
use crate::{Icon, IconName};
use gpui::prelude::*;
use gpui::*;

/// Active top-level navigation view tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    /// Game library and discovered handlers catalog.
    #[default]
    Library,
    /// Player count, monitor partitioning, and device assignment setup.
    Setup,
    /// Community handler hub script browser.
    Hub,
    /// Extensions and add-ons ecosystem manager.
    Extensions,
    /// Step-by-step handler debugger and runtime inspector.
    Debugger,
    /// Global application preferences and directories.
    Settings,
}

/// Sort option for Hub browser packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HubSortOption {
    #[default]
    Trending,
    Hottest,
    Downloads,
    ReleaseDate,
    Alphabetical,
}

/// Top-level application window rendered with GPUI Kit components.
pub struct MainWindow {
    /// Active navigation tab.
    pub active_tab: ActiveTab,
    /// Number of detected physical game controllers.
    pub detected_devices: usize,
    /// Detailed list of detected physical devices.
    pub detected_devices_list: Vec<DeviceItem>,
    /// List of legacy handler names for backward compatibility.
    pub handler_names: Vec<String>,
    /// Detailed list of installed/discovered game handlers.
    pub handlers: Vec<HandlerItem>,
    /// Currently selected game title or GUID for splitscreen configuration.
    pub selected_game: Option<String>,
    /// Configured player count for splitscreen session.
    pub player_count: usize,
    /// Configured monitor partition layout name.
    pub selected_layout: String,
    /// Per-player device assignment labels (e.g. "Gamepad #0 (Xbox)", "Keyboard & Mouse").
    pub player_device_assignments: Vec<String>,
    /// Query string for hub search.
    pub hub_search_query: String,
    /// Available community packages loaded from Hub API or cache.
    pub hub_packages: Vec<HubPackageItem>,
    /// Active page index in community hub browser.
    pub hub_page: usize,
    /// Number of items per page in community hub browser.
    pub hub_page_size: usize,
    /// Active Hub sorting option.
    pub hub_sort: HubSortOption,
    /// Whether Hub sorting is ascending.
    pub hub_sort_ascending: bool,
    /// Active filter tags in Community Hub.
    pub selected_tags: std::collections::HashSet<String>,
    /// Whether Hub packages have been loaded automatically.
    pub hub_loaded: bool,
    /// Currently inspected Hub package for details modal.
    pub inspecting_hub_package: Option<HubPackageItem>,
    /// Active tab in Hub package details view.
    pub hub_detail_tab: crate::hub_details::HubDetailTab,
    /// Loaded comments for the currently inspected Hub package.
    pub hub_detail_comments: Vec<nucleus_hub::HubComment>,
    /// Loaded package releases/versions for the currently inspected Hub package.
    pub hub_detail_packages: Vec<nucleus_hub::HubPackage>,
    /// Whether details (comments, versions, js code) are currently being fetched.
    pub hub_detail_loading: bool,
    /// Extra likes/hotness added in current session for this package.
    pub hub_detail_extra_likes: std::collections::HashSet<String>,
    /// Currently inspected local handler for details modal.
    pub inspecting_handler: Option<HandlerItem>,
    /// Configured path for game handler scripts.
    pub handlers_dir: String,
    /// Whether automatic game scanning on startup is enabled.
    pub auto_scan: bool,
    /// Name of the active visual theme.
    pub theme_name: String,
    /// Current status bar message.
    pub status_message: String,
    /// Full persistent application settings.
    pub app_settings: nucleus_profile::AppSettings,
    /// Active OSD overlay message (text, start_time, duration).
    pub osd_message: Option<(String, std::time::Instant, std::time::Duration)>,
    /// Focus handle for Community Hub search input.
    pub hub_search_focus: Option<FocusHandle>,
    /// Focus handle for Game Library search input.
    pub lib_search_focus: Option<FocusHandle>,
    /// Focus handle for interactive physical keypress recording.
    pub hotkey_record_focus: Option<FocusHandle>,
    /// Focus handle for top-level window key events.
    pub window_focus: Option<FocusHandle>,
    /// Map of package ID to download/install progress (0.0 to 1.0).
    pub installing_handlers: std::collections::HashMap<String, f32>,
    /// Set of package IDs that have been installed in this session.
    pub installed_package_ids: std::collections::HashSet<String>,
    /// Embedded library view model.
    pub game_list: crate::GameListView,
    /// Embedded setup screen view model.
    pub setup_screen: crate::SetupScreenView,
    /// Embedded hub browser view model.
    pub hub_browser: crate::HubBrowserView,
    /// Embedded settings view model.
    pub settings_view: crate::SettingsView,
    /// Active library sort criterion.
    pub library_sort: crate::models::LibrarySortOption,
    /// Active library sort direction.
    pub library_sort_dir: crate::models::LibrarySortDirection,
    /// Whether a Community Hub API search or load is currently in-flight.
    pub hub_loading: bool,
    /// Currently opened right-click contextual menu handler.
    pub context_menu_handler: Option<HandlerItem>,
    /// Currently selected custom options map (key -> selected option value).
    pub selected_handler_options: std::collections::HashMap<String, String>,
    /// Active modal confirmation dialog if displayed.
    pub confirm_dialog: Option<crate::models::ConfirmDialogState>,
    /// External event and action callbacks.
    pub callbacks: MainWindowCallbacks,
    /// Saved numbered profiles for the currently configured game: list of (profile_number, title).
    pub saved_profiles: Vec<(usize, String)>,
    /// Active loaded profile number if one is loaded.
    pub active_profile_id: Option<usize>,
    /// Whether the left navigation sidebar is collapsed into icon-only mode (None = follow window width breakpoint).
    pub sidebar_collapsed: Option<bool>,
    /// Player slot index currently displaying device selection dropdown.
    pub dropdown_player_device: Option<usize>,
    /// Whether the context menu Steam language picker is expanded.
    pub context_menu_language_picker: bool,
    /// Whether the controller shortcuts setup modal is currently open.
    pub show_controller_shortcuts_modal: bool,
    /// Action key currently being remapped in the controller shortcuts modal (e.g. Some("unfocus")).
    pub remapping_controller_shortcut: Option<String>,
    /// Whether Couch Big Picture mode is currently active.
    pub is_big_picture: bool,
    /// Currently active screen in Couch Big Picture mode.
    pub big_picture_screen: BigPictureScreen,
    /// Horizontal carousel state for library browsing in Couch Big Picture mode.
    pub carousel_state: CarouselState,
    /// Couch Splitscreen Setup state (auto-assignment, swap motion, unassigned tray).
    pub couch_setup_state: CouchSetupState,
    /// Couch Community Hub browsing state and modals.
    pub couch_hub_state: CouchHubState,
    /// In-game Couch OSD overlay state.
    pub in_game_osd: crate::big_picture::overlay::InGameOsdState,
    /// Whether a splitscreen game session is actively executing.
    pub session_active: bool,
    /// Connected displays detected by the platform display manager.
    pub connected_displays: Vec<nucleus_common::types::Display>,
    /// Display index assignments per player (0-based monitor index).
    pub player_monitor_assignments: Vec<usize>,
    /// Available live audio output endpoints: list of (device_id, friendly_name).
    pub audio_devices: Vec<(String, String)>,
    /// Assigned audio device IDs per player instance.
    pub player_audio_device_assignments: Vec<String>,
    /// Active interactive launch prompt request modal, if one is awaiting user response.
    pub active_launch_prompt: Option<crate::models::LaunchPromptRequest>,
    /// Startup pre-flight warning message, if any.
    pub preflight_warning: Option<String>,
    /// Startup pre-flight danger/error message, if any.
    pub preflight_danger: Option<String>,
    /// Hub account modal state.
    pub hub_account_modal: crate::hub_account_modal::HubAccountModalState,
    /// Authenticated Hub account summary if logged in.
    pub hub_account_summary: Option<nucleus_profile::hub_account::HubAccountSummary>,
    /// Save slots modal state.
    pub save_slots_modal: crate::save_slots_modal::SaveSlotsModalState,
    /// Active save slots for currently viewed game.
    pub current_save_slots: Vec<nucleus_profile::save_slots::SaveSlot>,
    /// Assigned save slot IDs per player slot (0-indexed).
    pub player_save_slots: Vec<Option<String>>,
    /// Extensions view state.
    pub extensions_view_state: crate::extensions_view::ExtensionsViewState,
    /// Installed extensions list for the UI.
    pub installed_extensions: Vec<crate::extensions_view::ExtensionItem>,
    /// Handler debugger view state.
    pub debugger_view_state: crate::debugger_view::DebuggerViewState,
    /// Active sub-tab in Big Picture Settings screen (0 = General, 1 = Controllers).
    pub big_picture_settings_tab: usize,
    /// Controller GUID currently being tested/identified (for pulse/rumble effect).
    pub identifying_controller_guid: Option<String>,
    /// Whether an asynchronous disk game scan is currently running.
    pub is_scanning_games: bool,
    /// Focus index for Screen 7 (Game Details): 0 = Set up session, 1 = Load saved profile, 2 = Manage game, 3 = Breadcrumb.
    pub details_focus_index: usize,
    /// Focus index for Screen 1 (Game Management): 0..8.
    pub management_focus_index: usize,
    /// Focus index for Screens 2 & 3 (Sessions): action or profile index.
    pub sessions_focus_index: usize,
    /// Focus zone for Screens 4 & 5 (Settings): 0 = Sidebar tabs, 1 = Settings content.
    pub settings_focus_zone: usize,
    /// Focus item index within active settings panel (0..N).
    pub settings_item_index: usize,
    /// Focus zone for Screen 8 (Couch Hub / Add games): 0 = Top action cards, 1 = Handlers list, 2 = Detail pane action.
    pub hub_focus_zone: usize,
    /// Top action card focus for Screen 8 (0 = Scan for games, 1 = Browse files).
    pub hub_top_card_index: usize,
}

impl MainWindow {
    /// Creates a new `MainWindow` component with detected hardware and handlers.
    pub fn new(detected_devices: usize, handler_names: Vec<String>) -> Self {
        let selected_game = handler_names.first().cloned();
        let initial_player_count = std::cmp::max(detected_devices, 2);

        let mut setup_screen = crate::SetupScreenView::new();
        setup_screen.set_player_count(initial_player_count);

        let mut game_list = crate::GameListView::new();
        if let Some(ref g) = selected_game {
            game_list.select_game(Some(g.clone()));
        }

        let detected_devices_list = Vec::new();

        let handlers: Vec<HandlerItem> = handler_names
            .iter()
            .map(|name| {
                HandlerItem::new(
                    name.to_lowercase().replace(' ', "-"),
                    name.clone(),
                    format!("{}.exe", name.to_lowercase().replace(' ', "")),
                    4,
                    format!("Official handler script for {}. Features multi-instance save isolation, custom viewport calculations, and gamepad focus hooks.", name),
                )
            })
            .collect();

        let mut win = Self {
            active_tab: ActiveTab::Library,
            detected_devices,
            detected_devices_list,
            handler_names,
            handlers,
            selected_game,
            player_count: initial_player_count,
            selected_layout: "2x1 (Side-by-Side)".to_string(),
            player_device_assignments: Vec::new(),
            hub_search_query: String::new(),
            hub_packages: Vec::new(),
            hub_page: 0,
            hub_page_size: 20,
            hub_sort: HubSortOption::Trending,
            hub_sort_ascending: false,
            selected_tags: std::collections::HashSet::new(),
            hub_loaded: false,
            inspecting_hub_package: None,
            hub_detail_tab: crate::hub_details::HubDetailTab::default(),
            hub_detail_comments: Vec::new(),
            hub_detail_packages: Vec::new(),
            hub_detail_loading: false,
            hub_detail_extra_likes: std::collections::HashSet::new(),
            inspecting_handler: None,
            handlers_dir: "handlers".to_string(),
            auto_scan: true,
            theme_name: "Dark".to_string(),
            status_message: "Ready. Select a game to configure splitscreen.".to_string(),
            app_settings: nucleus_profile::AppSettings::default(),
            osd_message: None,
            hub_search_focus: None,
            lib_search_focus: None,
            hotkey_record_focus: None,
            window_focus: None,
            installing_handlers: std::collections::HashMap::new(),
            installed_package_ids: std::collections::HashSet::new(),
            game_list,
            setup_screen,
            hub_browser: crate::HubBrowserView::new(),
            settings_view: crate::SettingsView::new(),
            library_sort: crate::models::LibrarySortOption::Alphabetical,
            library_sort_dir: crate::models::LibrarySortDirection::Ascending,
            hub_loading: false,
            context_menu_handler: None,
            selected_handler_options: std::collections::HashMap::new(),
            callbacks: MainWindowCallbacks::default(),
            saved_profiles: Vec::new(),
            active_profile_id: None,
            sidebar_collapsed: None,
            confirm_dialog: None,
            dropdown_player_device: None,
            context_menu_language_picker: false,
            show_controller_shortcuts_modal: false,
            remapping_controller_shortcut: None,
            is_big_picture: false,
            big_picture_screen: BigPictureScreen::Carousel,
            carousel_state: CarouselState::new(0),
            couch_setup_state: CouchSetupState::new(),
            couch_hub_state: CouchHubState::new(),
            in_game_osd: crate::big_picture::overlay::InGameOsdState::new(),
            session_active: false,
            connected_displays: Vec::new(),
            player_monitor_assignments: Vec::new(),
            audio_devices: Vec::new(),
            player_audio_device_assignments: Vec::new(),
            active_launch_prompt: None,
            preflight_warning: None,
            preflight_danger: None,
            hub_account_modal: crate::hub_account_modal::HubAccountModalState::new(),
            hub_account_summary: None,
            save_slots_modal: crate::save_slots_modal::SaveSlotsModalState::new(),
            current_save_slots: Vec::new(),
            player_save_slots: Vec::new(),
            extensions_view_state: crate::extensions_view::ExtensionsViewState::default(),
            installed_extensions: Vec::new(),
            debugger_view_state: crate::debugger_view::DebuggerViewState::default(),
            big_picture_settings_tab: 0,
            identifying_controller_guid: None,
            is_scanning_games: false,
            details_focus_index: 0,
            management_focus_index: 0,
            sessions_focus_index: 0,
            settings_focus_zone: 0,
            settings_item_index: 0,
            hub_focus_zone: 1,
            hub_top_card_index: 0,
        };

        win.sync_player_assignments();
        win
    }

    /// Injects a mock/virtual controller device for testing (Developer Mode only).
    pub fn add_virtual_gamepad(&mut self) {
        if !self.app_settings.developer_mode {
            return;
        }
        let next_id = self
            .detected_devices_list
            .iter()
            .map(|d| d.id)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        let dev = DeviceItem::new_virtual(
            next_id,
            format!("Virtual Controller #{} (XInput)", next_id),
            format!("VIRTUAL-PAD-{:04}", next_id),
        );
        self.detected_devices_list.push(dev);
        self.detected_devices = self.detected_devices_list.len();
        self.sync_player_assignments();
        self.status_message = format!(
            "Added Virtual Controller #{} (Ready for assignment)",
            next_id
        );
    }

    /// Clears any injected virtual controller devices.
    pub fn clear_virtual_gamepads(&mut self) {
        self.detected_devices_list.retain(|d| !d.is_virtual);
        self.detected_devices = self.detected_devices_list.len();
        self.sync_player_assignments();
        self.status_message = "Cleared virtual mock gamepads.".to_string();
    }

    /// Sets explicit rich `HandlerItem` instances from GameManager.
    pub fn with_handler_items(mut self, handlers: Vec<HandlerItem>) -> Self {
        self.handler_names = handlers.iter().map(|h| h.game_name.clone()).collect();
        if self.selected_game.is_none() && !handlers.is_empty() {
            self.selected_game = Some(handlers[0].game_name.clone());
        }
        self.carousel_state.set_count(handlers.len());
        self.handlers = handlers;
        self
    }

    /// Sets explicit detected hardware `DeviceItem` instances.
    pub fn with_device_items(mut self, devices: Vec<DeviceItem>) -> Self {
        self.detected_devices = devices.len();
        self.detected_devices_list = devices;
        self.sync_player_assignments();
        self
    }

    /// Sets initial community packages loaded from Hub API.
    pub fn with_hub_packages(mut self, packages: Vec<HubPackageItem>) -> Self {
        self.hub_packages = packages;
        self
    }

    /// Replaces the entire callback builder struct on MainWindow.
    pub fn with_callbacks(mut self, callbacks: MainWindowCallbacks) -> Self {
        self.callbacks = callbacks;
        self
    }

    /// Sets the launch callback for starting games from the GUI.
    pub fn with_launcher<F>(mut self, callback: F) -> Self
    where
        F: Fn(
                &str,
                usize,
                &str,
                &std::collections::HashMap<String, String>,
                &[String],
                &[usize],
                &[String],
            ) + Send
            + Sync
            + 'static,
    {
        self.callbacks = self.callbacks.with_launcher(callback);
        self
    }

    /// Sets callback for toggling favorite status.
    pub fn with_favorite_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, bool) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_favorite_toggler(callback);
        self
    }

    /// Sets callback for saving profile properties.
    pub fn with_profile_saver<F>(mut self, callback: F) -> Self
    where
        F: Fn(&HandlerItem) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_profile_saver(callback);
        self
    }

    /// Sets callback for deleting UserProfile save directory.
    pub fn with_save_path_deleter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_save_path_deleter(callback);
        self
    }

    /// Sets callback for removing a game from the library.
    pub fn with_game_remover<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_game_remover(callback);
        self
    }

    /// Sets callback for adding a game by executable path.
    pub fn with_game_adder<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Option<HandlerItem> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_game_adder(callback);
        self
    }

    /// Sets callback for searching the community hub.
    pub fn with_hub_searcher<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<HubPackageItem> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_hub_searcher(callback);
        self
    }

    /// Dispatches an asynchronous Hub package search on a background thread.

    /// Sets callback for installing a hub handler package with progress reporting.
    pub fn with_hub_installer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, Box<dyn Fn(f32) + Send + Sync>) -> Option<HandlerItem> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_hub_installer(callback);
        self
    }

    /// Sets callback for updating the handlers directory.
    pub fn with_handlers_dir_changer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<HandlerItem> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_handlers_dir_changer(callback);
        self
    }

    /// Sets callback for saving theme changes.
    pub fn with_theme_changer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_theme_changer(callback);
        self
    }

    /// Sets callback for auto scan toggles.
    pub fn with_auto_scan_changer<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_auto_scan_changer(callback);
        self
    }

    /// Sets callback for scanning storage drives for installed games matching discovered handlers.
    pub fn with_game_scanner<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Vec<HandlerItem> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_game_scanner(callback);
        self
    }

    /// Sets callback for listing saved numbered profiles for a game.
    pub fn with_numbered_profile_lister<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<(usize, String)> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_numbered_profile_lister(callback);
        self
    }

    /// Sets callback for loading a numbered profile.
    pub fn with_numbered_profile_loader<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize) -> Option<nucleus_profile::GameProfile> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_numbered_profile_loader(callback);
        self
    }

    /// Sets callback for saving a numbered profile.
    pub fn with_numbered_profile_saver<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, Option<usize>, &nucleus_profile::GameProfile) -> Option<usize>
            + Send
            + Sync
            + 'static,
    {
        self.callbacks = self.callbacks.with_numbered_profile_saver(callback);
        self
    }

    /// Sets callback for deleting a numbered profile.
    pub fn with_numbered_profile_deleter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize) -> bool + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_numbered_profile_deleter(callback);
        self
    }

    /// Sets callback for unloading the active profile.
    pub fn with_profile_unloader<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_profile_unloader(callback);
        self
    }

    /// Sets callback for toggling session freeze.
    pub fn with_freeze_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Option<bool> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_freeze_toggler(callback);
        self
    }

    /// Sets callback for hot-swapping controller assignment mid-game.
    pub fn with_controller_reassigner<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_controller_reassigner(callback);
        self
    }

    /// Sets callback for re-enforcing window bounds.
    pub fn with_window_re_enforcer<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_window_re_enforcer(callback);
        self
    }

    /// Sets callback for toggling raw input lock.
    pub fn with_input_lock_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_input_lock_toggler(callback);
        self
    }

    /// Sets callback for stopping an active session.
    pub fn with_session_stopper<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_session_stopper(callback);
        self
    }

    /// Sets callback to check if a session is actively running in the engine.
    pub fn with_session_active_checker<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_session_active_checker(callback);
        self
    }

    /// Sets callback to toggle the window's TOPMOST status when in-game OSD opens or closes.
    pub fn with_osd_topmost_handler<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool, Option<usize>) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_osd_topmost_handler(callback);
        self
    }

    /// Sets callback for Hub account login.
    pub fn with_hub_login<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, bool) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_hub_login(callback);
        self
    }

    /// Sets callback for Hub account logout.
    pub fn with_hub_logout<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_hub_logout(callback);
        self
    }

    /// Sets callback for listing save slots.
    pub fn with_save_slots_lister<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Vec<nucleus_profile::save_slots::SaveSlot> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_save_slots_lister(callback);
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
        self.callbacks = self.callbacks.with_save_slot_creator(callback);
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
        self.callbacks = self.callbacks.with_save_slot_cloner(callback);
        self
    }

    /// Sets callback for deleting a save slot.
    pub fn with_save_slot_deleter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_save_slot_deleter(callback);
        self
    }

    /// Sets callback for assigning a player save slot.
    pub fn with_player_save_slot_setter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize, Option<String>) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_player_save_slot_setter(callback);
        self
    }

    /// Sets callback for toggling an extension.
    pub fn with_extension_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, bool) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_extension_toggler(callback);
        self
    }

    /// Sets callback for installing an extension.
    pub fn with_extension_installer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_extension_installer(callback);
        self
    }

    /// Sets callback for uninstalling an extension.
    pub fn with_extension_uninstaller<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_extension_uninstaller(callback);
        self
    }

    /// Sets callback for rescanning extensions from disk and immediately populates installed_extensions.
    pub fn with_extension_rescanner<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Vec<crate::extensions_view::ExtensionItem> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_extension_rescanner(callback);
        if let Some(ref cb) = self.callbacks.on_rescan_extensions {
            self.installed_extensions = cb();
        }
        self
    }

    /// Sets callback for granting an extension permission.
    pub fn with_extension_permission_granter<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_extension_permission_granter(callback);
        self
    }

    /// Sets callback for debugger resume.
    pub fn with_debugger_resume<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_resume(callback);
        self
    }

    /// Sets callback for debugger step over.
    pub fn with_debugger_step_over<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_step_over(callback);
        self
    }

    /// Sets callback for debugger step into.
    pub fn with_debugger_step_into<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_step_into(callback);
        self
    }

    /// Sets callback for debugger step out.
    pub fn with_debugger_step_out<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_step_out(callback);
        self
    }

    /// Sets callback for debugger pause.
    pub fn with_debugger_pause<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_pause(callback);
        self
    }

    /// Sets callback for debugger stop.
    pub fn with_debugger_stop<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_stop(callback);
        self
    }

    /// Sets callback for debugger expression evaluation.
    pub fn with_debugger_eval<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_eval(callback);
        self
    }

    /// Sets callback for toggling breakpoint.
    pub fn with_debugger_breakpoint_toggler<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, u32) + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_debugger_breakpoint_toggler(callback);
        self
    }

    /// Sets callback for reading handler script source code.
    pub fn with_handler_script_reader<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_handler_script_reader(callback);
        self
    }

    /// Sets callback for saving handler script source code.
    pub fn with_handler_script_saver<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_handler_script_saver(callback);
        self
    }

    /// Sets callback for installing a specific historical Hub handler package version with progress.
    pub fn with_hub_version_installer<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, i32, Option<u64>, Box<dyn Fn(f32) + Send + Sync>) -> Option<HandlerItem>
            + Send
            + Sync
            + 'static,
    {
        self.callbacks = self.callbacks.with_hub_version_installer(callback);
        self
    }

    /// Sets callback for hot-adding a Drop-In player into an active split-screen session.
    pub fn with_drop_in_player<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) -> Result<usize, String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_drop_in_handler(callback);
        self
    }

    /// Sets callback for cleanly dropping out an active player slot mid-session.
    pub fn with_drop_out_player<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, &str) -> Result<usize, String> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_drop_out_handler(callback);
        self
    }

    /// Sets callback to query a live `ActiveSessionSnapshot` from the background engine.
    pub fn with_active_session_querier<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Option<nucleus_engine::session::ActiveSessionSnapshot> + Send + Sync + 'static,
    {
        self.callbacks = self.callbacks.with_active_session_querier(callback);
        self
    }

    /// Renders the client-side decorations (CSD) title bar with icon, title, drag handling, and window controls.
    fn render_title_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        #[cfg(target_os = "windows")]
        let is_maximized = unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
            use windows::Win32::UI::WindowsAndMessaging::IsZoomed;
            let hwnd = GetActiveWindow();
            !hwnd.is_invalid() && IsZoomed(hwnd).as_bool()
        };
        #[cfg(not(target_os = "windows"))]
        let is_maximized = false;

        CouchTitleBar::new()
            .selected_game(self.selected_game.as_deref())
            .is_maximized(is_maximized)
            .on_toggle_big_picture(cx.listener(|this, _, _, cx| this.toggle_big_picture(cx)))
            .on_minimize(cx.listener(|_this, _, window, _cx| window.minimize_window()))
            .on_maximize(cx.listener(|_this, _, window, _cx| {
                #[cfg(target_os = "windows")]
                unsafe {
                    use windows::Win32::Foundation::{LPARAM, WPARAM};
                    use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
                    use windows::Win32::UI::WindowsAndMessaging::{
                        IsZoomed, PostMessageW, WM_SYSCOMMAND,
                    };
                    let hwnd = GetActiveWindow();
                    if !hwnd.is_invalid() {
                        let is_zoomed = IsZoomed(hwnd).as_bool();
                        let sc_cmd = if is_zoomed {
                            0xF120 /* SC_RESTORE */
                        } else {
                            0xF030 /* SC_MAXIMIZE */
                        };
                        let _ = PostMessageW(hwnd, WM_SYSCOMMAND, WPARAM(sc_cmd), LPARAM(0));
                        _cx.notify();
                        return;
                    }
                }
                window.zoom_window();
                _cx.notify();
            }))
            .on_close(cx.listener(|_this, _, window, _cx| window.remove_window()))
            .into_any_element()
    }

    /// Renders the left sidebar with navigation tabs and device counters.
    /// Supports both full mode (200px) and compact icon-only mode (60px) based on window width or manual toggle.
    fn render_sidebar(&self, window_width: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let is_compact = window_width < px(960.0);
        let is_collapsed = self.sidebar_collapsed.unwrap_or(is_compact);

        let handle_tab = cx.listener(|this, tab: &ActiveTab, _, cx| {
            this.active_tab = *tab;
            if *tab == ActiveTab::Setup {
                this.refresh_saved_profiles();
            } else if *tab == ActiveTab::Extensions {
                this.refresh_extensions();
            }
            cx.notify();
        });

        CouchSidebarNav::new()
            .active_tab(self.active_tab)
            .detected_devices(self.detected_devices)
            .handler_count(self.handlers.len())
            .is_collapsed(is_collapsed)
            .on_tab_click(move |tab, win, app| handle_tab(&tab, win, app))
            .on_toggle_collapse(cx.listener(move |this, _, _, cx| {
                let cur = this.sidebar_collapsed.unwrap_or(is_compact);
                this.sidebar_collapsed = Some(!cur);
                cx.notify();
            }))
            .into_any_element()
    }

    /// Renders the bottom status bar with session information and device count.
    fn render_status_bar(&self) -> AnyElement {
        CouchStatusBar::new()
            .message(&self.status_message)
            .detected_devices(self.detected_devices)
            .session_active(self.is_session_active())
            .into_any_element()
    }

    /// Renders the modal dialog for inspecting handler or Hub package details with Markdown support.
    fn render_details_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(ref pkg) = self.inspecting_hub_package {
            let is_installed =
                pkg.is_installed && self.is_handler_or_pkg_installed(&pkg.id, &pkg.game_name);
            let install_progress = self.installing_handlers.get(&pkg.id).copied();
            let has_extra_like = self.hub_detail_extra_likes.contains(&pkg.id);

            Some(crate::hub_details::render_hub_details_screen(
                pkg,
                is_installed,
                install_progress,
                self.hub_detail_tab,
                &self.hub_detail_comments,
                &self.hub_detail_packages,
                self.hub_detail_loading,
                has_extra_like,
                cx,
            ))
        } else if let Some(ref h) = self.inspecting_handler {
            Some(crate::handler_details::render_handler_details_modal(h, cx))
        } else {
            None
        }
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Auto-fetch hub packages asynchronously if entering Hub tab and not yet loaded
        if self.active_tab == ActiveTab::Hub && !self.hub_loaded {
            self.hub_loaded = true;
            self.trigger_hub_search_async(cx);
        }

        let active_handler = self.handlers.iter().find(|h| {
            if let Some(ref sel) = self.selected_game {
                h.game_name.eq_ignore_ascii_case(sel) || h.guid.eq_ignore_ascii_case(sel)
            } else {
                false
            }
        });

        if self.hub_search_focus.is_none() {
            self.hub_search_focus = Some(cx.focus_handle());
        }
        if self.lib_search_focus.is_none() {
            self.lib_search_focus = Some(cx.focus_handle());
        }
        if self.hotkey_record_focus.is_none() {
            self.hotkey_record_focus = Some(cx.focus_handle());
        }
        if self.window_focus.is_none() {
            self.window_focus = Some(cx.focus_handle());
        }
        let hub_focus = self.hub_search_focus.clone().unwrap();
        let lib_focus = self.lib_search_focus.clone().unwrap();
        let win_focus = self.window_focus.clone().unwrap();

        // Enforce focus state:
        // 1. If actively recording physical hotkey, ensure hotkey_record_focus is focused.
        // 2. Otherwise if window currently has no focus, focus win_focus so global shortcuts (Ctrl+N) work.
        if self.settings_view.recording_hotkey.is_some() {
            if let Some(ref f) = self.hotkey_record_focus {
                f.focus(_window, cx);
            }
        } else if _window.focused(cx).is_none() {
            win_focus.focus(_window, cx);
        }

        // Dedicated Fullscreen Topmost In-Game Couch OSD Overlay presentation
        if self.in_game_osd.is_visible {
            return div()
                .id("in-game-couch-osd-root")
                .size_full()
                .track_focus(&win_focus)
                .on_key_down(cx.listener(
                    |this: &mut MainWindow, event: &gpui::KeyDownEvent, _window, cx| {
                        let key = event.keystroke.key.as_str();
                        let ctrl = event.keystroke.modifiers.control;
                        let shift = event.keystroke.modifiers.shift;

                        if let Some(action_name) =
                            crate::big_picture::input::map_key_to_couch_action_name(
                                key, ctrl, shift,
                            )
                        {
                            this.dispatch_couch_action_by_name(action_name, cx);
                        }
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NavigateUp, _window, cx| {
                        this.dispatch_couch_action_by_name("NavigateUp", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NavigateDown, _window, cx| {
                        this.dispatch_couch_action_by_name("NavigateDown", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::ConfirmAction, _window, cx| {
                        this.dispatch_couch_action_by_name("Confirm", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::CancelAction, _window, cx| {
                        this.dispatch_couch_action_by_name("Cancel", cx);
                    },
                ))
                .child(crate::big_picture::render_in_game_osd(self, cx))
                .into_any_element();
        }

        if self.is_big_picture {
            return div()
                .relative()
                .flex()
                .flex_col()
                .size_full()
                .bg(theme::bg_main())
                .text_color(theme::text_primary())
                .track_focus(&win_focus)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if window.focused(cx).is_none() {
                            if let Some(ref f) = this.window_focus {
                                f.focus(window, cx);
                            }
                        }
                    }),
                )
                .on_key_down(cx.listener(
                    |this: &mut MainWindow, event: &gpui::KeyDownEvent, _window, cx| {
                        let key = event.keystroke.key.as_str();
                        let ctrl = event.keystroke.modifiers.control;
                        let shift = event.keystroke.modifiers.shift;

                        // 1. If Carousel search popover is open, route typing directly into search query
                        if this.carousel_state.search_input_open {
                            if event.keystroke.modifiers.control
                                || event.keystroke.modifiers.alt
                                || event.keystroke.modifiers.platform
                            {
                                return;
                            }
                            if key.eq_ignore_ascii_case("escape")
                                || key.eq_ignore_ascii_case("enter")
                            {
                                this.carousel_state.search_input_open = false;
                                cx.notify();
                                return;
                            }
                            if key.eq_ignore_ascii_case("backspace") {
                                this.carousel_state.search_query.pop();
                                this.carousel_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            if key.eq_ignore_ascii_case("space") {
                                this.carousel_state.search_query.push(' ');
                                this.carousel_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            if let Some(ref s) = event.keystroke.key_char {
                                this.carousel_state.search_query.push_str(s);
                                this.carousel_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            if key.chars().count() == 1 {
                                this.carousel_state.search_query.push_str(key);
                                this.carousel_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            return;
                        }

                        // 2. If Hub search popover is open, route typing directly into Hub search query
                        if this.couch_hub_state.search_input_open {
                            if event.keystroke.modifiers.control
                                || event.keystroke.modifiers.alt
                                || event.keystroke.modifiers.platform
                            {
                                return;
                            }
                            if key.eq_ignore_ascii_case("escape")
                                || key.eq_ignore_ascii_case("enter")
                            {
                                this.couch_hub_state.search_input_open = false;
                                cx.notify();
                                return;
                            }
                            if key.eq_ignore_ascii_case("backspace") {
                                this.couch_hub_state.search_query.pop();
                                this.couch_hub_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            if key.eq_ignore_ascii_case("space") {
                                this.couch_hub_state.search_query.push(' ');
                                this.couch_hub_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            if let Some(ref s) = event.keystroke.key_char {
                                this.couch_hub_state.search_query.push_str(s);
                                this.couch_hub_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            if key.chars().count() == 1 {
                                this.couch_hub_state.search_query.push_str(key);
                                this.couch_hub_state.selected_index = 0;
                                cx.notify();
                                return;
                            }
                            return;
                        }

                        // 3. If Setup combobox is open, route typing directly into draft
                        if this.setup_screen.combobox_open_key.is_some() {
                            if event.keystroke.modifiers.control
                                || event.keystroke.modifiers.alt
                                || event.keystroke.modifiers.platform
                            {
                                return;
                            }
                            if key.eq_ignore_ascii_case("escape") {
                                this.setup_screen.close_combobox();
                                cx.notify();
                                return;
                            }
                            if key.eq_ignore_ascii_case("backspace") {
                                this.setup_screen.combobox_search_draft.pop();
                                this.setup_screen.combobox_validation_error = None;
                                cx.notify();
                                return;
                            }
                            if key.eq_ignore_ascii_case("space") {
                                this.setup_screen.combobox_search_draft.push(' ');
                                this.setup_screen.combobox_validation_error = None;
                                cx.notify();
                                return;
                            }
                            if let Some(ref s) = event.keystroke.key_char {
                                this.setup_screen.combobox_search_draft.push_str(s);
                                this.setup_screen.combobox_validation_error = None;
                                cx.notify();
                                return;
                            }
                            if key.chars().count() == 1 {
                                this.setup_screen.combobox_search_draft.push_str(key);
                                this.setup_screen.combobox_validation_error = None;
                                cx.notify();
                                return;
                            }
                            return;
                        }

                        if let Some(action_name) =
                            crate::big_picture::input::map_key_to_couch_action_name(
                                key, ctrl, shift,
                            )
                        {
                            this.dispatch_couch_action_by_name(action_name, cx);
                        }
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::ToggleBigPicture, _window, cx| {
                        this.dispatch_couch_action_by_name("ToggleBigPicture", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::ToggleOsd, _window, cx| {
                        this.dispatch_couch_action_by_name("ToggleOsd", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NavigateUp, _window, cx| {
                        this.dispatch_couch_action_by_name("NavigateUp", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NavigateDown, _window, cx| {
                        this.dispatch_couch_action_by_name("NavigateDown", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NavigateLeft, _window, cx| {
                        this.dispatch_couch_action_by_name("NavigateLeft", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NavigateRight, _window, cx| {
                        this.dispatch_couch_action_by_name("NavigateRight", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::ConfirmAction, _window, cx| {
                        this.dispatch_couch_action_by_name("Confirm", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::CancelAction, _window, cx| {
                        this.dispatch_couch_action_by_name("Cancel", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::QuickLaunch, _window, cx| {
                        this.dispatch_couch_action_by_name("QuickLaunch", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::ContextAction1, _window, cx| {
                        this.dispatch_couch_action_by_name("Context1", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::ContextAction2, _window, cx| {
                        this.dispatch_couch_action_by_name("Context2", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::NextTab, _window, cx| {
                        this.dispatch_couch_action_by_name("NextTab", cx);
                    },
                ))
                .on_action(cx.listener(
                    |this: &mut MainWindow, _: &crate::actions::PrevTab, _window, cx| {
                        this.dispatch_couch_action_by_name("PrevTab", cx);
                    },
                ))
                .child(crate::big_picture::render_big_picture_view(
                    self, _window, cx,
                ))
                .children(self.render_confirm_dialog(cx))
                .children(self.render_osd_overlay())
                .into_any_element();
        }

        let active_view: AnyElement = match self.active_tab {
            ActiveTab::Library => crate::game_list::render_library_view(
                &self.handlers,
                self.selected_game.as_deref(),
                &self.game_list.search_query,
                &lib_focus,
                &self.game_list.scroll_handle,
                self.library_sort,
                self.library_sort_dir,
                &self.game_list.selected_filters,
                cx,
            ),
            ActiveTab::Setup => crate::setup_screen::render_setup_view(
                &self.setup_screen,
                active_handler,
                self.selected_game.as_deref(),
                self.player_count,
                &self.selected_layout,
                &self.detected_devices_list,
                &self.player_device_assignments,
                self.active_profile_id,
                &self.saved_profiles,
                &self.selected_handler_options,
                self.dropdown_player_device,
                &self.app_settings,
                cx,
            ),
            ActiveTab::Hub => crate::hub_browser::render_hub_view(
                &self.hub_packages,
                &self.handlers,
                &self.hub_search_query,
                &hub_focus,
                self.hub_page,
                self.hub_page_size,
                self.hub_sort,
                self.hub_sort_ascending,
                &self.selected_tags,
                &self.installing_handlers,
                &self.installed_package_ids,
                self.hub_loading,
                cx,
            ),
            ActiveTab::Settings => crate::settings_view::render_settings_view(self, cx),
            ActiveTab::Extensions => crate::extensions_view::render_extensions_view(self, cx),
            ActiveTab::Debugger => crate::debugger_view::render_debugger_view(self, cx),
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::bg_main())
            .text_color(theme::text_primary())
            .track_focus(&win_focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.settings_view.recording_hotkey.is_none() && window.focused(cx).is_none()
                    {
                        if let Some(ref f) = this.window_focus {
                            f.focus(window, cx);
                        }
                    }
                }),
            )
            .on_key_down(cx.listener(
                |this: &mut MainWindow, event: &gpui::KeyDownEvent, _window, cx| {
                    if this.settings_view.recording_hotkey.is_some() {
                        this.handle_key_recording(event, cx);
                        return;
                    }
                    if this.setup_screen.combobox_open_key.is_some() {
                        let key = event.keystroke.key.as_str();
                        if key.eq_ignore_ascii_case("escape") {
                            this.setup_screen.close_combobox();
                            cx.notify();
                            return;
                        }
                        if key.eq_ignore_ascii_case("backspace") {
                            this.setup_screen.combobox_search_draft.pop();
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                            return;
                        }
                        if key.eq_ignore_ascii_case("space") {
                            this.setup_screen.combobox_search_draft.push(' ');
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                            return;
                        }
                        if let Some(ref s) = event.keystroke.key_char {
                            this.setup_screen.combobox_search_draft.push_str(s);
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                            return;
                        }
                        if key.chars().count() == 1 {
                            this.setup_screen.combobox_search_draft.push_str(key);
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                            return;
                        }
                    }
                    if let Some((idx, field)) = this.settings_view.editing_player {
                        let key = event.keystroke.key.as_str();
                        if key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("enter") {
                            this.settings_view.editing_player = None;
                            cx.notify();
                            return;
                        }
                        match field {
                            crate::settings_view::PlayerEditField::Nickname => {
                                let mut cur = this.app_settings.get_player_nickname(idx);
                                if key.eq_ignore_ascii_case("backspace") {
                                    cur.pop();
                                } else if key.eq_ignore_ascii_case("space") {
                                    if cur.len() < 24 {
                                        cur.push(' ');
                                    }
                                } else if let Some(ref s) = event.keystroke.key_char {
                                    if cur.len() < 24 {
                                        cur.push_str(s);
                                    }
                                } else if key.chars().count() == 1 {
                                    if cur.len() < 24 {
                                        cur.push_str(key);
                                    }
                                }
                                this.app_settings.set_player_nickname(idx, cur);
                                cx.notify();
                            }
                            crate::settings_view::PlayerEditField::SteamId => {
                                let mut cur = this.app_settings.get_player_steam_id(idx);
                                if key.eq_ignore_ascii_case("backspace") {
                                    cur.pop();
                                } else if let Some(ref s) = event.keystroke.key_char {
                                    for c in s.chars() {
                                        if c.is_ascii_digit() && cur.len() < 17 {
                                            cur.push(c);
                                        }
                                    }
                                } else if key.chars().count() == 1 {
                                    let c = key.chars().next().unwrap();
                                    if c.is_ascii_digit() && cur.len() < 17 {
                                        cur.push(c);
                                    }
                                }
                                this.app_settings.set_player_steam_id(idx, cur);
                                cx.notify();
                            }
                        }
                        return;
                    }
                    let key = event.keystroke.key.as_str();
                    if key.eq_ignore_ascii_case("f11") {
                        this.toggle_big_picture(cx);
                        return;
                    }
                    if key.eq_ignore_ascii_case("escape") {
                        if this.context_menu_handler.is_some() {
                            this.context_menu_handler = None;
                            cx.notify();
                            return;
                        }
                        if this.inspecting_hub_package.is_some() {
                            this.inspecting_hub_package = None;
                            cx.notify();
                            return;
                        }
                    }
                    if event.keystroke.modifiers.control && key.eq_ignore_ascii_case("escape") {
                        this.toggle_in_game_osd(cx);
                        return;
                    }
                    if this.in_game_osd.is_visible {
                        if key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("b") {
                            if this.in_game_osd.reassign_modal_open {
                                this.in_game_osd.close_reassign();
                            } else {
                                this.set_osd_visible(false);
                            }
                            cx.notify();
                            return;
                        }
                        if key.eq_ignore_ascii_case("up") || key.eq_ignore_ascii_case("w") {
                            this.in_game_osd.move_up();
                            cx.notify();
                            return;
                        }
                        if key.eq_ignore_ascii_case("down") || key.eq_ignore_ascii_case("s") {
                            this.in_game_osd.move_down();
                            cx.notify();
                            return;
                        }
                        if key.eq_ignore_ascii_case("enter") || key.eq_ignore_ascii_case("space") {
                            this.handle_osd_confirm(cx);
                            return;
                        }
                        return;
                    }
                    if event.keystroke.modifiers.control && key.eq_ignore_ascii_case("n") {
                        this.cycle_layout(cx);
                    }
                },
            ))
            .on_action(cx.listener(
                |this: &mut MainWindow, _: &crate::actions::ToggleBigPicture, _window, cx| {
                    this.toggle_big_picture(cx);
                },
            ))
            .on_action(cx.listener(
                |this: &mut MainWindow, _: &crate::actions::ToggleOsd, _window, cx| {
                    this.toggle_in_game_osd(cx);
                },
            ))
            .on_action(cx.listener(
                |this: &mut MainWindow, _: &crate::actions::NavigateLeft, _window, cx| {
                    if this.is_big_picture {
                        if let Some(ref mut m) = this.couch_hub_state.custom_options_modal {
                            m.cycle_choice(false);
                        } else if this.big_picture_screen == BigPictureScreen::CouchSetup {
                            this.couch_setup_state.navigate_slot_left();
                        } else if this.big_picture_screen == BigPictureScreen::CouchHub {
                            let count = this.get_filtered_couch_hub_packages().len();
                            this.couch_hub_state.move_left(count);
                        } else {
                            this.carousel_state.move_left();
                        }
                        cx.notify();
                    }
                },
            ))
            .on_action(cx.listener(
                |this: &mut MainWindow, _: &crate::actions::NavigateRight, _window, cx| {
                    if this.is_big_picture {
                        if let Some(ref mut m) = this.couch_hub_state.custom_options_modal {
                            m.cycle_choice(true);
                        } else if this.big_picture_screen == BigPictureScreen::CouchSetup {
                            this.couch_setup_state.navigate_slot_right();
                        } else if this.big_picture_screen == BigPictureScreen::CouchHub {
                            let count = this.get_filtered_couch_hub_packages().len();
                            this.couch_hub_state.move_right(count);
                        } else {
                            this.carousel_state.move_right();
                        }
                        cx.notify();
                    }
                },
            ))
            .on_action(cx.listener(
                |this: &mut MainWindow, _: &crate::actions::ConfirmAction, _window, cx| {
                    if this.is_big_picture {
                        if this.couch_hub_state.custom_options_modal.is_some() {
                            this.confirm_couch_options_and_launch(cx);
                            return;
                        }
                        if let Some(h_name) = this.couch_hub_state.missing_exe_modal_handler.clone()
                        {
                            this.locate_exe_for_handler(&h_name, cx);
                            return;
                        }
                        if this.big_picture_screen == BigPictureScreen::CouchSetup {
                            let focused = this.couch_setup_state.focused_slot;
                            if this.couch_setup_state.grabbed_slot.is_some() {
                                this.couch_setup_state.drop_swap_target(focused);
                            } else {
                                this.couch_setup_state.grab_slot(focused);
                            }
                            cx.notify();
                        } else if this.big_picture_screen == BigPictureScreen::CouchHub {
                            let pkgs = this.get_filtered_couch_hub_packages();
                            if this.couch_hub_state.selected_index < pkgs.len() {
                                let pid = pkgs[this.couch_hub_state.selected_index].id.clone();
                                this.download_and_play_couch_package(&pid, cx);
                            }
                        } else {
                            this.select_carousel_game(this.carousel_state.selected_index, cx);
                        }
                    }
                },
            ))
            // Client-Side Decorations (Custom Title Bar)
            .child(self.render_title_bar(_window, cx))
            // Top body (Sidebar + Content)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(self.render_sidebar(_window.viewport_size().width, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .child(active_view),
                    ),
            )
            // Bottom status bar
            .child(self.render_status_bar())
            // Pre-flight Diagnostics Alert Banner (Task 6)
            .children(self.render_preflight_banners(cx))
            // Launch Prompt Modal (Task 3)
            .children(self.render_launch_prompt_modal(cx))
            // Details Modal (rendered on top when inspecting package or handler)
            .children(self.render_details_modal(cx))
            // Interactive Hotkey Recording Modal
            .children(self.render_hotkey_recording_modal(cx))
            // Context Menu Modal (rendered on top when right-clicking or clicking wrench)
            .children(self.render_context_menu(cx))
            // Confirmation Dialog Modal (rendered on top for destructive action confirmation)
            .children(self.render_confirm_dialog(cx))
            // Controller Shortcuts & UI Navigation Modal (media_1793137385955_044e590e.png)
            .children(self.render_controller_shortcuts_modal(cx))
            // Hub Account Authentication Modal
            .children(self.render_hub_account_modal(cx))
            // Save Slots Manager Modal
            .children(self.render_save_slots_modal(cx))
            // OSD Notification Overlay (rendered topmost)
            .children(self.render_osd_overlay())
            .into_any_element()
    }
}

/// Parses an OSD RGB color string such as "255,255,255" into GPUI Hsla color.
fn parse_osd_color(color_str: &str) -> Hsla {
    let parts: Vec<&str> = color_str.split(',').map(|s| s.trim()).collect();
    if parts.len() == 3 {
        let r = parts[0].parse::<u8>().unwrap_or(255);
        let g = parts[1].parse::<u8>().unwrap_or(255);
        let b = parts[2].parse::<u8>().unwrap_or(255);
        return rgb((r as u32) << 16 | (g as u32) << 8 | (b as u32)).into();
    }
    rgb(0xffffff).into()
}

mod couch_actions;
mod desktop_actions;
mod modals;
mod session_osd;

#[cfg(test)]
mod tests;

pub use modals::format_gamepad_button_glyph;
