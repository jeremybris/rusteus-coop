//! Configuration properties defined by a game handler script.

use nucleus_common::dll_injection::DllInjectionConfig;
use serde::{Deserialize, Serialize};

/// Comprehensive configuration structure containing properties exposed by game handlers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HandlerConfig {
    /// Unique identifier for this handler.
    pub guid: String,
    /// Display name of the game.
    pub game_name: String,
    /// Steam Application ID if applicable.
    pub steam_id: String,
    /// Maximum number of players supported across all instances.
    pub max_players: i32,
    /// Maximum number of players supported on a single monitor.
    pub max_players_one_monitor: i32,
    /// Primary executable name (e.g. "game.exe").
    pub executable_name: String,
    /// Alternative executable names to detect.
    pub executable_names: Option<Vec<String>>,
    /// Contextual directories or prefixes for locating the executable.
    pub executable_context: Option<Vec<String>>,
    /// Command line arguments passed when starting the game.
    pub start_arguments: String,
    /// Subfolder where binary files reside relative to game root.
    pub binaries_folder: String,
    /// Working directory for launching the game.
    pub working_folder: String,
    /// Separate launcher executable name if game requires launcher.
    pub launcher_exe: String,
    /// Title of the launcher window.
    pub launcher_title: String,
    /// Whether to symlink only the executable instead of the full directory.
    pub symlink_exe: bool,
    /// Whether to symlink game assets into instance folders.
    pub symlink_game: bool,
    /// Whether to create a hard full copy of the game files for each instance.
    pub hardcopy_game: bool,
    /// Whether to use NTFS hardlinks for game files.
    pub hardlink_game: bool,
    /// Keep created symlinks on disk when instances exit.
    pub keep_symlink_on_exit: bool,
    /// Delay in milliseconds between launching consecutive instances.
    pub pause_between_starts: i32,
    /// Whether keyboard input is supported for one of the players.
    pub supports_keyboard: bool,
    /// Whether the keyboard player is assigned to player slot 0.
    pub keyboard_player_first: bool,
    /// Whether multiple keyboards and mice are supported (e.g. ProtoInput).
    pub supports_multiple_keyboards_and_mice: bool,
    /// Whether XInput controller input is supported.
    pub supports_xinput: bool,
    /// Whether DirectInput controller input is supported.
    pub supports_dinput: bool,
    /// Whether RawInput (ProtoInput / multiple keyboards and mice) is supported.
    pub supports_raw_input: bool,
    /// Raw Game.Hook.XInputEnabled property from script.
    pub hook_xinput_enabled: bool,
    /// Raw Game.Hook.DInputEnabled property from script.
    pub hook_dinput_enabled: bool,
    /// Raw Game.Hook.XInputReroute property from script.
    pub hook_xinput_reroute: bool,
    /// Raw Game.Hook.SDL2Enabled property from script.
    pub hook_sdl2_enabled: bool,
    /// Raw Game.ProtoInput.XinputHook property from script.
    pub proto_input_xinput_hook: bool,
    /// Raw Game.ProtoInput.DinputDeviceHook / DinputHook property from script.
    pub proto_input_dinput_hook: bool,
    /// Whether custom DLL hooking is enabled (xinput1_3.dll, ncoop.ini).
    pub hook_custom_dll_enabled: bool,
    /// Whether window focus hooking is enabled.
    pub hook_focus: bool,
    /// Whether fake focus messages should be sent to instances.
    pub fake_focus: bool,
    /// Hide Windows taskbar during session.
    pub hide_taskbar: bool,
    /// Lock mouse cursor within game instance boundaries.
    pub lock_mouse: bool,
    /// Enable Goldberg Steam emulator for multiplayer/LAN emulation.
    pub use_goldberg: bool,
    /// Language code passed to Goldberg emulator.
    pub goldberg_language: String,
    /// Enable x360ce controller wrapper.
    pub use_x360ce: bool,
    /// Launch game through cmd.exe.
    pub cmd_launch: bool,
    /// Options passed to cmd.exe when cmd_launch is enabled.
    pub cmd_options: Option<Vec<String>>,
    /// Mutex names to search and close to allow multiple instances.
    pub kill_mutex: Option<Vec<String>>,
    /// Type of mutex object to kill (default "Mutant").
    pub kill_mutex_type: String,
    /// Delay in milliseconds before terminating the mutex.
    pub kill_mutex_delay: i32,
    /// Use ForceBindIP to bind instances to specific network adapters/IPs.
    pub use_force_bind_ip: bool,
    /// Use custom Nucleus environment variables during launch.
    pub use_nucleus_environment: bool,
    /// Whether window positioning and resizing is supported.
    pub supports_positioning: bool,
    /// Whether to preserve game window aspect ratio (letterbox/pillarbox).
    pub keep_aspect_ratio: bool,
    /// Whether to preserve monitor aspect ratio.
    pub keep_monitor_aspect_ratio: bool,
    /// Human-readable handler description and instructions.
    pub description: String,
    /// Whether Steam client emulation is required.
    pub needs_steam_emulation: bool,
    /// Timer interval in seconds between handler update ticks.
    pub handler_interval: f64,
    /// Enable debug logging and troubleshooting behavior.
    pub debug_mode: bool,
    /// Enable ProtoInput hooks for multiple keyboard/mouse support.
    pub use_proto_input: bool,

    // Additional important handler properties (reaching 50+ total)
    /// Relative path to save data directory.
    pub save_path: String,
    /// Directory patterns excluded from symlink creation.
    pub dir_symlink_exclusions: Option<Vec<String>>,
    /// File patterns excluded from symlink creation.
    pub file_symlink_exclusions: Option<Vec<String>>,
    /// Files copied directly instead of being symlinked.
    pub file_symlink_copy_instead: Option<Vec<String>>,
    /// Files backed up prior to instance execution.
    pub backup_files: Option<Vec<String>>,
    /// Folders backed up prior to instance execution.
    pub backup_folders: Option<Vec<String>>,
    /// Interval in milliseconds for sending fake focus messages.
    pub fake_focus_interval: i32,
    /// Hide the mouse cursor during gameplay.
    pub hide_cursor: bool,
    /// Process names to terminate when session ends.
    pub kill_processes_on_close: Option<Vec<String>>,
    /// Shell commands to run when session ends.
    pub cmd_batch_close: Option<Vec<String>>,
    /// Files to delete when session ends.
    pub delete_on_close: Option<Vec<String>>,
    /// Do not resize game windows to fit assigned viewport.
    pub dont_resize: bool,
    /// Do not reposition game windows.
    pub dont_reposition: bool,
    /// Do not make game windows topmost (default false, meaning topmost is enabled).
    pub not_top_most: bool,
    /// Whether to force topmost state on all instances after all finish launching.
    pub set_top_most_at_end: bool,
    /// Number of players assigned per instance (e.g. 1 for standard split-screen).
    pub players_per_instance: i32,

    // DevReorder
    pub use_dev_reorder: bool,
    pub dev_reorder_dlls: Option<Vec<String>>,

    // SDL2
    pub sdl_paths: Option<Vec<String>>,
    pub use_sdl: bool,

    // XInputPlus
    pub use_xinput_plus: bool,
    pub xinput_plus_dll: Option<Vec<String>>,
    pub use_xinput_index: bool,

    // x360ce
    pub x360ce_dll: Option<Vec<String>>,
    pub use_x360ce_on_steam: bool,

    // Hex editing
    pub hex_edit_exe: Option<Vec<String>>,
    pub hex_edit_dll_files: Option<Vec<String>>,

    // Advanced options
    pub xbox_one_controller_fix: bool,
    pub goldberg_need_steam_interface: bool,
    pub goldberg_write_account_name: bool,
    pub goldberg_lan_only: bool,
    pub goldberg_no_local_save: bool,
    pub create_steam_appid_by_exe: bool,
    pub goldberg_experimental: bool,
    pub goldberg_ignore_steam_appid: bool,
    pub custom_steam_api_dll_path: Option<Vec<String>>,
    pub process_changes_at_end: bool,
    pub create_single_device_file: bool,
    pub game_data_dir: String,
    pub launch_as_admin: bool,
    pub resume_game_on_lost_focus: bool,
    pub use_api_index: bool,
    pub hide_desktop: bool,
    pub set_thread_affinity: bool,
    pub thread_affinity: i32,
    pub forward_raw_keyboard_input: bool,
    pub forward_raw_mouse_input: bool,
    pub allow_fake_battrey: bool, // Note: keep this typo as it's from original C# source
    pub use_input_emulation: bool,
    pub use_realnpad: bool,

    // Hub metadata
    pub hub_handler_id: Option<String>,
    pub hub_handler_version: Option<i32>,
    pub hub_maintainer_name: Option<String>,
    pub hub_maintainer_id: Option<String>,

    // Parity filesystem & environment fields
    /// Whether to symlink folders instead of copying/linking files individually when possible.
    pub symlink_folders: bool,
    /// Files copied directly from root game directory to instance directory.
    pub copy_files: Option<Vec<String>>,
    /// Directory patterns completely excluded from being symlinked or traversed.
    pub dir_exclusions: Option<Vec<String>>,
    /// Directory patterns where the entire folder and contents must be copied rather than linked.
    pub dir_symlink_copy_instead: Option<Vec<String>>,
    /// Whether DirSymlinkCopyInstead should copy all subfolders recursively or link subfolders.
    pub dir_symlink_copy_instead_include_sub_folders: bool,
    /// UserProfile config path to replicate for each player.
    pub user_profile_config_path: Option<String>,
    /// UserProfile save path to replicate for each player.
    pub user_profile_save_path: Option<String>,
    /// Whether to avoid copying user profile save data when isolating player.
    pub user_profile_save_path_no_copy: bool,
    /// Documents config path to replicate for each player.
    pub documents_config_path: Option<String>,
    /// Documents save path to replicate for each player.
    pub documents_save_path: Option<String>,
    /// Script folder containing this handler.
    pub handler_folder: Option<String>,
    /// Full path to the handler .js file on disk.
    pub handler_path: Option<String>,

    // Advanced mutex fields
    pub kill_mutex_launcher: Option<Vec<String>>,
    pub kill_mutex_type_launcher: String,
    pub kill_mutex_delay_launcher: i32,
    pub partial_mutex_search_launcher: bool,
    pub kill_mutex_process: Option<Vec<String>>,
    pub mutex_process_exe: Option<String>,
    pub partial_mutex_search_process: bool,
    pub kill_mutex_type_process: String,
    pub kill_mutex_delay_process: i32,
    pub pause_before_mutex_killing: i32,
    pub kill_last_instance_mutex: bool,
    pub kill_mutex_at_end: bool,
    pub rename_not_kill_mutex: bool,
    pub partial_mutex_search: bool,

    // Window styling fields
    pub window_style_values: Option<Vec<String>>,
    pub ext_window_style_values: Option<Vec<String>>,
    pub window_style_end_changes: Option<Vec<String>>,
    pub ext_window_style_end_changes: Option<Vec<String>>,
    pub dont_remove_borders: bool,
    pub ignore_window_border_check: bool,
    pub refresh_window_after_start: bool,
    pub reset_windows: bool,
    pub set_foreground_window_elsewhere: bool,
    pub prevent_window_deactivation: bool,
    pub keyboard_player_skip_prevent_window_deactivate: bool,
    pub force_window_title: bool,
    pub has_dynamic_window_title: bool,
    pub id_in_window_title: bool,
    pub enable_windows: bool,
    pub desktop_scale: Option<Vec<f64>>,

    // Emulator / Hook fields
    pub use_nemirtingas_epic_emu: bool,
    /// Arbitrary DLLs to inject into target game instances.
    pub injected_dlls: Vec<DllInjectionConfig>,
    /// Whether PeerParty P2P multiplayer emulation is enabled.
    pub use_peerparty: bool,
    /// Explicit list of services to emulate via PeerParty.
    pub peerparty_services: Option<Vec<String>>,
    /// Optional private room code for PeerParty DHT internet co-op.
    pub peerparty_room: Option<String>,
    /// Enable public session discovery on BitTorrent DHT.
    pub peerparty_public: bool,
    /// Enable in-game Opus voice chat.
    pub peerparty_voice: bool,
    /// Enable PeerParty in-game overlay.
    pub peerparty_overlay: bool,
    /// Enable per-player save isolation copying.
    pub peerparty_copy_saves: bool,
    pub use_nemirtingas_galaxy_emu: bool,
    pub epic_emu_args: bool,
    pub alt_epic_emu_args: bool,
    pub flawless_widescreen: Option<String>,
    pub flawless_widescreen_plugin_path: Option<String>,
    pub flawless_widescreen_override_display: bool,
    pub use_steamless: bool,
    pub steamless_args: Option<String>,
    pub steamless_timing: i32,
    pub steamless_patch: Option<Vec<String>>,
    pub use_steam_stub_drm_patcher: bool,
    pub steam_stub_drm_patcher_arch: Option<String>,
    pub use_eac_bypass: bool,
    pub use_directx9_wrapper: bool,
    pub goldberg_no_warning: bool,
    pub goldberg_experimental_steam_client: bool,
    pub goldberg_experimental_rename: bool,
    pub goldberg_lobby_connect: bool,
    pub lock_input_toggle_key: i32,
    pub requires_admin: bool,

    // Process flags
    pub cmd_start_args_inside: bool,
    pub cmd_batch_before: Option<Vec<String>>,
    pub cmd_batch_after: Option<Vec<String>>,
    pub pause_cmd_batch_before: i32,
    pub pause_cmd_batch_after: i32,
    pub launcher_exe_ignore_file_check: bool,
    pub force_launcher_exe_ignore_file_check: bool,
    pub run_launcher_and_exe: bool,
    pub executable_to_launch: Option<String>,
    pub third_party_launch: bool,
    pub ignore_third_party_prompt: bool,
    pub force_process_pick: bool,
    pub force_process_search: bool,
    pub prompt_before_process_grab: bool,
    pub pause_between_process_grab: i32,
    pub pause_between_context_and_launch: i32,
    pub prompt_between_instances: bool,
    pub prompt_between_instances_end: bool,
    pub prompt_after_first_instance: bool,
    pub prompt_process_changes_at_end: bool,
    pub ignore_delete_files_prompt: bool,
    pub change_exe: bool,
    pub force_game_arch: Option<String>,
    pub write_to_process_memory: Option<String>,
    pub ideal_processor: i32,
    pub use_processor: Option<String>,
    pub processor_priority_class: Option<String>,
    pub use_processors_per_instance: Option<Vec<String>>,

    /// Remaining `Game.*` properties of the original handler API.
    #[serde(default)]
    pub legacy: crate::legacy_props::LegacyGameProps,
    /// Full `Game.ProtoInput.*` option block.
    #[serde(default)]
    pub proto_input: nucleus_hook::proto_input::ProtoInputConfig,

    // Options configured via Game.AddOption
    pub options: Vec<HandlerOption>,
    // Custom steps defined via Game.ShowOptionAsStep
    pub custom_steps: Vec<CustomStep>,

    // AutoHotkey Integration
    pub use_autohotkey: bool,
    pub autohotkey_version: nucleus_tools::autohotkey::AhkVersion,
    pub autohotkey_files: Option<Vec<String>>,
    pub autohotkey_code: Option<String>,
    pub autohotkey_timing: nucleus_tools::autohotkey::AhkTiming,

    // Ludusavi Savegame Integration
    pub ludusavi_title: Option<String>,
    pub ludusavi_paths: Option<Vec<String>>,
    pub ludusavi_registry: Option<Vec<String>>,
    pub ludusavi_disabled: bool,

    // Extensions & Plugins Integration
    pub required_plugins: Option<Vec<String>>,
    pub required_addons: Option<Vec<String>>,
}

/// Dynamic game option defined by `Game.AddOption`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HandlerOption {
    pub name: String,
    pub description: String,
    pub key: String,
    pub values: Vec<String>,
    pub hidden: bool,
}

/// Custom step prompt defined by `Game.ShowOptionAsStep`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomStep {
    pub option_key: String,
    pub title: String,
    pub required: bool,
}

impl Default for HandlerConfig {
    fn default() -> Self {
        Self {
            guid: String::new(),
            game_name: String::new(),
            steam_id: String::new(),
            max_players: 4,
            max_players_one_monitor: 4,
            executable_name: String::new(),
            executable_names: None,
            executable_context: None,
            start_arguments: String::new(),
            binaries_folder: String::new(),
            working_folder: String::new(),
            launcher_exe: String::new(),
            launcher_title: String::new(),
            symlink_exe: false,
            symlink_game: true,
            hardcopy_game: false,
            hardlink_game: false,
            keep_symlink_on_exit: false,
            pause_between_starts: 1000,
            supports_keyboard: false,
            keyboard_player_first: false,
            supports_multiple_keyboards_and_mice: false,
            supports_xinput: true,
            supports_dinput: false,
            supports_raw_input: false,
            hook_xinput_enabled: true,
            hook_dinput_enabled: false,
            hook_xinput_reroute: false,
            hook_sdl2_enabled: false,
            proto_input_xinput_hook: false,
            proto_input_dinput_hook: false,
            hook_custom_dll_enabled: false,
            hook_focus: false,
            fake_focus: false,
            hide_taskbar: false,
            lock_mouse: false,
            use_goldberg: false,
            goldberg_language: "english".to_string(),
            use_x360ce: false,
            cmd_launch: false,
            cmd_options: None,
            kill_mutex: None,
            kill_mutex_type: "Mutant".to_string(),
            kill_mutex_delay: 0,
            use_force_bind_ip: false,
            use_nucleus_environment: false,
            supports_positioning: true,
            keep_aspect_ratio: false,
            keep_monitor_aspect_ratio: false,
            description: String::new(),
            needs_steam_emulation: false,
            handler_interval: 0.0,
            debug_mode: false,
            use_proto_input: false,
            save_path: String::new(),
            dir_symlink_exclusions: None,
            file_symlink_exclusions: None,
            file_symlink_copy_instead: None,
            backup_files: None,
            backup_folders: None,
            fake_focus_interval: 1000,
            hide_cursor: false,
            kill_processes_on_close: None,
            cmd_batch_close: None,
            delete_on_close: None,
            dont_resize: false,
            dont_reposition: false,
            not_top_most: false,
            set_top_most_at_end: false,
            players_per_instance: 1,

            // DevReorder
            use_dev_reorder: false,
            dev_reorder_dlls: None,

            // SDL2
            sdl_paths: None,
            use_sdl: false,

            // XInputPlus
            use_xinput_plus: false,
            xinput_plus_dll: None,
            use_xinput_index: false,

            // x360ce
            x360ce_dll: None,
            use_x360ce_on_steam: false,

            // Hex editing
            hex_edit_exe: None,
            hex_edit_dll_files: None,

            // Advanced options
            xbox_one_controller_fix: false,
            goldberg_need_steam_interface: false,
            goldberg_write_account_name: false,
            goldberg_lan_only: false,
            goldberg_no_local_save: false,
            create_steam_appid_by_exe: false,
            goldberg_experimental: false,
            goldberg_ignore_steam_appid: false,
            custom_steam_api_dll_path: None,
            process_changes_at_end: false,
            create_single_device_file: false,
            game_data_dir: String::new(),
            launch_as_admin: false,
            resume_game_on_lost_focus: false,
            use_api_index: false,
            hide_desktop: false,
            set_thread_affinity: false,
            thread_affinity: -1,
            forward_raw_keyboard_input: false,
            forward_raw_mouse_input: false,
            allow_fake_battrey: false,
            use_input_emulation: false,
            use_realnpad: false,
            hub_handler_id: None,
            hub_handler_version: None,
            hub_maintainer_name: None,
            hub_maintainer_id: None,
            symlink_folders: false,
            copy_files: None,
            dir_exclusions: None,
            dir_symlink_copy_instead: None,
            dir_symlink_copy_instead_include_sub_folders: false,
            user_profile_config_path: None,
            user_profile_save_path: None,
            user_profile_save_path_no_copy: false,
            documents_config_path: None,
            documents_save_path: None,
            handler_folder: None,
            handler_path: None,

            // Advanced mutex defaults
            kill_mutex_launcher: None,
            kill_mutex_type_launcher: "Mutant".to_string(),
            kill_mutex_delay_launcher: 0,
            partial_mutex_search_launcher: false,
            kill_mutex_process: None,
            mutex_process_exe: None,
            partial_mutex_search_process: false,
            kill_mutex_type_process: "Mutant".to_string(),
            kill_mutex_delay_process: 0,
            pause_before_mutex_killing: 0,
            kill_last_instance_mutex: false,
            kill_mutex_at_end: false,
            rename_not_kill_mutex: false,
            partial_mutex_search: false,

            // Window styling defaults
            window_style_values: None,
            ext_window_style_values: None,
            window_style_end_changes: None,
            ext_window_style_end_changes: None,
            dont_remove_borders: false,
            ignore_window_border_check: false,
            refresh_window_after_start: false,
            reset_windows: false,
            set_foreground_window_elsewhere: false,
            prevent_window_deactivation: false,
            keyboard_player_skip_prevent_window_deactivate: false,
            force_window_title: false,
            has_dynamic_window_title: false,
            id_in_window_title: false,
            enable_windows: false,
            desktop_scale: None,

            // Emulator / Hook defaults
            use_nemirtingas_epic_emu: false,
            injected_dlls: Vec::new(),
            use_nemirtingas_galaxy_emu: false,
            epic_emu_args: false,
            alt_epic_emu_args: false,
            flawless_widescreen: None,
            flawless_widescreen_plugin_path: None,
            flawless_widescreen_override_display: false,
            use_steamless: false,
            steamless_args: None,
            steamless_timing: 2500,
            steamless_patch: None,
            use_steam_stub_drm_patcher: false,
            steam_stub_drm_patcher_arch: None,
            use_eac_bypass: false,
            use_directx9_wrapper: false,
            goldberg_no_warning: false,
            goldberg_experimental_steam_client: false,
            goldberg_experimental_rename: false,
            goldberg_lobby_connect: false,
            lock_input_toggle_key: 0x23,
            requires_admin: false,

            // Process flags defaults
            cmd_start_args_inside: false,
            cmd_batch_before: None,
            cmd_batch_after: None,
            pause_cmd_batch_before: 0,
            pause_cmd_batch_after: 0,
            launcher_exe_ignore_file_check: false,
            force_launcher_exe_ignore_file_check: false,
            run_launcher_and_exe: false,
            executable_to_launch: None,
            third_party_launch: false,
            ignore_third_party_prompt: false,
            force_process_pick: false,
            force_process_search: false,
            prompt_before_process_grab: false,
            pause_between_process_grab: 0,
            pause_between_context_and_launch: 0,
            prompt_between_instances: false,
            prompt_between_instances_end: false,
            prompt_after_first_instance: false,
            prompt_process_changes_at_end: false,
            ignore_delete_files_prompt: false,
            change_exe: false,
            force_game_arch: None,
            write_to_process_memory: None,
            ideal_processor: -1,
            use_processor: None,
            processor_priority_class: None,
            use_processors_per_instance: None,

            legacy: crate::legacy_props::LegacyGameProps::default(),
            proto_input: nucleus_hook::proto_input::ProtoInputConfig::default(),

            options: Vec::new(),
            custom_steps: Vec::new(),

            use_autohotkey: false,
            autohotkey_version: nucleus_tools::autohotkey::AhkVersion::Auto,
            autohotkey_files: None,
            autohotkey_code: None,
            autohotkey_timing: nucleus_tools::autohotkey::AhkTiming::PostWindow,

            ludusavi_title: None,
            ludusavi_paths: None,
            ludusavi_registry: None,
            ludusavi_disabled: false,

            required_plugins: None,
            required_addons: None,

            // PeerParty
            use_peerparty: false,
            peerparty_services: None,
            peerparty_room: None,
            peerparty_public: false,
            peerparty_voice: false,
            peerparty_overlay: false,
            peerparty_copy_saves: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_config_defaults() {
        let config = HandlerConfig::default();
        assert!(!config.use_dev_reorder);
        assert_eq!(config.dev_reorder_dlls, None);
        assert_eq!(config.sdl_paths, None);
        assert!(!config.use_sdl);
        assert!(!config.use_xinput_plus);
        assert_eq!(config.xinput_plus_dll, None);
        assert!(!config.use_xinput_index);
        assert_eq!(config.x360ce_dll, None);
        assert!(!config.use_x360ce_on_steam);
        assert_eq!(config.hex_edit_exe, None);
        assert_eq!(config.hex_edit_dll_files, None);
        assert!(!config.xbox_one_controller_fix);
        assert!(!config.goldberg_need_steam_interface);
        assert!(!config.goldberg_write_account_name);
        assert!(!config.goldberg_lan_only);
        assert!(!config.process_changes_at_end);
        assert!(!config.create_single_device_file);
        assert_eq!(config.game_data_dir, "");
        assert!(!config.launch_as_admin);
        assert!(!config.resume_game_on_lost_focus);
        assert!(!config.use_api_index);
        assert!(!config.hide_desktop);
        assert!(!config.set_thread_affinity);
        assert_eq!(config.thread_affinity, -1);
        assert!(!config.forward_raw_keyboard_input);
        assert!(!config.forward_raw_mouse_input);
        assert!(!config.allow_fake_battrey);
        assert!(!config.use_input_emulation);
        assert!(!config.use_realnpad);
    }

    #[test]
    fn test_handler_config_serde() {
        let config = HandlerConfig::default();
        let json = serde_json::to_string(&config).expect("Serialization failed");
        let deserialized: HandlerConfig =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(config, deserialized);
    }
}
