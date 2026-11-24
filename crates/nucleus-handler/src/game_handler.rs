//! Main GameHandler implementation managing multi-instance game execution.

use crate::environment::EnvironmentSetup;
use crate::hooks::HookManager;
use crate::instance::InstanceSetup;
use crate::launch::ProcessLauncher;
use crate::mutex::MutexManager;
use crate::traits::IGameHandler;
use crate::window::{WindowBounds, WindowSetup};
use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_platform::PlatformServices;
use nucleus_script::{HandlerConfig, HandlerContext, ScriptEngine};
use nucleus_tools::{DevReorderTool, Sdl2Tool, SteamEmuTool, X360ceTool, XInputPlusTool};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Primary game handler orchestrator that controls the complete lifecycle of a split-screen session.
pub struct GameHandler {
    /// Handler script configuration parameters.
    pub config: HandlerConfig,
    /// Absolute path to the original game directory.
    pub game_dir: PathBuf,
    /// Absolute path to the game executable.
    pub exe_path: PathBuf,
    /// Base staging path for instance directories.
    pub staging_dir: PathBuf,
    /// Total player count.
    pub player_count: usize,
    /// Per-player runtime contexts.
    pub contexts: Vec<HandlerContext>,
    /// Launched process IDs.
    pub launched_pids: Vec<u32>,
    /// Staged instance directories.
    pub instance_folders: Vec<PathBuf>,
    /// Optional script source code for executing Game.Play hooks.
    pub script_source: Option<String>,
    /// Optional platform services interface.
    pub platform: Option<Arc<dyn PlatformServices>>,
    /// Configured window splitting mode.
    pub split_mode: crate::window::SplitMode,
    /// Explicit window boundaries override per player.
    pub custom_splits: Option<Vec<WindowBounds>>,
    /// Configured monitor partition layout name.
    pub layout: String,
    /// Configured script options key-value pairs.
    pub options: std::collections::HashMap<String, String>,
    /// Configured player device assignments (e.g. "Keyboard & Mouse (Paired)", "Keyboard #1 (ProtoInput)", "Gamepad #0").
    pub player_device_assignments: Vec<String>,
    /// Whether instances are currently minimized via ShowHideWindows / TopMost hotkey.
    pub windows_minimized: bool,
    /// Whether cutscenes mode is currently active.
    pub cutscenes_active: bool,
    /// Whether the partition layout has been dynamically switched via switch_layout / Ctrl+N.
    pub is_layout_switched: bool,
    /// Optional explicit monitor index assignment per player (e.g. [0, 0, 1, 1]).
    pub player_monitor_assignments: Option<Vec<usize>>,
    /// Optional explicit audio endpoint ID assignment per player.
    pub player_audio_device_assignments: Option<Vec<String>>,
    /// Active RAII guard for Windows Explorer Personal shell folder redirection.
    pub personal_folder_guard: Option<nucleus_tools::users::PersonalFolderGuard>,
    /// Whether interactive prompts should proceed automatically via countdown instead of blocking dialog.
    pub auto_proceed_countdown: bool,
    /// Optional prompt handler callback (for testing and custom UI dialog integration).
    pub prompt_handler: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
    /// Central AutoHotkey process and runtime manager.
    pub ahk_manager: nucleus_tools::autohotkey::AhkManager,
    /// Optional Ludusavi save manager driver.
    pub ludusavi_driver: Option<nucleus_tools::ludusavi::LudusaviDriver>,
    /// Selected Ludusavi save slot per player slot.
    pub player_save_slots: Vec<Option<String>>,
    /// Whether Ludusavi automatic save backup and restore is enabled.
    pub ludusavi_auto_sync: bool,
    /// Custom config directory for isolated Ludusavi configuration.
    pub ludusavi_config_dir: Option<PathBuf>,
    /// Optional Extensions, Addons & Plugins manager.
    pub extension_manager: Option<Arc<parking_lot::Mutex<nucleus_plugin::ExtensionManager>>>,
    /// Optional step-by-step debugger controller.
    pub debug_controller: Option<nucleus_debug::DebugController>,
    /// Injected DLL module handles per PID: (pid, module_handle, eject_on_close)
    pub injected_modules: Vec<(u32, usize, bool)>,
    /// ProtoInput state for the running session (present once a handler requested it).
    pub proto_session: Option<crate::proto_launch::ProtoSession>,
    /// Physical keyboards and mice available for raw-input assignment.
    pub raw_devices: Vec<nucleus_input::RawDevice>,
    /// Set when the handler asked to keep its registry edits after the session.
    pub keep_edited_reg_keys: bool,
    /// Input-lock state last applied to the ProtoInput instances.
    pub applied_input_lock: bool,
    /// Set when a handler's older hook settings could not be applied because ProtoInput is
    /// not installed; the session then runs without them instead of failing.
    pub proto_unavailable: bool,
    /// Optional text prompt (message, default answer) for `Game.CustomUser*Prompts`.
    pub text_prompt_handler: Option<Arc<dyn Fn(&str, &str) -> String + Send + Sync>>,
    /// Answers to `Game.CustomUserGeneralPrompts`, asked once per session.
    pub custom_general_values: Option<Vec<String>>,
    /// Password of the `nucleusplayerN` Windows accounts (`LaunchAsDifferentUsers`).
    pub nucleus_account_password: String,
    /// Operations recorded from the handler's ProtoInput lifecycle callbacks.
    pub proto_recordings: Vec<(
        nucleus_script::proto_script::ProtoCallback,
        Vec<nucleus_script::proto_script::ProtoCall>,
    )>,
}

impl GameHandler {
    /// Creates a new `GameHandler` from the given parameters.
    pub fn new(
        config: HandlerConfig,
        game_dir: PathBuf,
        exe_path: PathBuf,
        staging_dir: PathBuf,
        player_count: usize,
    ) -> Self {
        Self {
            config,
            game_dir,
            exe_path,
            staging_dir,
            player_count,
            contexts: Vec::new(),
            launched_pids: Vec::new(),
            instance_folders: Vec::new(),
            script_source: None,
            platform: None,
            split_mode: crate::window::SplitMode::Auto,
            custom_splits: None,
            layout: "Auto".to_string(),
            options: std::collections::HashMap::new(),
            player_device_assignments: Vec::new(),
            windows_minimized: false,
            cutscenes_active: false,
            is_layout_switched: false,
            player_monitor_assignments: None,
            player_audio_device_assignments: None,
            personal_folder_guard: None,
            auto_proceed_countdown: false,
            prompt_handler: None,
            ahk_manager: nucleus_tools::autohotkey::AhkManager::new(),
            ludusavi_driver: None,
            player_save_slots: Vec::new(),
            ludusavi_auto_sync: true,
            ludusavi_config_dir: None,
            extension_manager: None,
            debug_controller: None,
            injected_modules: Vec::new(),
            proto_session: None,
            raw_devices: Vec::new(),
            keep_edited_reg_keys: false,
            applied_input_lock: false,
            proto_unavailable: false,
            text_prompt_handler: None,
            custom_general_values: None,
            nucleus_account_password: "12345".to_string(),
            proto_recordings: Vec::new(),
        }
    }

    /// Sets the text prompt used for `Game.CustomUser*Prompts` (message, default) -> answer.
    pub fn with_text_prompt_handler(
        mut self,
        handler: Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
    ) -> Self {
        self.text_prompt_handler = Some(handler);
        self
    }

    /// Sets the password used for the `nucleusplayerN` Windows accounts.
    pub fn with_nucleus_account_password(mut self, password: impl Into<String>) -> Self {
        let password = password.into();
        if !password.is_empty() {
            self.nucleus_account_password = password;
        }
        self
    }

    /// Asks the user for a line of text. Sessions without any UI get the default back.
    fn prompt_text(&self, message: &str, default: &str) -> String {
        if let Some(ref handler) = self.text_prompt_handler {
            return handler(message, default);
        }
        if cfg!(windows) && self.platform.is_some() && self.debug_controller.is_none() {
            // A plain native input box; the values are passed through the environment so
            // no quoting of user-visible text is needed.
            let script = "Add-Type -AssemblyName Microsoft.VisualBasic; \
                          [Microsoft.VisualBasic.Interaction]::InputBox($env:NUCLEUS_PROMPT_TEXT, \
                          'Nucleus Co-op', $env:NUCLEUS_PROMPT_DEFAULT)";
            let mut cmd = std::process::Command::new("powershell");
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("NUCLEUS_PROMPT_TEXT", message)
                .env("NUCLEUS_PROMPT_DEFAULT", default);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            if let Ok(out) = cmd.output() {
                let answer = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !answer.is_empty() {
                    return answer;
                }
            }
        }
        default.to_string()
    }

    /// File remembering answers to a set of custom prompts for this handler.
    fn custom_values_file(&self, scope: &str) -> PathBuf {
        let safe: String = self
            .config
            .guid
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.resolve_nucleus_root()
            .join("content")
            .join("custom_values")
            .join(format!("{safe}.{scope}.txt"))
    }

    /// Answers one group of `Game.CustomUser*Prompts`.
    ///
    /// With `save`, earlier answers are reused without asking; with `save_and_edit` they are
    /// offered as defaults and the new answers are stored.
    fn ask_custom_values(
        &self,
        prompts: &[String],
        scope: &str,
        who: &str,
        save: bool,
        save_and_edit: bool,
    ) -> Vec<String> {
        if prompts.is_empty() {
            return Vec::new();
        }
        let file = self.custom_values_file(scope);
        let remembered: Vec<String> = if save || save_and_edit {
            std::fs::read_to_string(&file)
                .map(|t| t.lines().map(|l| l.to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if save && !save_and_edit && remembered.len() == prompts.len() {
            return remembered;
        }
        let answers: Vec<String> = prompts
            .iter()
            .enumerate()
            .map(|(i, prompt)| {
                let default = remembered.get(i).map(|s| s.as_str()).unwrap_or_default();
                let message = if who.is_empty() {
                    prompt.clone()
                } else {
                    format!("{who}: {prompt}")
                };
                // Answers are stored one per line.
                self.prompt_text(&message, default)
                    .replace(['\r', '\n'], " ")
            })
            .collect();
        if save || save_and_edit {
            if let Some(parent) = file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&file, answers.join("\n"));
        }
        answers
    }

    /// Collects the handler's custom prompt answers for a player slot into its context.
    fn collect_custom_values(&mut self, ctx: &mut HandlerContext, slot_idx: usize) {
        let l = self.config.legacy.clone();
        if self.custom_general_values.is_none() {
            self.custom_general_values = Some(self.ask_custom_values(
                &l.custom_user_general_prompts,
                "general",
                "",
                l.save_custom_user_general_values,
                l.save_and_edit_custom_user_general_values,
            ));
        }
        ctx.custom_user_general_values = self.custom_general_values.clone().unwrap_or_default();
        let who = format!("Player {}", slot_idx + 1);
        ctx.custom_user_player_values = self.ask_custom_values(
            &l.custom_user_player_prompts,
            &format!("player{}", slot_idx + 1),
            &who,
            l.save_custom_user_player_values,
            l.save_and_edit_custom_user_player_values,
        );
        ctx.custom_user_instance_values = self.ask_custom_values(
            &l.custom_user_instance_prompts,
            &format!("instance{}", slot_idx + 1),
            &format!("Instance {}", slot_idx + 1),
            l.save_custom_user_instance_values,
            l.save_and_edit_custom_user_instance_values,
        );
    }

    /// The session's players as handler callbacks see them (`PlayerList`), with placeholder
    /// ProtoInput handles that are resolved when a recording is replayed.
    fn proto_player_list(
        &self,
        total_players: usize,
        current: &HandlerContext,
    ) -> Vec<nucleus_script::proto_script::ProtoPlayer> {
        (0..total_players)
            .map(|slot| {
                let mut ctx = if slot == current.player_index {
                    current.clone()
                } else if let Some(existing) = self.contexts.get(slot) {
                    existing.clone()
                } else {
                    let mut fresh = HandlerContext::new(slot, format!("Player{}", slot + 1));
                    let assignment = self
                        .player_device_assignments
                        .get(slot)
                        .map(|s| s.as_str())
                        .unwrap_or_default();
                    Self::apply_device_assignment_to_ctx(&mut fresh, slot, assignment);
                    fresh
                };
                ctx.player_index = slot;
                nucleus_script::proto_script::ProtoPlayer {
                    player_index: slot,
                    nickname: ctx.nickname.clone(),
                    instance_handle: crate::proto_launch::PLACEHOLDER_HANDLE_BASE + slot as u32,
                    process_id: ctx.process_id.unwrap_or(0),
                    is_keyboard_player: ctx.is_keyboard_player,
                    is_raw_keyboard: ctx.is_raw_keyboard,
                    is_raw_mouse: ctx.is_raw_mouse,
                    gamepad_id: ctx.gamepad_id.max(0),
                    screen_index: ctx.monitor_index,
                    proto_controllers: ctx.proto_controllers,
                    bounds: (ctx.pos_x, ctx.pos_y, ctx.width, ctx.height),
                }
            })
            .collect()
    }

    /// Replays the recording of a handler ProtoInput callback on the live instances.
    fn replay_proto_callback(&self, which: nucleus_script::proto_script::ProtoCallback) {
        let Some(ref session) = self.proto_session else {
            return;
        };
        if let Some((_, calls)) = self.proto_recordings.iter().find(|(w, _)| *w == which) {
            tracing::info!(
                "Applying handler {:?} callback ({} operation(s))",
                which,
                calls.len()
            );
            session.replay(calls);
        }
    }

    /// Applies a change of the global input lock (End key) to the ProtoInput instances.
    /// Called from the session heartbeat; does nothing for sessions without ProtoInput.
    pub fn sync_input_lock(&mut self) {
        let locked = nucleus_input::global_input_locked();
        if locked == self.applied_input_lock {
            return;
        }
        if let Some(ref session) = self.proto_session {
            session.set_input_lock(locked, self.config.legacy.lock_input_suspends_explorer);
            tracing::info!(
                "ProtoInput input lock {}",
                if locked { "engaged" } else { "released" }
            );
        }
        self.replay_proto_callback(if locked {
            nucleus_script::proto_script::ProtoCallback::InputLocked
        } else {
            nucleus_script::proto_script::ProtoCallback::InputUnlocked
        });
        self.applied_input_lock = locked;
    }

    /// Nucleus installation folder (parent of `utils`).
    fn resolve_nucleus_root(&self) -> PathBuf {
        if let Some(globals) = nucleus_common::Globals::try_get() {
            return globals.install_root.clone();
        }
        self.resolve_utils_dir()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    }

    /// SteamID64 for a player slot: the handler's own list when `UseHandlerSteamIds` is set,
    /// otherwise the same sequence the Goldberg deployment writes.
    fn steam_id_for_slot(&self, slot_idx: usize) -> u64 {
        if self.config.legacy.use_handler_steam_ids {
            if let Some(id) = self
                .config
                .legacy
                .player_steam_ids
                .get(slot_idx)
                .and_then(|s| s.trim().parse::<u64>().ok())
            {
                return id;
            }
        }
        76561198000000000u64 + slot_idx as u64 + 1
    }

    /// Whether the given device assignment label denotes a keyboard/mouse player.
    fn is_keyboard_assignment(assignment: &str) -> bool {
        assignment.contains("Keyboard") || assignment.contains("ProtoInput")
    }

    /// Sets the AutoHotkey manager.
    pub fn with_ahk_manager(mut self, mgr: nucleus_tools::autohotkey::AhkManager) -> Self {
        self.ahk_manager = mgr;
        self
    }

    /// Sets the Ludusavi save manager driver and player slots.
    pub fn with_ludusavi(
        mut self,
        driver: Option<nucleus_tools::ludusavi::LudusaviDriver>,
        player_save_slots: Vec<Option<String>>,
        auto_sync: bool,
    ) -> Self {
        self.ludusavi_driver = driver;
        self.player_save_slots = player_save_slots;
        self.ludusavi_auto_sync = auto_sync;
        self
    }

    /// Sets the Extension and Plugin manager.
    pub fn with_extension_manager(
        mut self,
        mgr: Arc<parking_lot::Mutex<nucleus_plugin::ExtensionManager>>,
    ) -> Self {
        self.extension_manager = Some(mgr);
        self
    }

    /// Sets the Debug controller.
    pub fn with_debug_controller(mut self, ctrl: nucleus_debug::DebugController) -> Self {
        self.debug_controller = Some(ctrl);
        self
    }

    /// Sets explicit monitor index assignments per player (e.g. `[0, 0, 1, 1]`).
    pub fn with_player_monitor_assignments(mut self, assignments: Vec<usize>) -> Self {
        self.player_monitor_assignments = Some(assignments);
        self
    }

    /// Sets explicit audio endpoint assignments per player.
    pub fn with_player_audio_device_assignments(mut self, assignments: Vec<String>) -> Self {
        self.player_audio_device_assignments = Some(assignments);
        self
    }

    /// Resolves the absolute path for an injected DLL configuration relative to instance or game directory.
    pub fn resolve_dll_path(
        &self,
        cfg: &nucleus_common::dll_injection::DllInjectionConfig,
        player_index: usize,
    ) -> nucleus_common::dll_injection::DllInjectionConfig {
        let mut resolved = cfg.clone();
        let p = Path::new(&cfg.path);
        if !p.exists() {
            if let Some(ctx) = self.contexts.get(player_index) {
                let inst_p = Path::new(&ctx.instance_folder).join(&cfg.path);
                if inst_p.exists() {
                    resolved.path = inst_p.to_string_lossy().to_string();
                    return resolved;
                }
            }
            let game_p = self.game_dir.join(&cfg.path);
            if game_p.exists() {
                resolved.path = game_p.to_string_lossy().to_string();
                return resolved;
            }
        }
        resolved
    }

    /// Sets whether interactive prompts should proceed automatically via countdown.
    pub fn with_auto_proceed_countdown(mut self, enabled: bool) -> Self {
        self.auto_proceed_countdown = enabled;
        self
    }

    /// Sets a custom callback handler for interactive prompts.
    pub fn with_prompt_handler(mut self, handler: Arc<dyn Fn(&str) -> bool + Send + Sync>) -> Self {
        self.prompt_handler = Some(handler);
        self
    }

    /// Prompts the user before proceeding to the next instance.
    /// Asks a yes/no question through the configured prompt handler. Without a handler
    /// (headless sessions) the answer is "no", so destructive steps are skipped.
    pub fn confirm_user(&self, message: &str) -> bool {
        match self.prompt_handler {
            Some(ref handler) => handler(message),
            None => {
                tracing::warn!("No prompt handler available; answering 'no' to: {message}");
                false
            }
        }
    }

    pub fn prompt_user(&self, message: &str) {
        tracing::info!("[Interactive Prompt] {}", message);
        if let Some(ref cb) = self.prompt_handler {
            let _ = cb(message);
            return;
        }

        if let Some(ref platform) = self.platform {
            platform.osd_manager().show(4000, message);
        }

        if self.auto_proceed_countdown {
            tracing::info!(
                "Auto-proceed countdown active: counting down 3s for prompt: {}",
                message
            );
            for sec in (1..=3).rev() {
                if let Some(ref platform) = self.platform {
                    platform
                        .osd_manager()
                        .show(1000, &format!("{} (Continuing in {}s...)", message, sec));
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        } else {
            #[cfg(windows)]
            {
                let ps_cmd = format!(
                    "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('{}', 'Nucleus Co-op', [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information)",
                    message.replace('\'', "''")
                );
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
                    .output();
            }
            #[cfg(not(windows))]
            {
                println!("[PROMPT] {}", message);
            }
        }
    }

    /// Sets the platform services provider for this handler.
    pub fn with_platform(mut self, platform: Arc<dyn PlatformServices>) -> Self {
        self.platform = Some(platform);
        self
    }

    /// Sets the layout name for viewport and script context calculations.
    pub fn with_layout(mut self, layout: impl Into<String>) -> Self {
        self.layout = layout.into();
        self
    }

    /// Sets script options key-value pairs.
    pub fn with_options(mut self, options: std::collections::HashMap<String, String>) -> Self {
        self.options = options;
        self
    }

    /// Sets player device assignments.
    pub fn with_player_device_assignments(mut self, assignments: Vec<String>) -> Self {
        self.player_device_assignments = assignments;
        self
    }

    /// Sets the script source code for executing handler lifecycle hooks like Game.Play.
    pub fn with_script_source(mut self, script: String) -> Self {
        self.script_source = Some(script);
        self
    }

    /// Configures the split screen layout mode for window positioning.
    pub fn with_split_mode(mut self, mode: crate::window::SplitMode) -> Self {
        self.split_mode = mode;
        self
    }

    /// Sets explicit window boundaries per player.
    pub fn with_custom_splits(mut self, bounds: Vec<WindowBounds>) -> Self {
        self.custom_splits = Some(bounds);
        self
    }

    /// Resolves the shared `utils` tool directory alongside globals or executable.
    fn resolve_utils_dir(&self) -> PathBuf {
        if let Some(globals) = nucleus_common::Globals::try_get() {
            if globals.utils_path.exists() {
                globals.utils_path.clone()
            } else if Path::new(r"C:\Games\NucleusCoop242\utils").exists() {
                PathBuf::from(r"C:\Games\NucleusCoop242\utils")
            } else {
                globals.utils_path.clone()
            }
        } else if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                if parent.join("utils").exists() {
                    parent.join("utils")
                } else if Path::new(r"C:\Games\NucleusCoop242\utils").exists() {
                    PathBuf::from(r"C:\Games\NucleusCoop242\utils")
                } else {
                    std::env::current_dir().unwrap_or_default().join("utils")
                }
            } else {
                std::env::current_dir().unwrap_or_default().join("utils")
            }
        } else if Path::new(r"C:\Games\NucleusCoop242\utils").exists() {
            PathBuf::from(r"C:\Games\NucleusCoop242\utils")
        } else {
            std::env::current_dir().unwrap_or_default().join("utils")
        }
    }

    /// Discovers relative Steam API DLL paths in the original game directory once for reuse across slots.
    pub fn discover_steam_api_dll_paths(&self) -> Option<Vec<String>> {
        if !self.config.use_goldberg || self.config.custom_steam_api_dll_path.is_some() {
            return None;
        }
        let mut found_dlls = Vec::new();
        if let Some(exe_dir) = self.exe_path.parent() {
            for dll_name in &["steam_api.dll", "steam_api64.dll"] {
                let cand = exe_dir.join(dll_name);
                if cand.is_file() {
                    found_dlls.push(cand);
                }
            }
        }
        if found_dlls.is_empty() {
            nucleus_tools::steam_emu::SteamEmuTool::find_steam_dlls_fast(
                &self.game_dir,
                &mut found_dlls,
            );
        }
        let rels: Vec<String> = found_dlls
            .into_iter()
            .filter_map(|p| {
                p.strip_prefix(&self.game_dir)
                    .ok()
                    .map(|r| r.to_string_lossy().to_string())
            })
            .collect();
        if rels.is_empty() { None } else { Some(rels) }
    }

    /// Applies paired K&M, ProtoInput, or Gamepad assignment metadata onto a slot's `HandlerContext`.
    fn apply_device_assignment_to_ctx(ctx: &mut HandlerContext, slot_idx: usize, assignment: &str) {
        if assignment.is_empty() {
            return;
        }
        if assignment.contains("Keyboard & Mouse (Paired)") || assignment == "Keyboard & Mouse Slot"
        {
            ctx.is_keyboard_player = true;
            ctx.is_raw_keyboard = false;
            ctx.is_raw_mouse = false;
            ctx.gamepad_id = -1;
        } else if assignment.contains("ProtoInput") {
            ctx.is_keyboard_player = true;
            if assignment.contains("Keyboard") || assignment.contains("K#") {
                ctx.is_raw_keyboard = true;
                ctx.raw_keyboard_guid = Some(format!("RAW-KB-{}", slot_idx + 1));
            }
            if assignment.contains("Mouse") || assignment.contains("M#") {
                ctx.is_raw_mouse = true;
                ctx.raw_mouse_guid = Some(format!("RAW-MOUSE-{}", slot_idx + 1));
            }
            ctx.gamepad_id = -1;
        } else {
            ctx.is_keyboard_player = false;
            ctx.is_raw_keyboard = false;
            ctx.is_raw_mouse = false;
            if let Some(pos) = assignment.find('#') {
                let rem = &assignment[pos + 1..];
                let num_str: String = rem.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(num) = num_str.parse::<i32>() {
                    ctx.gamepad_id = num + 1;
                }
            }
        }
    }

    fn resolve_primary_screen(&self) -> nucleus_common::types::Rect {
        if let Some(ref platform) = self.platform {
            platform
                .display_manager()
                .get_primary_display()
                .map(|d| d.bounds)
                .unwrap_or(nucleus_common::types::Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                })
        } else {
            nucleus_common::types::Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }
        }
    }

    /// Synchronizes `launched_pids` strictly from currently active player contexts.
    pub fn sync_launched_pids(&mut self) {
        self.launched_pids = self
            .contexts
            .iter()
            .filter(|c| c.is_active)
            .filter_map(|c| c.process_id)
            .collect();
    }

    /// Returns sorted slot indices of all currently active player instances.
    pub fn active_slot_indices(&self) -> Vec<usize> {
        self.contexts
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_active)
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns PIDs of all currently active player instances.
    pub fn active_pids(&self) -> Vec<u32> {
        self.contexts
            .iter()
            .filter(|c| c.is_active)
            .filter_map(|c| c.process_id)
            .collect()
    }

    /// Re-tiles all currently active player instances ordinally using `mode` without moving
    /// inactive slots, applying new bounds to surviving HWNDs.
    pub fn re_tile_active_instances(
        &mut self,
        mode: crate::window::SplitMode,
    ) -> NucleusResult<Vec<(usize, WindowBounds)>> {
        let surviving = self.active_slot_indices();
        if surviving.is_empty() {
            return Ok(Vec::new());
        }
        let primary_screen = self.resolve_primary_screen();
        let splits = WindowSetup::calculate_splits(primary_screen, surviving.len(), mode);
        let mut updates = Vec::with_capacity(surviving.len());

        for (ordinal, slot_idx) in surviving.into_iter().enumerate() {
            if let Some(&bounds) = splits.get(ordinal) {
                if let Some(ctx) = self.contexts.get_mut(slot_idx) {
                    ctx.pos_x = bounds.x;
                    ctx.pos_y = bounds.y;
                    ctx.width = bounds.width;
                    ctx.height = bounds.height;
                }
                let _ = self.reposition_instance_window(slot_idx);
                updates.push((slot_idx, bounds));
            }
        }

        Ok(updates)
    }

    /// Stages and configures a single player slot (files, emulators, hex edits, `HandlerContext`,
    /// and local JS `Game.Play`), updating `contexts[slot_idx]` & `instance_folders[slot_idx]` in place
    /// when reclaiming an existing slot or appending for a new slot.
    pub fn prepare_single_instance(
        &mut self,
        slot_idx: usize,
        device_assignment: &str,
        split_bounds: WindowBounds,
        discovered_steam_paths: Option<&[String]>,
        total_players: usize,
    ) -> NucleusResult<HandlerContext> {
        let utils_dir = self.resolve_utils_dir();
        let is_64bit = nucleus_common::is_64bit_executable(&self.exe_path).unwrap_or(true);
        let guid_count = total_players.max(slot_idx + 1);
        let all_player_guids: Vec<String> = (0..guid_count)
            .map(|p| format!("00000000-0000-0000-0000-{:012x}", p + 1))
            .collect();
        let all_guids_refs: Vec<&str> = all_player_guids.iter().map(|s| s.as_str()).collect();
        let primary_screen = self.resolve_primary_screen();

        let inst_folder = InstanceSetup::create_instance_folder(&self.staging_dir, slot_idx)?;

        if self.config.symlink_game || self.config.hardcopy_game || self.config.hardlink_game {
            let mut file_exclusions = self
                .config
                .file_symlink_exclusions
                .clone()
                .unwrap_or_default();
            if self.config.hook_custom_dll_enabled {
                file_exclusions.push("xinput1_3.dll".to_string());
                file_exclusions.push("ncoop.ini".to_string());
            }
            if !self.config.launcher_exe.is_empty() {
                let launcher_name = Path::new(&self.config.launcher_exe)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&self.config.launcher_exe)
                    .to_string();
                file_exclusions.push(launcher_name);
            }

            let options = crate::instance::LinkOptions {
                symlink_folders: self.config.symlink_folders && !self.config.hardcopy_game,
                hardlink: self.config.hardlink_game,
                hardcopy: self.config.hardcopy_game,
                symlink_exe: self.config.symlink_exe,
                dir_symlink_exclusions: self
                    .config
                    .dir_symlink_exclusions
                    .clone()
                    .unwrap_or_default(),
                dir_exclusions: self.config.dir_exclusions.clone().unwrap_or_default(),
                dir_symlink_copy_instead: self
                    .config
                    .dir_symlink_copy_instead
                    .clone()
                    .unwrap_or_default(),
                dir_symlink_copy_instead_include_sub_folders: self
                    .config
                    .dir_symlink_copy_instead_include_sub_folders,
                file_symlink_exclusions: file_exclusions,
                file_symlink_copy_instead: self
                    .config
                    .file_symlink_copy_instead
                    .clone()
                    .unwrap_or_default(),
                copy_files: self.config.copy_files.clone().unwrap_or_default(),
                exe_name: self
                    .exe_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string(),
            };
            InstanceSetup::link_game_directory(&self.game_dir, &inst_folder, &options)?;
        }

        let exe_rel = self.exe_path.file_name().unwrap_or_default();
        let inst_exe =
            if self.config.symlink_game || self.config.hardcopy_game || self.config.hardlink_game {
                if !self.config.binaries_folder.is_empty() {
                    inst_folder.join(&self.config.binaries_folder).join(exe_rel)
                } else {
                    inst_folder.join(exe_rel)
                }
            } else {
                self.exe_path.clone()
            };
        let inst_exe_dir =
            if self.config.symlink_game || self.config.hardcopy_game || self.config.hardlink_game {
                inst_exe.parent().unwrap_or(&inst_folder)
            } else {
                &inst_folder
            };

        // Folder placement, renames and deletions requested by the handler.
        {
            use crate::file_ops::{FolderLinkMode, place_folders};
            let legacy = &self.config.legacy;
            place_folders(
                &self.game_dir,
                &inst_folder,
                &legacy.copy_folders_to,
                FolderLinkMode::Copy,
            );
            place_folders(
                &self.game_dir,
                &inst_folder,
                &legacy.symlink_folders_to,
                FolderLinkMode::Symlink,
            );
            place_folders(
                &self.game_dir,
                &inst_folder,
                &legacy.hardlink_folders_to,
                FolderLinkMode::Hardlink,
            );
            crate::file_ops::rename_or_move_files(
                &inst_folder,
                &legacy.rename_and_or_move_files,
                slot_idx,
            );
            if !legacy.delete_files.is_empty() {
                let ignore_prompt = self.config.ignore_delete_files_prompt;
                crate::file_ops::delete_files(
                    &inst_folder,
                    &legacy.delete_files,
                    slot_idx,
                    |path| {
                        ignore_prompt
                            || self.confirm_user(&format!(
                                "Are you sure you want to delete '{}'?",
                                path.display()
                            ))
                    },
                );
            }
        }

        // Emulators and input tools are normally deployed before Game.Play runs;
        // `Game.GamePlayBeforeGameSetup` asks for the opposite order.
        let play_first = self.config.legacy.game_play_before_game_setup;
        let mut staged_ctx: Option<HandlerContext> = None;
        for run_play in if play_first {
            [true, false]
        } else {
            [false, true]
        } {
            if !run_play {
                let peerparty_handles_steam = self.config.use_peerparty
                    && self
                        .config
                        .peerparty_services
                        .as_ref()
                        .map(|s| {
                            s.iter().any(|v| {
                                v.eq_ignore_ascii_case("steam")
                                    || v.eq_ignore_ascii_case("steamworks")
                            })
                        })
                        .unwrap_or(true);

                if self.config.use_goldberg && !peerparty_handles_steam {
                    let steam_id = if self.config.steam_id.is_empty() {
                        "480"
                    } else {
                        &self.config.steam_id
                    };
                    let nickname = format!("Player{}", slot_idx + 1);
                    let handler_folder_path = self.config.handler_folder.as_deref().map(Path::new);
                    let custom_steam_paths: Option<&[String]> =
                        if let Some(ref custom) = self.config.custom_steam_api_dll_path {
                            Some(custom.as_slice())
                        } else {
                            discovered_steam_paths
                        };
                    let goldberg_opts = nucleus_tools::steam_emu::GoldbergDeployOptions {
                        instance_dir: &inst_folder,
                        orig_game_dir: &self.game_dir,
                        utils_dir: &utils_dir,
                        steam_id,
                        player_index: slot_idx,
                        nickname: &nickname,
                        is_64bit,
                        language: Some(&self.config.goldberg_language),
                        listen_port: None,
                        no_local_save: self.config.goldberg_no_local_save,
                        create_steam_appid_by_exe: self.config.create_steam_appid_by_exe,
                        experimental: self.config.goldberg_experimental,
                        ignore_steam_appid: self.config.goldberg_ignore_steam_appid,
                        need_steam_interface: self.config.goldberg_need_steam_interface,
                        use_nucleus_environment: self.config.use_nucleus_environment,
                        custom_steam_api_dll_path: custom_steam_paths,
                        handler_folder: handler_folder_path,
                        experimental_steam_client: self.config.goldberg_experimental_steam_client,
                        experimental_rename: self.config.goldberg_experimental_rename,
                        orig_steam_dll_path: None,
                        exe_name: Some(&self.config.executable_name),
                        start_arguments: None,
                    };
                    let _ = SteamEmuTool::deploy_goldberg(&goldberg_opts);
                    if self.config.legacy.use_handler_steam_ids {
                        let _ = SteamEmuTool::write_steam_id_and_account(
                            &inst_folder,
                            &nickname,
                            self.steam_id_for_slot(slot_idx),
                        );
                    }
                }

                if self.config.use_dev_reorder {
                    let visible_guid = &all_player_guids[slot_idx];
                    let _ = DevReorderTool::setup_dev_reorder(
                        &inst_folder,
                        visible_guid,
                        &all_guids_refs,
                        &utils_dir,
                        is_64bit,
                        true,
                    );
                    if inst_exe_dir != &inst_folder {
                        let _ = DevReorderTool::setup_dev_reorder(
                            inst_exe_dir,
                            visible_guid,
                            &all_guids_refs,
                            &utils_dir,
                            is_64bit,
                            true,
                        );
                    }
                    if let Some(ref custom_dlls) = self.config.dev_reorder_dlls {
                        let arch_folder = if is_64bit { "x64" } else { "x86" };
                        let src_dll = utils_dir
                            .join("devreorder")
                            .join(arch_folder)
                            .join("dinput8.dll");
                        if src_dll.exists() {
                            for dll_name in custom_dlls {
                                let _ = nucleus_common::file_util::copy_file(
                                    &src_dll,
                                    &inst_folder.join(dll_name),
                                    true,
                                );
                                if inst_exe_dir != &inst_folder {
                                    let _ = nucleus_common::file_util::copy_file(
                                        &src_dll,
                                        &inst_exe_dir.join(dll_name),
                                        true,
                                    );
                                }
                            }
                        }
                    }
                }

                if self.config.use_x360ce {
                    let dll_strings = self.config.x360ce_dll.as_deref().unwrap_or(&[]);
                    let dll_refs: Vec<&str> = dll_strings.iter().map(|s| s.as_str()).collect();
                    let player_guid = &all_player_guids[slot_idx];
                    let _ = X360ceTool::setup_x360ce(
                        &inst_folder,
                        player_guid,
                        &utils_dir,
                        is_64bit,
                        &dll_refs,
                        self.config.xbox_one_controller_fix,
                    );
                    if inst_exe_dir != &inst_folder {
                        let _ = X360ceTool::setup_x360ce(
                            inst_exe_dir,
                            player_guid,
                            &utils_dir,
                            is_64bit,
                            &dll_refs,
                            self.config.xbox_one_controller_fix,
                        );
                    }
                }

                if self.config.use_sdl {
                    let path_strings = self.config.sdl_paths.as_deref().unwrap_or(&[]);
                    let path_refs: Vec<&str> = path_strings.iter().map(|s| s.as_str()).collect();
                    let _ = Sdl2Tool::setup_sdl2(
                        &inst_folder,
                        Some(slot_idx as i32),
                        &[],
                        &utils_dir,
                        &path_refs,
                    );
                    if inst_exe_dir != &inst_folder {
                        let _ = Sdl2Tool::setup_sdl2(
                            inst_exe_dir,
                            Some(slot_idx as i32),
                            &[],
                            &utils_dir,
                            &path_refs,
                        );
                    }
                }

                if self.config.use_xinput_plus {
                    let dll_strings = self.config.xinput_plus_dll.as_deref().unwrap_or(&[]);
                    let dll_refs: Vec<&str> = dll_strings.iter().map(|s| s.as_str()).collect();
                    let use_old_dll = self.config.legacy.xinput_plus_old_dll;
                    let write_ini = !self.config.legacy.xinput_plus_no_ini;
                    let _ = XInputPlusTool::setup_xinput_plus(
                        &inst_folder,
                        &utils_dir,
                        is_64bit,
                        &dll_refs,
                        use_old_dll,
                    );
                    let ini_content = XInputPlusTool::generate_ini((slot_idx + 1) as u32);
                    if write_ini {
                        let _ = nucleus_common::file_util::write_text_file(
                            &inst_folder.join("XInputPlus.ini"),
                            &ini_content,
                        );
                    }
                    if inst_exe_dir != &inst_folder {
                        let _ = XInputPlusTool::setup_xinput_plus(
                            inst_exe_dir,
                            &utils_dir,
                            is_64bit,
                            &dll_refs,
                            use_old_dll,
                        );
                        if write_ini {
                            let _ = nucleus_common::file_util::write_text_file(
                                &inst_exe_dir.join("XInputPlus.ini"),
                                &ini_content,
                            );
                        }
                    }
                }

                // HexEdit* properties. `HexEditExe` and `HexEditFile` hold one entry per player; the
                // `All`/`Address` variants apply to every instance (or the one they name).
                {
                    let legacy = &self.config.legacy;
                    let edits = crate::file_ops::HexEdits {
                        exe: self.config.hex_edit_exe.as_deref().unwrap_or(&[]),
                        all_exes: &legacy.hex_edit_all_exes,
                        exe_address: &legacy.hex_edit_exe_address,
                        file: &legacy.hex_edit_file,
                        all_files: &legacy.hex_edit_all_files,
                        file_address: &legacy.hex_edit_file_address,
                    };
                    if !edits.is_empty() {
                        if inst_exe == self.exe_path {
                            tracing::warn!(
                                "Hex edits requested without an instanced copy of the game; the \
                             installed executable is left untouched"
                            );
                        } else {
                            let applied = edits.apply(&inst_exe, &inst_folder, slot_idx);
                            tracing::info!(
                                "Applied {applied} hex edit(s) for player {}",
                                slot_idx + 1
                            );
                        }
                    }
                }

                if let Some(ref hex_dlls) = self.config.hex_edit_dll_files {
                    for hex_entry in hex_dlls {
                        let delim = if hex_entry.contains(':') { ':' } else { '|' };
                        let parts: Vec<&str> = hex_entry.splitn(3, delim).collect();
                        if parts.len() == 3 {
                            let target = inst_folder.join(parts[0]);
                            if target.exists() && inst_folder != self.game_dir {
                                let find = parts[1].as_bytes().to_vec();
                                let replace = parts[2].as_bytes().to_vec();
                                if find.len() == replace.len() && find != replace {
                                    crate::file_ops::patch_detached(&target, |bytes| {
                                        let mut changed = false;
                                        while crate::file_ops::replace_first(bytes, &find, &replace)
                                        {
                                            changed = true;
                                        }
                                        changed
                                    });
                                }
                            }
                        }
                    }
                }

                if !self.config.legacy.copy_custom_utils.is_empty() {
                    let copied = crate::file_ops::copy_custom_utils(
                        &utils_dir,
                        &inst_folder,
                        &self.config.legacy.copy_custom_utils,
                        slot_idx,
                    );
                    tracing::info!(
                        "Copied {copied} custom util file(s) for player {}",
                        slot_idx + 1
                    );
                }

                if self.config.legacy.use_dinput_blocker {
                    let arch_folder = if is_64bit { "x64" } else { "x86" };
                    let blocker = utils_dir
                        .join("DInputBlocker")
                        .join(arch_folder)
                        .join("dinput8.dll");
                    if blocker.exists() {
                        let _ = nucleus_common::file_util::copy_file(
                            &blocker,
                            &inst_exe_dir.join("dinput8.dll"),
                            true,
                        );
                    } else {
                        tracing::warn!("UseDInputBlocker: {} not found", blocker.display());
                    }
                    let _ = nucleus_tools::dinput_blocker::DinputBlockerTool::setup_blocker(
                        inst_exe_dir,
                        true,
                        &[],
                    );
                }
            } else {
                let mut ctx = HandlerContext::new(slot_idx, format!("Player{}", slot_idx + 1));
                ctx.instance_folder = inst_folder.to_string_lossy().to_string();
                ctx.root_folder = inst_folder.to_string_lossy().to_string();
                ctx.root_install_folder = self.game_dir.to_string_lossy().to_string();
                ctx.orig_root_folder = self.game_dir.to_string_lossy().to_string();
                ctx.exe_path = inst_exe.to_string_lossy().to_string();
                ctx.start_arguments = self.config.start_arguments.clone();
                ctx.handler_guid = self.config.guid.clone();
                ctx.number_of_players = total_players;
                ctx.layout = self.layout.clone();
                ctx.options = self.options.clone();
                ctx.local_ip = "127.0.0.1".to_string();
                ctx.monitor_width = primary_screen.width;
                ctx.monitor_height = primary_screen.height;
                ctx.pos_x = split_bounds.x;
                ctx.pos_y = split_bounds.y;
                ctx.width = split_bounds.width;
                ctx.height = split_bounds.height;
                ctx.is_active = true;
                ctx.arch = if is_64bit { "x64" } else { "x86" }.to_string();
                ctx.is_fullscreen = split_bounds.width >= primary_screen.width
                    && split_bounds.height >= primary_screen.height;
                ctx.orig_width = Some(primary_screen.width);
                ctx.orig_height = Some(primary_screen.height);
                ctx.player_steam_id = Some(self.steam_id_for_slot(slot_idx).to_string());
                ctx.player_steam_ids = (0..guid_count)
                    .map(|i| self.steam_id_for_slot(i).to_string())
                    .collect();
                ctx.nucleus_folder =
                    Some(self.resolve_nucleus_root().to_string_lossy().to_string());
                // `handler_folder` is the folder holding the handler script; its companion folder
                // (`handlers/<script name>`) is what handlers copy from with `CopyScriptFolder`.
                ctx.handlers_folder = self.config.handler_folder.clone();
                ctx.script_folder = self
                    .config
                    .handler_path
                    .as_deref()
                    .map(|p| Path::new(p).with_extension(""))
                    .map(|p| p.to_string_lossy().to_string());
                if !self.config.launcher_exe.is_empty() {
                    ctx.launcher_folder = Path::new(&self.config.launcher_exe)
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(|p| inst_folder.join(p).to_string_lossy().to_string())
                        .or_else(|| Some(inst_folder.to_string_lossy().to_string()));
                }
                ctx.symlink_files = self.config.legacy.symlink_files.clone();
                ctx.user_profile_config_path = self.config.user_profile_config_path.clone();
                ctx.user_profile_save_path = self.config.user_profile_save_path.clone();
                ctx.documents_config_path = self.config.documents_config_path.clone();
                ctx.documents_save_path = self.config.documents_save_path.clone();
                ctx.num_keyboards = self
                    .player_device_assignments
                    .iter()
                    .take(guid_count)
                    .filter(|a| Self::is_keyboard_assignment(a))
                    .count();
                ctx.num_controllers = guid_count.saturating_sub(ctx.num_keyboards);
                ctx.has_keyboard_player = ctx.num_keyboards > 0;

                Self::apply_device_assignment_to_ctx(&mut ctx, slot_idx, device_assignment);

                // Replace the placeholder device IDs with the real raw-input handles when known.
                let (kb_handle, mouse_handle) = crate::proto_launch::raw_handles_for_assignment(
                    device_assignment,
                    &self.raw_devices,
                );
                if ctx.is_raw_keyboard && kb_handle != -1 {
                    ctx.raw_keyboard_guid = Some((kb_handle as u32).to_string());
                }
                if ctx.is_raw_mouse && mouse_handle != -1 {
                    ctx.raw_mouse_guid = Some((mouse_handle as u32).to_string());
                }
                if !self.config.legacy.use_manual_proto_controllers_setup {
                    let own = ctx.gamepad_id.max(0);
                    ctx.proto_controllers = if self.config.proto_input.multiple_proto_controllers {
                        [own; 4]
                    } else {
                        [own, 0, 0, 0]
                    };
                }

                ctx.using_nucleus_accounts = self.config.legacy.launch_as_different_users
                    || self.config.legacy.launch_as_different_users_alt;
                self.collect_custom_values(&mut ctx, slot_idx);

                // Execute Game.Play locally inside short-lived !Send ScriptEngine
                if let Some(ref script) = self.script_source {
                    let mut engine = ScriptEngine::new()?;
                    let folder_path = self.config.handler_folder.as_deref().map(Path::new);

                    // Inject active JavaScript add-ons into global context before script evaluation
                    if let Some(ref mgr) = self.extension_manager {
                        let addons_code = mgr.lock().get_combined_js_addons_code();
                        if !addons_code.is_empty() {
                            let _ = engine.execute(&addons_code);
                        }
                    }
                    if let Some(ref ctrl) = self.debug_controller {
                        ctrl.set_stage(nucleus_debug::protocol::PipelineStage::ScriptEvaluate);
                        let file_id = if !self.config.executable_name.is_empty() {
                            self.config.executable_name.clone()
                        } else {
                            "handler.js".to_string()
                        };
                        engine.execute_play_with_debugger(
                            &file_id,
                            script,
                            folder_path,
                            &mut ctx,
                            ctrl,
                        )?;
                    } else {
                        let _ = engine.load_handler_script_with_folder(script, folder_path)?;
                        engine.execute_play(&mut ctx)?;
                    }

                    // Handlers often define their input-lock callbacks inside Game.Play. Record what
                    // they ask for now; it is replayed on the live instances when the event happens.
                    let players = self.proto_player_list(total_players, &ctx);
                    let mut recordings = Vec::new();
                    for which in [
                        nucleus_script::proto_script::ProtoCallback::InputLocked,
                        nucleus_script::proto_script::ProtoCallback::InputUnlocked,
                        nucleus_script::proto_script::ProtoCallback::FinishedSetup,
                    ] {
                        match engine.run_proto_callback(which, &players) {
                            Ok(Some(calls)) => recordings.push((which, calls)),
                            Ok(None) => {}
                            Err(e) => tracing::warn!("Handler {:?} callback failed: {}", which, e),
                        }
                    }
                    self.proto_recordings = recordings;
                }

                staged_ctx = Some(ctx);
            }
        }
        let mut ctx = staged_ctx.expect("Game.Play phase always runs");

        // SmartSteamEmu wraps the game: the instance is started through its loader, which
        // receives the real executable and arguments from the generated ini.
        if self.config.needs_steam_emulation
            && !self.config.use_goldberg
            && inst_exe != self.exe_path
        {
            let app_id = if self.config.steam_id.is_empty() {
                "480"
            } else {
                self.config.steam_id.as_str()
            };
            let extra_lines = &self.config.legacy.sse_additional_lines;
            let sse = nucleus_tools::SmartSteamEmuOptions {
                instance_dir: &inst_folder,
                utils_dir: &utils_dir,
                target_exe: &inst_exe,
                start_in: inst_exe.parent(),
                cmd_line: &ctx.start_arguments,
                app_id,
                steam_id: self.steam_id_for_slot(slot_idx),
                nickname: &ctx.nickname,
                is_64bit,
                language: Some(&self.config.goldberg_language),
                additional_lines: if extra_lines.is_empty() {
                    None
                } else {
                    Some(extra_lines.as_slice())
                },
            };
            match SteamEmuTool::deploy_smart_steam_emu(&sse) {
                Ok(result) if result.loader_exe.is_file() => {
                    ctx.exe_path = result.loader_exe.to_string_lossy().to_string();
                    ctx.start_arguments.clear();
                }
                Ok(result) => tracing::warn!(
                    "SmartSteamEmu loader missing at {}; starting the game directly",
                    result.loader_exe.display()
                ),
                Err(e) => tracing::warn!("SmartSteamEmu deployment failed: {e}"),
            }
        }

        if ctx.keep_edited_reg_keys {
            self.keep_edited_reg_keys = true;
        }
        if ctx.hide_taskbar_requested {
            if let Some(ref platform) = self.platform {
                let _ = platform.taskbar_manager().hide();
            }
        }

        if let Some(ref mgr) = self.extension_manager {
            let _ = mgr.lock().dispatch_lifecycle_hook(
                nucleus_plugin::LifecycleHookKind::PostStage,
                Some(&inst_folder),
                slot_idx as i32,
            );
        }

        if slot_idx < self.contexts.len() {
            self.contexts[slot_idx] = ctx.clone();
            if slot_idx < self.instance_folders.len() {
                self.instance_folders[slot_idx] = inst_folder;
            }
        } else {
            self.contexts.push(ctx.clone());
            self.instance_folders.push(inst_folder);
        }

        Ok(ctx)
    }

    /// Drops out an arbitrary player slot (`slot_idx`), terminating ONLY that player's process,
    /// keeping `Instance<N>` saves intact, marking the slot inactive, and re-tiling survivors.
    pub fn drop_out_player(
        &mut self,
        slot_idx: usize,
        new_mode: crate::window::SplitMode,
    ) -> NucleusResult<usize> {
        if slot_idx >= self.contexts.len() || !self.contexts[slot_idx].is_active {
            return Err(NucleusError::Other(format!(
                "Player slot {} is not currently active",
                slot_idx + 1
            )));
        }

        if self.active_slot_indices().len() <= 1 {
            return Err(NucleusError::Other(
                "Cannot drop out the last remaining active player; stop the session instead."
                    .to_string(),
            ));
        }

        if let Some(pid) = self.contexts[slot_idx].process_id {
            if let Some(ref platform) = self.platform {
                let _ = platform.input_hook_manager().block_raw_input(pid, false);
                let _ = platform.process_manager().kill_process(pid);
            } else {
                let _ = ProcessLauncher::terminate(pid);
            }
        }

        self.contexts[slot_idx].is_active = false;
        self.contexts[slot_idx].process_id = None;
        self.contexts[slot_idx].window_handle = None;
        if let Some(ref mut session) = self.proto_session {
            session.forget(slot_idx);
        }

        self.sync_launched_pids();
        self.split_mode = new_mode;
        self.re_tile_active_instances(new_mode)?;

        Ok(self.active_slot_indices().len())
    }

    /// Drops in a new player on the lowest vacant slot (or appends a new slot up to `max_players`),
    /// prepares its instance isolation & `Game.Play`, launches it when platform services exist,
    /// and re-tiles all active windows with `new_mode`.
    pub fn drop_in_player(
        &mut self,
        device_assignment: &str,
        new_mode: crate::window::SplitMode,
        new_layout_name: &str,
    ) -> NucleusResult<usize> {
        let active_count = self.active_slot_indices().len();
        let max_players = if self.config.max_players > 0 {
            self.config.max_players as usize
        } else {
            usize::MAX
        };
        if active_count >= max_players {
            return Err(NucleusError::Other(format!(
                "Cannot drop in another player: session is already at max_players ({})",
                max_players
            )));
        }

        let slot_idx = self
            .contexts
            .iter()
            .position(|c| !c.is_active)
            .unwrap_or(self.contexts.len());

        if self.player_device_assignments.len() <= slot_idx {
            self.player_device_assignments
                .resize(slot_idx + 1, String::new());
        }
        self.player_device_assignments[slot_idx] = device_assignment.to_string();

        if self.config.symlink_game
            || self.config.symlink_exe
            || self.config.symlink_folders
            || self.config.hardlink_game
        {
            if let Some(ref platform) = self.platform {
                let _ = platform.elevation_manager().ensure_symlink_support();
            }
        }

        let new_total = active_count + 1;
        let discovered_steam_paths = self.discover_steam_api_dll_paths();
        let primary_screen = self.resolve_primary_screen();
        let preview_splits = WindowSetup::calculate_splits(primary_screen, new_total, new_mode);
        let mut active_with_new = self.active_slot_indices();
        if !active_with_new.contains(&slot_idx) {
            active_with_new.push(slot_idx);
            active_with_new.sort_unstable();
        }
        let ordinal = active_with_new
            .iter()
            .position(|&s| s == slot_idx)
            .unwrap_or_else(|| preview_splits.len().saturating_sub(1));
        let initial_bounds = preview_splits
            .get(ordinal)
            .or_else(|| preview_splits.last())
            .copied()
            .unwrap_or_default();

        self.layout = new_layout_name.to_string();
        self.split_mode = new_mode;

        self.prepare_single_instance(
            slot_idx,
            device_assignment,
            initial_bounds,
            discovered_steam_paths.as_deref(),
            new_total,
        )?;

        if self.platform.is_some() {
            self.launch_instance(slot_idx)?;
            let _ = self.setup_window(slot_idx);
        }

        self.player_count = self.player_count.max(slot_idx + 1);
        self.re_tile_active_instances(new_mode)?;

        Ok(slot_idx)
    }

    /// Triggers configured AutoHotkey scripts matching the specified timing for a player instance.
    pub fn trigger_ahk_scripts(
        &self,
        timing: nucleus_tools::autohotkey::AhkTiming,
        player_index: usize,
    ) {
        if player_index >= self.contexts.len() {
            return;
        }

        let ctx = self.contexts[player_index].clone();
        let inst_folder = PathBuf::from(&ctx.instance_folder);
        let handler_folder = self
            .config
            .handler_folder
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.game_dir.clone());

        let prelude_vars = nucleus_tools::autohotkey::AhkPreludeVars {
            player_index,
            player_id: (player_index + 1) as u64,
            nickname: ctx.nickname.clone(),
            process_id: ctx.process_id.unwrap_or(0),
            window_handle: ctx.window_handle.unwrap_or(0),
            pos_x: ctx.pos_x,
            pos_y: ctx.pos_y,
            width: ctx.width,
            height: ctx.height,
            instance_folder: ctx.instance_folder.clone(),
            root_install_folder: ctx.root_install_folder.clone(),
            handler_folder: handler_folder.to_string_lossy().to_string(),
            is_keyboard_player: ctx.is_keyboard_player,
            gamepad_id: ctx.gamepad_id,
        };

        // 1. Directives collected from Context.ImportAhkFile and Context.RunAhkCode
        for directive in &ctx.ahk_directives {
            let dir_timing = match directive
                .timing
                .as_deref()
                .map(|s| s.to_lowercase())
                .as_deref()
            {
                Some("immediate") => nucleus_tools::autohotkey::AhkTiming::Immediate,
                Some("prelaunch") | Some("pre") | Some("before") => {
                    nucleus_tools::autohotkey::AhkTiming::PreLaunch
                }
                _ => nucleus_tools::autohotkey::AhkTiming::PostWindow,
            };

            if dir_timing != timing {
                continue;
            }

            let ver = match directive
                .version
                .as_deref()
                .map(|s| s.to_lowercase())
                .as_deref()
            {
                Some("v1") | Some("1") => nucleus_tools::autohotkey::AhkVersion::V1,
                Some("v2") | Some("2") => nucleus_tools::autohotkey::AhkVersion::V2,
                _ => nucleus_tools::autohotkey::AhkVersion::Auto,
            };

            let source = if directive.is_inline {
                nucleus_tools::autohotkey::AhkSource::Inline {
                    label: directive.id.clone(),
                    code: directive.content.clone(),
                    working_dir: inst_folder.clone(),
                }
            } else {
                match nucleus_tools::autohotkey::AhkManager::resolve_handler_ahk_file(
                    &handler_folder,
                    &inst_folder,
                    &directive.content,
                ) {
                    Ok(path) => nucleus_tools::autohotkey::AhkSource::File { path },
                    Err(e) => {
                        tracing::warn!(
                            "Failed resolving AHK directive script '{}': {}",
                            directive.content,
                            e
                        );
                        continue;
                    }
                }
            };

            let spec = nucleus_tools::autohotkey::AhkLaunchSpec {
                id: format!("{}_{}", directive.id, player_index),
                source,
                version: ver,
                timing,
                vars: prelude_vars.clone(),
                custom_runtime: None,
                extra_args: directive.args.clone(),
            };

            let _ = self.ahk_manager.spawn_script(&spec);
        }

        // 2. Global scripts declared on Game (Game.AutoHotkeyFiles / Game.AutoHotkeyCode)
        if self.config.use_autohotkey && self.config.autohotkey_timing == timing {
            if let Some(ref files) = self.config.autohotkey_files {
                for (f_idx, rel_file) in files.iter().enumerate() {
                    if let Ok(file_path) =
                        nucleus_tools::autohotkey::AhkManager::resolve_handler_ahk_file(
                            &handler_folder,
                            &inst_folder,
                            rel_file,
                        )
                    {
                        let spec = nucleus_tools::autohotkey::AhkLaunchSpec {
                            id: format!("global_file_{}_{}", f_idx, player_index),
                            source: nucleus_tools::autohotkey::AhkSource::File { path: file_path },
                            version: self.config.autohotkey_version,
                            timing,
                            vars: prelude_vars.clone(),
                            custom_runtime: None,
                            extra_args: Vec::new(),
                        };
                        let _ = self.ahk_manager.spawn_script(&spec);
                    }
                }
            }

            if let Some(ref code) = self.config.autohotkey_code {
                let spec = nucleus_tools::autohotkey::AhkLaunchSpec {
                    id: format!("global_inline_{}", player_index),
                    source: nucleus_tools::autohotkey::AhkSource::Inline {
                        label: format!("code_{}", player_index),
                        code: code.clone(),
                        working_dir: inst_folder.clone(),
                    },
                    version: self.config.autohotkey_version,
                    timing,
                    vars: prelude_vars,
                    custom_runtime: None,
                    extra_args: Vec::new(),
                };
                let _ = self.ahk_manager.spawn_script(&spec);
            }
        }
    }

    /// Syncs player save slot via Ludusavi (restore before launch, or backup after stop).
    pub fn sync_ludusavi_slot(&self, player_index: usize, is_backup: bool) -> NucleusResult<()> {
        if !self.ludusavi_auto_sync || self.config.ludusavi_disabled {
            return Ok(());
        }

        let driver = match self.ludusavi_driver.as_ref() {
            Some(d) => d,
            None => return Ok(()),
        };

        let slot_id = match self
            .player_save_slots
            .get(player_index)
            .and_then(|s| s.as_ref())
        {
            Some(id) => id,
            None => return Ok(()),
        };

        let ctx = match self.contexts.get(player_index) {
            Some(c) => c,
            None => return Ok(()),
        };

        let config_dir = match self.ludusavi_config_dir.as_ref() {
            Some(d) => d.clone(),
            None => std::env::temp_dir().join("nucleus_ludusavi_config"),
        };

        let slots_root = if let Some(globals) = nucleus_common::Globals::try_get() {
            globals.app_data_path.join("saves")
        } else {
            PathBuf::from("content").join("saves")
        };
        let backup_repo_dir = slots_root.join(&self.config.guid).join(slot_id);

        let game_title = self
            .config
            .ludusavi_title
            .clone()
            .unwrap_or_else(|| self.config.game_name.clone());

        let mut custom_files = self.config.ludusavi_paths.clone().unwrap_or_default();
        let instance_path = PathBuf::from(&ctx.instance_folder);
        for file in &mut custom_files {
            let p = Path::new(file);
            if p.is_relative() {
                *file = instance_path.join(p).to_string_lossy().to_string();
            }
        }

        let target = nucleus_tools::ludusavi_config::LudusaviInstanceTarget {
            game_title,
            backup_repo_dir,
            redirects: Vec::new(),
            custom_files,
            custom_registry: self.config.ludusavi_registry.clone().unwrap_or_default(),
            full_backup_limit: 5,
        };

        // Write isolated config
        nucleus_tools::ludusavi_config::write_isolated_config(&config_dir, &target)?;

        if is_backup {
            tracing::info!(
                "Ludusavi: Backing up instance {} into slot '{}'",
                player_index,
                slot_id
            );
            let _ = driver.backup_instance(&config_dir, &target, false);
        } else {
            tracing::info!(
                "Ludusavi: Restoring slot '{}' into instance {}",
                slot_id,
                player_index
            );
            if target.backup_repo_dir.exists() {
                let _ = driver.restore_instance(&config_dir, &target, false);
            }
        }

        Ok(())
    }
}

impl Drop for GameHandler {
    fn drop(&mut self) {
        self.ahk_manager.stop_all();
    }
}

impl IGameHandler for GameHandler {
    fn prepare_instances(&mut self) -> NucleusResult<()> {
        if let Some(ref ctrl) = self.debug_controller {
            ctrl.set_stage(nucleus_debug::protocol::PipelineStage::PrepareInstances);
        }
        if let Some(ref mgr) = self.extension_manager {
            let _ = mgr.lock().dispatch_lifecycle_hook(
                nucleus_plugin::LifecycleHookKind::PrePrepare,
                None,
                self.player_count as i32,
            );
        }

        self.contexts.clear();
        self.instance_folders.clear();

        if self
            .player_device_assignments
            .iter()
            .any(|a| a.contains("ProtoInput"))
        {
            self.raw_devices = nucleus_input::enumerate_raw_devices();
            tracing::info!(
                "Raw input devices: {} keyboard(s), {} mouse/mice",
                nucleus_input::device_count(
                    &self.raw_devices,
                    nucleus_input::RawDeviceKind::Keyboard
                ),
                nucleus_input::device_count(&self.raw_devices, nucleus_input::RawDeviceKind::Mouse)
            );
        }

        let utils_dir = self.resolve_utils_dir();
        tracing::info!("GameHandler using utils directory: {}", utils_dir.display());
        let primary_screen = self.resolve_primary_screen();

        let calculated_splits = if let Some(ref custom) = self.custom_splits {
            custom.clone()
        } else if let (Some(platform), Some(monitor_map)) =
            (&self.platform, &self.player_monitor_assignments)
        {
            let displays = platform
                .display_manager()
                .get_displays()
                .unwrap_or_default();
            if !displays.is_empty() && monitor_map.len() == self.player_count {
                let mut monitor_to_players: std::collections::BTreeMap<usize, Vec<usize>> =
                    std::collections::BTreeMap::new();
                for (p_idx, &m_idx) in monitor_map.iter().enumerate() {
                    monitor_to_players.entry(m_idx).or_default().push(p_idx);
                }

                let mut splits = vec![crate::window::WindowBounds::default(); self.player_count];
                for (m_idx, players_on_monitor) in monitor_to_players {
                    let display_rect = displays
                        .get(m_idx)
                        .map(|d| d.bounds)
                        .unwrap_or(primary_screen);

                    let mon_splits = WindowSetup::calculate_splits(
                        display_rect,
                        players_on_monitor.len(),
                        self.split_mode,
                    );
                    for (slot_idx, &player_idx) in players_on_monitor.iter().enumerate() {
                        if let Some(&bounds) = mon_splits.get(slot_idx) {
                            splits[player_idx] = bounds;
                        }
                    }
                }
                splits
            } else {
                WindowSetup::calculate_splits(primary_screen, self.player_count, self.split_mode)
            }
        } else {
            WindowSetup::calculate_splits(primary_screen, self.player_count, self.split_mode)
        };

        let discovered_steam_paths = self.discover_steam_api_dll_paths();

        if self.config.symlink_game
            || self.config.symlink_exe
            || self.config.symlink_folders
            || self.config.hardlink_game
        {
            if let Some(ref platform) = self.platform {
                let _ = platform.elevation_manager().ensure_symlink_support();
            }
        }

        let total_players = self.player_count;
        for i in 0..total_players {
            let assignment = self
                .player_device_assignments
                .get(i)
                .cloned()
                .unwrap_or_default();
            let bounds = calculated_splits.get(i).copied().unwrap_or_default();
            self.prepare_single_instance(
                i,
                &assignment,
                bounds,
                discovered_steam_paths.as_deref(),
                total_players,
            )?;
        }

        Ok(())
    }

    fn launch_instance(&mut self, player_index: usize) -> NucleusResult<u32> {
        if player_index >= self.contexts.len() {
            return Err(NucleusError::Other(format!(
                "Invalid player index {} (total contexts: {})",
                player_index,
                self.contexts.len()
            )));
        }

        // Interactive prompts between instances or after the first instance (parity with C# GenericGameHandler.cs:2223-2240)
        if player_index > 0 {
            if player_index == 1 && self.config.prompt_after_first_instance {
                self.prompt_user("Press OK when ready to launch the rest of the instances.");
            }
            if self.config.prompt_between_instances {
                self.prompt_user(&format!(
                    "Press OK when ready to launch instance {}.",
                    player_index + 1
                ));
            }
        }

        HookManager::pre_launch_hooks(player_index)?;

        let ctx = &self.contexts[player_index];
        let exe = PathBuf::from(&ctx.exe_path);
        let working_dir = exe.parent().unwrap_or(&self.game_dir);

        let app_id = if self.config.steam_id.is_empty() {
            None
        } else {
            Some(self.config.steam_id.as_str())
        };
        let player_nick = format!("Player{}", player_index + 1);
        let player_env = if self.config.use_nucleus_environment
            || self.config.legacy.force_environment_use
            || self.config.user_profile_config_path.is_some()
            || self.config.user_profile_save_path.is_some()
            || self.config.documents_config_path.is_some()
            || self.config.documents_save_path.is_some()
        {
            match nucleus_tools::users::UsersTool::create_player_environment(&player_nick) {
                Ok(env) => {
                    // Each path is copied from the real profile unless the handler opts out
                    // (`*NoCopy`), and refreshed on every launch when it asks (`Force*Copy`).
                    let l = &self.config.legacy;
                    if let Some(ref path) = self.config.user_profile_config_path {
                        if !l.user_profile_config_path_no_copy {
                            let _ = nucleus_tools::users::UsersTool::copy_user_profile_config_path(
                                &env,
                                path,
                                l.force_user_profile_config_copy,
                            );
                        }
                    }
                    if let Some(ref path) = self.config.user_profile_save_path {
                        if !self.config.user_profile_save_path_no_copy {
                            let _ = nucleus_tools::users::UsersTool::copy_user_profile_config_path(
                                &env,
                                path,
                                l.force_user_profile_save_copy,
                            );
                        }
                    }
                    if let Some(ref path) = self.config.documents_config_path {
                        if !l.documents_config_path_no_copy {
                            let _ = nucleus_tools::users::UsersTool::copy_documents_path(
                                &env,
                                path,
                                l.force_documents_config_copy,
                            );
                        }
                    }
                    if let Some(ref path) = self.config.documents_save_path {
                        if !l.documents_save_path_no_copy {
                            let _ = nucleus_tools::users::UsersTool::copy_documents_path(
                                &env,
                                path,
                                l.force_documents_save_copy,
                            );
                        }
                    }
                    Some(env)
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to create player environment for {}: {}",
                        player_nick,
                        err
                    );
                    None
                }
            }
        } else {
            None
        };

        if self.personal_folder_guard.is_none()
            && (self.config.use_nucleus_environment
                || self.config.documents_config_path.is_some()
                || self.config.documents_save_path.is_some())
        {
            if let Some(ref env) = player_env {
                let backup_dir = self.game_dir.join("utils").join("backup");
                if let Ok(guard) = nucleus_tools::users::UsersTool::redirect_personal_folder(
                    &env.documents,
                    Some(&backup_dir),
                ) {
                    self.personal_folder_guard = Some(guard);
                }
            }
        }

        let mut env_vars =
            EnvironmentSetup::build_environment(player_index, app_id, player_env.as_ref());

        if self.config.use_peerparty {
            let utils_dir = self.resolve_utils_dir();
            let pp_tool = nucleus_tools::PeerPartyTool::new(utils_dir);
            let mut pp_cfg = nucleus_tools::PeerPartyConfig::new()
                .with_public_sessions(self.config.peerparty_public)
                .with_copy_saves(self.config.peerparty_copy_saves);
            if let Some(ref room) = self.config.peerparty_room {
                pp_cfg = pp_cfg.with_room_code(room);
            }
            if let Some(ref services) = self.config.peerparty_services {
                let parsed_services = services
                    .iter()
                    .filter_map(|s| s.parse::<nucleus_tools::PeerPartyService>().ok())
                    .collect::<Vec<_>>();
                if !parsed_services.is_empty() {
                    pp_cfg.services = parsed_services;
                }
            }
            let pp_envs = pp_tool.generate_env_vars(
                player_index,
                &ctx.nickname,
                Path::new(&ctx.instance_folder),
                &pp_cfg,
            );
            env_vars.extend(pp_envs);
        }

        let args: Vec<String> = if ctx.start_arguments.is_empty() {
            Vec::new()
        } else {
            crate::launch::split_command_line(&ctx.start_arguments)
        };

        // Before launching an instance, make sure all active sibling instances have had their single-instance mutexes killed
        if let Some(ref patterns) = self.config.kill_mutex {
            if !patterns.is_empty() {
                if let Some(ref platform) = self.platform {
                    for (prev_idx, prev_ctx) in self.contexts.iter().enumerate() {
                        if prev_idx == player_index || !prev_ctx.is_active {
                            continue;
                        }
                        if let Some(prev_pid) = prev_ctx.process_id {
                            tracing::info!(
                                "Ensuring mutexes killed for active player {} (PID {}) before launching player {}",
                                prev_idx,
                                prev_pid,
                                player_index
                            );
                            let _ = MutexManager::kill_matching_mutexes_typed(
                                platform.mutex_manager(),
                                prev_pid,
                                patterns,
                                &self.config.kill_mutex_type,
                            );
                        }
                    }
                }
            }
        }

        let nickname = ctx.nickname.as_str();
        let osd_msg = format!(
            "Starting {} instance for {} as Player #{}",
            self.config.game_name,
            nickname,
            player_index + 1
        );
        tracing::info!("[OSD] {}", osd_msg);
        if let Some(ref platform) = self.platform {
            platform.osd_manager().show(1200, &osd_msg);
        }

        // Trigger AutoHotkey Immediate scripts for player 0
        if player_index == 0 {
            self.trigger_ahk_scripts(nucleus_tools::autohotkey::AhkTiming::Immediate, 0);
        }

        // Trigger PreLaunch AutoHotkey scripts & Ludusavi restore
        self.trigger_ahk_scripts(
            nucleus_tools::autohotkey::AhkTiming::PreLaunch,
            player_index,
        );
        let _ = self.sync_ludusavi_slot(player_index, false);

        if let Some(ref mgr) = self.extension_manager {
            let _ = mgr.lock().dispatch_lifecycle_hook(
                nucleus_plugin::LifecycleHookKind::PreLaunch,
                Some(Path::new(&ctx.instance_folder)),
                player_index as i32,
            );
        }

        let has_startup_dlls = self.config.injected_dlls.iter().any(|d| {
            d.target_instances
                .applies_to(player_index, self.contexts.len())
                && d.timing == nucleus_common::dll_injection::InjectionTiming::AtStartupSuspended
        });

        let dry_run = self
            .debug_controller
            .as_ref()
            .map(|c| c.is_dry_run())
            .unwrap_or(false);
        let mut proto_wanted = !dry_run
            && !self.proto_unavailable
            && nucleus_script::legacy_props::proto_input_requested(&self.config.proto_input);
        if proto_wanted && self.proto_session.is_none() {
            let root = self.resolve_nucleus_root();
            match crate::proto_launch::ProtoSession::open(&root) {
                Ok(mut session) => {
                    tracing::info!("ProtoInput found in {}", session.dll_folder().display());
                    session.set_extra_config(self.config.proto_extra.clone());
                    self.proto_session = Some(session);
                }
                Err(e) if self.config.proto_from_legacy_hooks => {
                    tracing::warn!(
                        "This handler's hook settings need ProtoInput, which is not installed; \
                         continuing without them. {e}"
                    );
                    if let Some(ref platform) = self.platform {
                        platform
                            .osd_manager()
                            .show(4000, "ProtoInput not installed: input hooks disabled");
                    }
                    self.proto_unavailable = true;
                    proto_wanted = false;
                }
                Err(e) => {
                    tracing::error!("{e}");
                    if let Some(ref platform) = self.platform {
                        platform
                            .osd_manager()
                            .show(4000, "ProtoInput DLLs not found");
                    }
                    return Err(e);
                }
            }
        }
        let assignment = self
            .player_device_assignments
            .get(player_index)
            .cloned()
            .unwrap_or_default();
        let manual_controllers = self.config.legacy.use_manual_proto_controllers_setup;

        let proto_startup_pid = if proto_wanted && self.config.proto_input.inject_startup {
            let session = self
                .proto_session
                .as_mut()
                .expect("ProtoInput session opened above");
            let cfg = &self.config.proto_input;
            let devices = &self.raw_devices;
            let spawned = session.start_with_injection(
                player_index,
                &exe,
                &ctx.start_arguments,
                &env_vars,
                cfg,
                |handle| {
                    crate::proto_launch::player_config(
                        cfg,
                        ctx,
                        &assignment,
                        devices,
                        manual_controllers,
                        handle,
                    )
                },
            )?;
            tracing::info!(
                "Started player {} through ProtoInput startup injection (PID {})",
                player_index + 1,
                spawned
            );
            Some(spawned)
        } else {
            None
        };

        let separate_user = (self.config.legacy.launch_as_different_users
            || self.config.legacy.launch_as_different_users_alt)
            && !dry_run;
        let user_manager = self
            .platform
            .as_ref()
            .and_then(|p| p.user_account_manager())
            .filter(|_| separate_user && proto_startup_pid.is_none());

        let pid = if let Some(spawned) = proto_startup_pid {
            spawned
        } else if let Some(users) = user_manager {
            let username = format!("nucleusplayer{}", player_index + 1);
            if !users.user_exists(&username).unwrap_or(false) {
                users
                    .create_user(&username, &self.nucleus_account_password)
                    .map_err(|e| {
                        NucleusError::Other(format!(
                            "This handler runs each instance as its own Windows user, and the \
                             account '{username}' could not be created. Start Nucleus Co-op as \
                             administrator once so it can create the accounts. ({e})"
                        ))
                    })?;
                tracing::info!("Created Windows account {username}");
            }
            let proc_cfg = nucleus_common::types::ProcessConfig {
                exe_path: exe.clone(),
                args: args.clone(),
                working_dir: Some(working_dir.to_path_buf()),
                environment: env_vars.clone(),
                run_as_admin: false,
                run_as_user: Some(username.clone()),
            };
            let handle = users.run_as_user(&username, &self.nucleus_account_password, &proc_cfg)?;
            tracing::info!(
                "Started player {} as Windows user {} (PID {})",
                player_index + 1,
                username,
                handle.pid
            );
            handle.pid
        } else if has_startup_dlls && self.platform.is_some() {
            let p_mgr = self.platform.as_ref().unwrap().process_manager();
            let proc_cfg = nucleus_common::types::ProcessConfig {
                exe_path: exe.clone(),
                args: args.clone(),
                working_dir: Some(working_dir.to_path_buf()),
                environment: env_vars.clone(),
                run_as_admin: false,
                run_as_user: None,
            };
            let handle = p_mgr.start_process_extended(&proc_cfg, true)?;
            let spawned_pid = handle.pid;

            // Inject AtStartupSuspended DLLs
            for dll_cfg in &self.config.injected_dlls {
                if dll_cfg
                    .target_instances
                    .applies_to(player_index, self.contexts.len())
                    && dll_cfg.timing
                        == nucleus_common::dll_injection::InjectionTiming::AtStartupSuspended
                {
                    let resolved_cfg = self.resolve_dll_path(dll_cfg, player_index);
                    match nucleus_hook::injector::DllInjector::inject_advanced(
                        spawned_pid,
                        &resolved_cfg,
                    ) {
                        Ok(h) => {
                            tracing::info!(
                                "Injected startup DLL {:?} into PID {} (handle {:#x})",
                                resolved_cfg.path,
                                spawned_pid,
                                h
                            );
                            self.injected_modules.push((
                                spawned_pid,
                                h,
                                resolved_cfg.eject_on_close,
                            ));
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to inject startup DLL {:?} into PID {}: {}",
                                resolved_cfg.path,
                                spawned_pid,
                                e
                            );
                            if resolved_cfg.required {
                                let _ = p_mgr.kill_process(spawned_pid);
                                return Err(e);
                            }
                        }
                    }
                }
            }

            // Resume the process
            p_mgr.resume_process(spawned_pid)?;
            spawned_pid
        } else if let Some(ref ctrl) = self.debug_controller {
            ctrl.set_stage(nucleus_debug::protocol::PipelineStage::ProcessSpawn);
            if ctrl.is_dry_run() {
                tracing::info!(
                    "DebugController dry-run mode: simulating process spawn for player {}",
                    player_index
                );
                10000 + (player_index as u32)
            } else {
                ProcessLauncher::launch_process(&exe, &args, Some(working_dir), &env_vars)?
            }
        } else {
            ProcessLauncher::launch_process(&exe, &args, Some(working_dir), &env_vars)?
        };
        self.contexts[player_index].process_id = Some(pid);
        self.contexts[player_index].is_active = true;
        self.sync_launched_pids();

        if proto_wanted {
            if let Some(method) = crate::proto_launch::runtime_method(&self.config.proto_input) {
                let cfg = &self.config.proto_input;
                let devices = &self.raw_devices;
                let ctx = &self.contexts[player_index];
                let session = self
                    .proto_session
                    .as_mut()
                    .expect("ProtoInput session opened above");
                let injected = session.inject_running(player_index, pid, method, cfg, |handle| {
                    crate::proto_launch::player_config(
                        cfg,
                        ctx,
                        &assignment,
                        devices,
                        manual_controllers,
                        handle,
                    )
                });
                if let Err(e) = injected {
                    tracing::error!(
                        "ProtoInput runtime injection failed for player {} (PID {}): {}",
                        player_index + 1,
                        pid,
                        e
                    );
                    let _ = ProcessLauncher::terminate(pid);
                    return Err(e);
                }
                tracing::info!(
                    "Injected ProtoInput into player {} (PID {}) using {:?}",
                    player_index + 1,
                    pid,
                    method
                );
            }
        }

        // Inject PostStartup DLLs
        for dll_cfg in &self.config.injected_dlls {
            if dll_cfg
                .target_instances
                .applies_to(player_index, self.contexts.len())
                && dll_cfg.timing == nucleus_common::dll_injection::InjectionTiming::PostStartup
            {
                let resolved_cfg = self.resolve_dll_path(dll_cfg, player_index);
                match nucleus_hook::injector::DllInjector::inject_advanced(pid, &resolved_cfg) {
                    Ok(h) => {
                        tracing::info!(
                            "Injected post-startup DLL {:?} into PID {} (handle {:#x})",
                            resolved_cfg.path,
                            pid,
                            h
                        );
                        self.injected_modules
                            .push((pid, h, resolved_cfg.eject_on_close));
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to inject post-startup DLL {:?} into PID {}: {}",
                            resolved_cfg.path,
                            pid,
                            e
                        );
                        if resolved_cfg.required {
                            let _ = ProcessLauncher::terminate(pid);
                            return Err(e);
                        }
                    }
                }
            }
        }

        // Mutex handling immediately after launch (if delay specified)
        if let Some(ref patterns) = self.config.kill_mutex {
            if !patterns.is_empty() {
                if let Some(ref platform) = self.platform {
                    let delay_ms = self.config.kill_mutex_delay.max(0) as u64;
                    if delay_ms > 0 {
                        let _ = MutexManager::wait_and_kill_matching_mutexes_typed(
                            platform.mutex_manager(),
                            pid,
                            patterns,
                            &self.config.kill_mutex_type,
                            std::time::Duration::from_millis(delay_ms),
                            std::time::Duration::from_millis(150),
                        );
                    } else {
                        let _ = MutexManager::kill_matching_mutexes_typed(
                            platform.mutex_manager(),
                            pid,
                            patterns,
                            &self.config.kill_mutex_type,
                        );
                    }
                }
            }
        }

        let input_hook = self.platform.as_ref().map(|p| p.input_hook_manager());
        HookManager::post_launch_hooks(input_hook, pid, player_index)?;

        // Per-instance audio routing if configured
        if let Some(ref audio_assignments) = self.player_audio_device_assignments {
            if let Some(endpoint_id) = audio_assignments.get(player_index) {
                if !endpoint_id.is_empty() && endpoint_id != "{default-playback-device}" {
                    tracing::info!(
                        "Routing Player {} (PID {}) audio to endpoint: {}",
                        player_index,
                        pid,
                        endpoint_id
                    );
                    let _ =
                        nucleus_tools::audio::AudioRouter::route_process_audio(pid, endpoint_id);
                }
            }
        }

        Ok(pid)
    }

    fn setup_window(&mut self, player_index: usize) -> NucleusResult<()> {
        if !self.config.supports_positioning {
            return Ok(());
        }

        if let Some(ref ctrl) = self.debug_controller {
            ctrl.set_stage(nucleus_debug::protocol::PipelineStage::HwndPoll);
        }

        if let (Some(pid), Some(ref platform)) = (
            self.contexts.get(player_index).and_then(|c| c.process_id),
            self.platform.as_ref(),
        ) {
            let win_mgr = platform.window_manager();

            // Pause before searching/grabbing window if configured (matching C# GenericGameHandler.cs:2265)
            if self.config.pause_between_process_grab > 0 {
                let pause_secs = self.config.pause_between_process_grab as u64;
                tracing::info!(
                    "Pausing for {} seconds before process grab for player {} (PID {})",
                    pause_secs,
                    player_index,
                    pid
                );
                std::thread::sleep(std::time::Duration::from_secs(pause_secs));
            }

            // Poll for the window to appear (games take time to initialize and create an HWND)
            // Left 4 Dead 2 and other 3D games take 10-30s to boot D3D and create the viewport
            let mut found_hwnd = if self
                .debug_controller
                .as_ref()
                .map(|c| c.is_dry_run())
                .unwrap_or(false)
            {
                Some(0x10000 + player_index)
            } else {
                None
            };
            let max_retries = if found_hwnd.is_some() { 0 } else { 200 };
            for _ in 0..max_retries {
                // Poll and kill single-instance mutexes as the game initializes its engine
                if let Some(ref patterns) = self.config.kill_mutex {
                    if !patterns.is_empty() {
                        let _ = MutexManager::kill_matching_mutexes_typed(
                            platform.mutex_manager(),
                            pid,
                            patterns,
                            &self.config.kill_mutex_type,
                        );
                    }
                }

                if let Ok(Some(hwnd)) = WindowSetup::find_window_by_pid(win_mgr, pid) {
                    found_hwnd = Some(hwnd);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            // Once window is found or finished waiting, ensure any newly-created mutex is killed
            if let Some(ref patterns) = self.config.kill_mutex {
                if !patterns.is_empty() {
                    let _ = MutexManager::kill_matching_mutexes_typed(
                        platform.mutex_manager(),
                        pid,
                        patterns,
                        &self.config.kill_mutex_type,
                    );
                }
            }

            if let Some(hwnd) = found_hwnd {
                if let Some(ref session) = self.proto_session {
                    session.set_main_window(player_index, hwnd as u64);
                }
                tracing::info!(
                    "Found window HWND {:#x} for player {} (PID {})",
                    hwnd,
                    player_index,
                    pid
                );
                self.contexts[player_index].window_handle = Some(hwnd);

                // Wait 1s for graphics context initialization (matching C# Thread.Sleep(1000))
                std::thread::sleep(std::time::Duration::from_millis(1000));

                if let Ok(rect) =
                    win_mgr.get_window_rect(nucleus_common::types::WindowHandle(hwnd as u64))
                {
                    if rect.width > 0 && rect.height > 0 {
                        self.contexts[player_index].orig_width = Some(rect.width);
                        self.contexts[player_index].orig_height = Some(rect.height);
                    }
                }
                if self.contexts[player_index].orig_width.is_none() {
                    self.contexts[player_index].orig_width =
                        Some(self.contexts[player_index].width);
                    self.contexts[player_index].orig_height =
                        Some(self.contexts[player_index].height);
                }

                let ctx = &self.contexts[player_index];
                let mut bounds = WindowBounds {
                    x: ctx.pos_x,
                    y: ctx.pos_y,
                    width: ctx.width,
                    height: ctx.height,
                };

                let should_keep_aspect = self.config.keep_aspect_ratio
                    || self.config.keep_monitor_aspect_ratio
                    || ctx.keep_aspect_ratio;

                if should_keep_aspect {
                    let (orig_w, orig_h) =
                        if self.config.keep_monitor_aspect_ratio || ctx.orig_width.is_none() {
                            (ctx.monitor_width, ctx.monitor_height)
                        } else if let (Some(w), Some(h)) = (ctx.orig_width, ctx.orig_height) {
                            (w, h)
                        } else {
                            (ctx.monitor_width, ctx.monitor_height)
                        };
                    bounds = WindowSetup::apply_aspect_ratio_correction(bounds, orig_w, orig_h);
                }

                let _ = WindowSetup::make_borderless(win_mgr, hwnd);

                // Support custom window styles (WindowStyleValues / ExtWindowStyleValues)
                if self.config.window_style_values.is_some()
                    || self.config.ext_window_style_values.is_some()
                {
                    let styles = self.config.window_style_values.as_deref().unwrap_or(&[]);
                    let ext_styles = self
                        .config
                        .ext_window_style_values
                        .as_deref()
                        .unwrap_or(&[]);
                    let _ = win_mgr.apply_custom_window_styles(
                        nucleus_common::types::WindowHandle(hwnd as u64),
                        styles,
                        ext_styles,
                    );
                }

                if !self.config.dont_reposition && !self.config.dont_resize {
                    let _ = WindowSetup::apply_bounds_ext(
                        win_mgr,
                        hwnd,
                        bounds,
                        self.config.not_top_most,
                    );
                    // 1000ms settle delay after applying window bounds before subsequent operations (parity with C# GenericGameHandler.cs:3203, 3207)
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }

                // C# parity: Set game window to HWND_TOPMOST unless NotTopMost is configured.
                // If SetTopMostAtEnd is configured, this is deferred until all instances are launched.
                if !self.config.not_top_most && !self.config.set_top_most_at_end {
                    let _ =
                        win_mgr.set_topmost(nucleus_common::types::WindowHandle(hwnd as u64), true);
                }

                // Hide taskbar if requested by handler
                if self.config.hide_taskbar {
                    let _ = platform.taskbar_manager().hide();
                }

                let skip_fake_focus = self.config.legacy.keyboard_player_skip_fake_focus
                    && self.contexts[player_index].is_keyboard_player;
                if self.config.fake_focus && !skip_fake_focus {
                    let _ = platform
                        .input_hook_manager()
                        .send_fake_focus(nucleus_common::types::WindowHandle(hwnd as u64));
                }

                // Trigger AutoHotkey PostWindow scripts
                self.trigger_ahk_scripts(
                    nucleus_tools::autohotkey::AhkTiming::PostWindow,
                    player_index,
                );

                // Dispatch Extension Lifecycle PostWindow Hook
                if let Some(ref mgr) = self.extension_manager {
                    let inst_folder = PathBuf::from(&self.contexts[player_index].instance_folder);
                    let _ = mgr.lock().dispatch_lifecycle_hook(
                        nucleus_plugin::LifecycleHookKind::PostWindow,
                        Some(&inst_folder),
                        player_index as i32,
                    );
                }

                // Trigger OnWindowCreated and AfterDelayMs DLL injections
                for dll_cfg in &self.config.injected_dlls {
                    if !dll_cfg
                        .target_instances
                        .applies_to(player_index, self.contexts.len())
                    {
                        continue;
                    }
                    match dll_cfg.timing {
                        nucleus_common::dll_injection::InjectionTiming::OnWindowCreated => {
                            let resolved_cfg = self.resolve_dll_path(dll_cfg, player_index);
                            match nucleus_hook::injector::DllInjector::inject_advanced(
                                pid,
                                &resolved_cfg,
                            ) {
                                Ok(h) => {
                                    tracing::info!(
                                        "Injected on-window DLL {:?} into PID {} (handle {:#x})",
                                        resolved_cfg.path,
                                        pid,
                                        h
                                    );
                                    self.injected_modules.push((
                                        pid,
                                        h,
                                        resolved_cfg.eject_on_close,
                                    ));
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to inject on-window DLL {:?} into PID {}: {}",
                                        resolved_cfg.path,
                                        pid,
                                        e
                                    );
                                    if resolved_cfg.required {
                                        return Err(e);
                                    }
                                }
                            }
                        }
                        nucleus_common::dll_injection::InjectionTiming::AfterDelayMs(delay) => {
                            let delay_ms = if delay > 0 { delay } else { dll_cfg.delay_ms };
                            let resolved_cfg = self.resolve_dll_path(dll_cfg, player_index);
                            std::thread::spawn(move || {
                                if delay_ms > 0 {
                                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                                }
                                let _ = nucleus_hook::injector::DllInjector::inject_advanced(
                                    pid,
                                    &resolved_cfg,
                                );
                            });
                        }
                        _ => {}
                    }
                }
            } else {
                tracing::warn!(
                    "Timeout waiting for window handle for player {} (PID {})",
                    player_index,
                    pid
                );
            }
        }

        Ok(())
    }

    /// Re-applies borderless styles and bounds to an existing active instance window.
    fn reposition_instance_window(&self, player_index: usize) -> NucleusResult<bool> {
        if !self.config.supports_positioning {
            return Ok(false);
        }
        let Some(ctx) = self.contexts.get(player_index) else {
            return Ok(false);
        };
        if !ctx.is_active {
            return Ok(false);
        }
        if let (Some(pid), Some(ref platform)) = (ctx.process_id, self.platform.as_ref()) {
            let win_mgr = platform.window_manager();
            if let Ok(Some(hwnd)) = WindowSetup::find_window_by_pid(win_mgr, pid) {
                let ctx = &self.contexts[player_index];
                let mut bounds = WindowBounds {
                    x: ctx.pos_x,
                    y: ctx.pos_y,
                    width: ctx.width,
                    height: ctx.height,
                };

                let should_keep_aspect = self.config.keep_aspect_ratio
                    || self.config.keep_monitor_aspect_ratio
                    || ctx.keep_aspect_ratio;

                if should_keep_aspect {
                    let (orig_w, orig_h) =
                        if self.config.keep_monitor_aspect_ratio || ctx.orig_width.is_none() {
                            (ctx.monitor_width, ctx.monitor_height)
                        } else if let (Some(w), Some(h)) = (ctx.orig_width, ctx.orig_height) {
                            (w, h)
                        } else {
                            (ctx.monitor_width, ctx.monitor_height)
                        };
                    bounds = WindowSetup::apply_aspect_ratio_correction(bounds, orig_w, orig_h);
                }

                let _ = WindowSetup::make_borderless(win_mgr, hwnd);
                if !self.config.dont_reposition && !self.config.dont_resize {
                    let _ = WindowSetup::apply_bounds_ext(
                        win_mgr,
                        hwnd,
                        bounds,
                        self.config.not_top_most,
                    );
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Dynamically switches/cycles splitscreen layout partitions for active instances.
    fn switch_layout(&mut self) -> NucleusResult<String> {
        let current_mode = self.split_mode;
        let active_count = self.active_slot_indices().len().max(1);
        let next_mode = match (active_count, current_mode) {
            (2, crate::window::SplitMode::Vertical) => crate::window::SplitMode::Horizontal,
            (2, crate::window::SplitMode::Horizontal) => crate::window::SplitMode::Vertical,
            (3, crate::window::SplitMode::TopOneBottomTwo) => {
                crate::window::SplitMode::TopTwoBottomOne
            }
            (3, crate::window::SplitMode::TopTwoBottomOne) => {
                crate::window::SplitMode::TopOneBottomTwo
            }
            (4, crate::window::SplitMode::Grid4) => crate::window::SplitMode::Horizontal,
            (4, crate::window::SplitMode::Horizontal) => crate::window::SplitMode::Vertical,
            (4, crate::window::SplitMode::Vertical) => crate::window::SplitMode::Grid4,
            (_, crate::window::SplitMode::Vertical) => crate::window::SplitMode::Horizontal,
            (_, crate::window::SplitMode::Horizontal) => crate::window::SplitMode::Grid4,
            (_, crate::window::SplitMode::Grid4) => crate::window::SplitMode::TopOneBottomTwo,
            (_, crate::window::SplitMode::TopOneBottomTwo) => {
                crate::window::SplitMode::TopTwoBottomOne
            }
            (_, _) => crate::window::SplitMode::Vertical,
        };
        self.split_mode = next_mode;
        self.is_layout_switched = true;
        self.re_tile_active_instances(next_mode)?;

        let desc = match next_mode {
            crate::window::SplitMode::Vertical => "Side-by-Side (2x1)",
            crate::window::SplitMode::Horizontal => "Stacked (1x2)",
            crate::window::SplitMode::Grid4 => "2x2 Grid",
            crate::window::SplitMode::Grid6 => "2x3 Grid (6 Players)",
            crate::window::SplitMode::Grid8 => "2x4 Grid (8 Players)",
            crate::window::SplitMode::Grid16 => "4x4 Grid (16 Players)",
            crate::window::SplitMode::CustomGrid { rows: _, cols: _ } => "Custom Regular Grid",
            crate::window::SplitMode::TopOneBottomTwo => "Top 1 / Bottom 2",
            crate::window::SplitMode::TopTwoBottomOne => "Top 2 / Bottom 1",
            crate::window::SplitMode::Auto => "Auto",
        };
        tracing::info!("GameHandler::switch_layout: Switched layout to {}", desc);
        Ok(desc.to_string())
    }

    fn toggle_minimize_windows(&mut self) -> NucleusResult<bool> {
        let platform = match self.platform.as_ref() {
            Some(p) => p,
            None => return Ok(false),
        };
        let win_mgr = platform.window_manager();

        if !self.windows_minimized {
            // Minimize all active instances (ShowHideWindows parity)
            let mut minimized_any = false;
            for ctx in &self.contexts {
                if !ctx.is_active {
                    continue;
                }
                if let Some(hwnd) = ctx.window_handle {
                    let handle = nucleus_common::types::WindowHandle(hwnd as u64);
                    let _ = win_mgr.set_topmost(handle, false);
                    let _ = win_mgr.minimize_window(handle);
                    minimized_any = true;
                }
            }
            if minimized_any {
                let _ = platform.taskbar_manager().show();
                platform.osd_manager().show(1600, "Game Windows Minimized");
                self.windows_minimized = true;
                return Ok(true);
            } else {
                platform.osd_manager().show(1600, "Can't Be Used Now");
            }
        } else {
            // Restore all active instances
            let _ = platform.taskbar_manager().hide();
            let mut restored_any = false;
            for (i, ctx) in self.contexts.iter().enumerate() {
                if !ctx.is_active {
                    continue;
                }
                if let Some(hwnd) = ctx.window_handle {
                    let handle = nucleus_common::types::WindowHandle(hwnd as u64);
                    let _ = win_mgr.restore_window(handle);
                    let _ = win_mgr.set_topmost(handle, true);
                    let _ = self.reposition_instance_window(i);
                    restored_any = true;
                }
            }
            if restored_any {
                platform.osd_manager().show(1600, "Game Windows Restored");
                self.windows_minimized = false;
                return Ok(false);
            } else {
                platform.osd_manager().show(1600, "Can't Be Used Now");
            }
        }
        Ok(self.windows_minimized)
    }

    fn toggle_cutscenes_mode(&mut self) -> NucleusResult<bool> {
        let Some(first_active_idx) = self
            .contexts
            .iter()
            .enumerate()
            .find(|(_, c)| c.is_active && c.window_handle.is_some())
            .map(|(i, _)| i)
        else {
            if let Some(ref platform) = self.platform {
                platform.osd_manager().show(1600, "Can't Be Used Now");
            }
            return Ok(false);
        };

        let primary_screen = self.resolve_primary_screen();
        let platform = match self.platform.as_ref() {
            Some(p) => p,
            None => return Ok(false),
        };
        let win_mgr = platform.window_manager();

        if !self.cutscenes_active {
            // Cutscenes Mode ON: First active player fullscreen, hide/minimize other active instances
            if let Some(first_ctx) = self.contexts.get(first_active_idx) {
                if let Some(hwnd) = first_ctx.window_handle {
                    let _ =
                        win_mgr.restore_window(nucleus_common::types::WindowHandle(hwnd as u64));
                    let _ = WindowSetup::apply_bounds(
                        win_mgr,
                        hwnd,
                        WindowBounds {
                            x: primary_screen.x,
                            y: primary_screen.y,
                            width: primary_screen.width,
                            height: primary_screen.height,
                        },
                    );
                    let _ =
                        win_mgr.set_topmost(nucleus_common::types::WindowHandle(hwnd as u64), true);
                }
            }

            // Other active players: minimize / hide
            for (i, ctx) in self.contexts.iter().enumerate() {
                if !ctx.is_active || i == first_active_idx {
                    continue;
                }
                if let Some(hwnd) = ctx.window_handle {
                    let handle = nucleus_common::types::WindowHandle(hwnd as u64);
                    let _ = win_mgr.set_topmost(handle, false);
                    let _ = win_mgr.minimize_window(handle);
                }
            }

            platform.osd_manager().show(1600, "Cutscenes Mode On");
            self.cutscenes_active = true;
            Ok(true)
        } else {
            // Cutscenes Mode OFF: restore all active players to their split bounds
            for (i, ctx) in self.contexts.iter().enumerate() {
                if !ctx.is_active {
                    continue;
                }
                if let Some(hwnd) = ctx.window_handle {
                    let handle = nucleus_common::types::WindowHandle(hwnd as u64);
                    let _ = win_mgr.restore_window(handle);
                    let _ = win_mgr.set_topmost(handle, true);
                    let _ = self.reposition_instance_window(i);
                }
            }

            platform.osd_manager().show(1600, "Cutscenes Mode Off");
            self.cutscenes_active = false;
            Ok(false)
        }
    }

    fn finalize_instances(&self) -> NucleusResult<()> {
        self.replay_proto_callback(nucleus_script::proto_script::ProtoCallback::FinishedSetup);
        // `Game.LockInputAtStart`: hand the devices to the instances as soon as setup is done.
        if self.config.legacy.lock_input_at_start && self.proto_session.is_some() {
            nucleus_input::request_input_lock(true);
        }
        if let Some(ref platform) = self.platform {
            let win_mgr = platform.window_manager();
            if self.config.set_top_most_at_end && !self.config.not_top_most {
                for ctx in &self.contexts {
                    if !ctx.is_active {
                        continue;
                    }
                    if let Some(hwnd) = ctx.window_handle {
                        let _ = win_mgr
                            .set_topmost(nucleus_common::types::WindowHandle(hwnd as u64), true);
                    }
                }
            }
            if self.config.hide_taskbar {
                let _ = platform.taskbar_manager().hide();
            }
            // Parity with C# GenericGameHandler/GameProfile: show Splitscreen Setup Finished notification
            platform
                .osd_manager()
                .show(1600, "Splitscreen Setup Finished");
        }
        Ok(())
    }

    fn stop(&mut self) -> NucleusResult<()> {
        tracing::info!(
            "GameHandler::stop: Starting session teardown sequence (parity with C# GenericGameHandler.End)..."
        );

        // 0. Terminate all running AutoHotkey scripts & dispatch Plugin OnStop hook
        self.ahk_manager.stop_all();
        if let Some(ref mgr) = self.extension_manager {
            let _ = mgr.lock().dispatch_lifecycle_hook(
                nucleus_plugin::LifecycleHookKind::OnStop,
                None,
                0,
            );
        }

        // Backup active instance savegames via Ludusavi if auto-sync enabled
        for i in 0..self.contexts.len() {
            let _ = self.sync_ludusavi_slot(i, true);
        }

        // 1. Unclip mouse cursor, unblock input hooks, and restore Windows taskbar
        if let Some(ref platform) = self.platform {
            let _ = platform.window_manager().unclip_cursor();
            let input_mgr = platform.input_hook_manager();
            for &pid in &self.launched_pids {
                let _ = input_mgr.block_raw_input(pid, false);
            }
            let _ = platform.taskbar_manager().show();
        }

        // 2. Invoke Game.OnStop() callback if defined in handler script
        if let Some(ref script) = self.script_source {
            if script.contains("OnStop") {
                tracing::info!("Invoking handler Game.OnStop callback...");
                if let Ok(mut engine) = nucleus_script::ScriptEngine::new() {
                    let _ = engine.load_handler_script(script);
                    let _ = engine.execute("if (typeof Game !== 'undefined' && typeof Game.OnStop === 'function') { Game.OnStop(); }");
                }
            }
        }

        // Eject any injected modules configured with eject_on_close before process termination
        for &(pid, module_handle, eject_on_close) in &self.injected_modules {
            if eject_on_close && module_handle != 0 {
                tracing::info!("Ejecting module {:#x} from PID {}", module_handle, pid);
                let _ = nucleus_hook::injector::DllInjector::eject_dll(pid, module_handle);
            }
        }
        self.injected_modules.clear();

        // 3. Terminate launched processes and child process trees
        for pid in self.launched_pids.drain(..) {
            tracing::info!("Terminating process tree for PID {}", pid);
            let _ = ProcessLauncher::terminate(pid);
        }

        // 4. Terminate any explicit processes declared in Game.KillProcessesOnClose
        if let Some(ref procs) = self.config.kill_processes_on_close {
            for proc_name in procs {
                let name = proc_name.trim();
                if !name.is_empty() {
                    #[cfg(windows)]
                    {
                        let image_name = if name.ends_with(".exe") {
                            name.to_string()
                        } else {
                            format!("{}.exe", name)
                        };
                        let _ = std::process::Command::new("taskkill")
                            .args(["/F", "/T", "/IM", &image_name])
                            .output();
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = std::process::Command::new("pkill")
                            .args(["-9", "-f", name])
                            .output();
                    }
                }
            }

            // Also terminate any unmanaged child processes spawned by the primary game executable
            if let Some(exe_name) = self.exe_path.file_name().and_then(|n| n.to_str()) {
                #[cfg(windows)]
                {
                    let image_name = if exe_name.ends_with(".exe") {
                        exe_name.to_string()
                    } else {
                        format!("{}.exe", exe_name)
                    };
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/T", "/IM", &image_name])
                        .output();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("pkill")
                        .args(["-9", "-f", exe_name])
                        .output();
                }
            }
        }

        // 5. Execute CMDBatchClose batch commands via cmd.exe
        if let Some(ref batch_cmds) = self.config.cmd_batch_close {
            for cmd_str in batch_cmds {
                let trimmed = cmd_str.trim();
                if !trimmed.is_empty() {
                    tracing::info!("Executing CMDBatchClose: {}", trimmed);
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("cmd.exe")
                            .args(["/C", trimmed])
                            .output();
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = std::process::Command::new("sh")
                            .args(["-c", trimmed])
                            .output();
                    }
                }
            }
        }

        // 6. Delete temporary files declared in Game.DeleteOnClose
        if let Some(ref del_files) = self.config.delete_on_close {
            for rel_file in del_files {
                let target = self.game_dir.join(rel_file);
                if target.is_file() {
                    tracing::info!("DeleteOnClose: deleting {:?}", target);
                    let _ = std::fs::remove_file(&target);
                }
                for inst in &self.instance_folders {
                    let inst_target = inst.join(rel_file);
                    if inst_target.is_file() {
                        let _ = std::fs::remove_file(&inst_target);
                    }
                }
            }
        }

        // 7. Restore backed up files
        if let Some(ref backed_files) = self.config.backup_files {
            for rel in backed_files {
                let orig = self.game_dir.join(rel);
                let backup_file = self.game_dir.join(format!("{}.nucleus_bak", rel));
                if backup_file.is_file() {
                    tracing::info!("Restoring backup file {:?} -> {:?}", backup_file, orig);
                    let _ = std::fs::copy(&backup_file, &orig);
                    let _ = std::fs::remove_file(&backup_file);
                }
            }
        }
        if let Ok(entries) = std::fs::read_dir(&self.game_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.contains("_NUCLEUS_BACKUP") {
                        let orig_name = name.replace("_NUCLEUS_BACKUP", "");
                        let orig_path = self.game_dir.join(orig_name);
                        tracing::info!("Restoring _NUCLEUS_BACKUP {:?} -> {:?}", p, orig_path);
                        let _ = std::fs::copy(&p, &orig_path);
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }

        // 8. Restore registry keys and personal shell folder redirection
        if let Some(session) = self.proto_session.take() {
            // Never leave the user's keyboard and mouse locked to instances that are gone.
            if self.applied_input_lock {
                session.set_input_lock(false, false);
                nucleus_input::request_input_lock(false);
            }
        }
        self.applied_input_lock = false;
        if self.keep_edited_reg_keys {
            tracing::info!("Handler asked to keep its registry edits; skipping registry restore");
        } else {
            let restored = nucleus_script::context_ext::registry::restore_backups(
                &self.resolve_nucleus_root(),
            );
            if restored > 0 {
                tracing::info!("Restored {restored} registry key backup(s) edited by the handler");
            }
        }
        self.keep_edited_reg_keys = false;
        if let Some(mut guard) = self.personal_folder_guard.take() {
            let _ = guard.restore();
        }
        let backup_dir = self.game_dir.join("utils").join("backup");
        let _ =
            nucleus_tools::users::UsersTool::restore_user_shell_folders_from_backup(&backup_dir);
        if let Some(ref platform) = self.platform {
            if let Some(reg) = platform.registry_manager() {
                let _ = reg.delete_value("Software\\NucleusCoop", "SessionActive");
            }
        }

        // 9. Clean up instance directories if keep_symlink_on_exit is false
        if !self.config.keep_symlink_on_exit {
            for folder in self.instance_folders.drain(..) {
                tracing::info!("Cleaning instance folder {:?}", folder);
                let _ = InstanceSetup::cleanup_instance_folder(&folder);
            }
        }

        if let Some(ref platform) = self.platform {
            platform.osd_manager().show(2000, "Session Ended");
        }

        tracing::info!("GameHandler::stop: Session teardown complete.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_handler_lifecycle() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_handler_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

        let mut config = HandlerConfig::default();
        config.game_name = "TestSplitScreenGame".to_string();
        config.guid = "test-split-game".to_string();
        config.symlink_game = true;
        config.use_goldberg = true;
        config.max_players = 2;

        let mut handler = GameHandler::new(config, game_dir, exe_path, staging_dir, 2);

        // Test preparation
        handler
            .prepare_instances()
            .expect("Failed to prepare instances");
        assert_eq!(handler.contexts.len(), 2);
        assert_eq!(handler.instance_folders.len(), 2);

        assert!(handler.instance_folders[0].exists());
        assert!(handler.instance_folders[1].exists());

        // Check Goldberg deployment
        let goldberg_cfg_0 = handler.instance_folders[0].join("steam_settings");
        assert!(
            goldberg_cfg_0.exists(),
            "Goldberg steam_settings should be created"
        );

        // Test teardown
        handler.stop().expect("Failed to stop handler");
        assert!(
            !handler.instance_folders.iter().any(|f| f.exists()),
            "Instances should be cleaned up"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_tools_wiring() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_handler_tools_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"DUMMY_GAME_EXE_PAYLOAD").unwrap();

        let mut config = HandlerConfig::default();
        config.game_name = "ToolsGame".to_string();
        config.guid = "tools-game".to_string();
        config.symlink_game = true;
        config.use_dev_reorder = true;
        config.use_x360ce = true;
        config.use_sdl = true;
        config.use_xinput_plus = true;
        // Original format: one "find|replace" entry per player.
        config.hex_edit_exe = Some(vec!["DUMMY|PONE_".to_string(), "DUMMY|PTWO_".to_string()]);
        config.legacy.hex_edit_all_exes = vec!["PAYLOAD|PATCHED".to_string()];
        config.max_players = 2;

        let mut handler = GameHandler::new(config, game_dir, exe_path, staging_dir, 2);
        handler
            .prepare_instances()
            .expect("prepare_instances failed");

        assert_eq!(handler.instance_folders.len(), 2);
        for (idx, folder) in handler.instance_folders.iter().enumerate() {
            // Verify DevReorder
            assert!(
                folder.join("devreorder.ini").exists(),
                "devreorder.ini must exist"
            );
            let dev_content = std::fs::read_to_string(folder.join("devreorder.ini")).unwrap();
            assert!(dev_content.contains("[order]"));
            assert!(dev_content.contains("[hidden]"));

            // Verify x360ce
            assert!(folder.join("x360ce.ini").exists(), "x360ce.ini must exist");
            let x360_content = std::fs::read_to_string(folder.join("x360ce.ini")).unwrap();
            assert!(x360_content.contains("[Mappings]"));

            // Verify SDL2
            assert!(folder.join("SDL2.ini").exists(), "SDL2.ini must exist");

            // Verify XInputPlus
            assert!(
                folder.join("XInputPlus.ini").exists(),
                "XInputPlus.ini must exist"
            );
            let xip_content = std::fs::read_to_string(folder.join("XInputPlus.ini")).unwrap();
            assert!(xip_content.contains("[Device]"));

            // Verify HexEdit
            let patched_exe = std::fs::read(folder.join("game.exe")).unwrap();
            let expected: &[u8] = if idx == 0 {
                b"PONE__GAME_EXE_PATCHED"
            } else {
                b"PTWO__GAME_EXE_PATCHED"
            };
            assert_eq!(
                patched_exe, expected,
                "each instance gets its own HexEditExe entry plus the HexEditAllExes patch"
            );
        }
        // The installed game must never be modified through the instance links.
        assert_eq!(
            std::fs::read(&handler.exe_path).unwrap(),
            b"DUMMY_GAME_EXE_PAYLOAD",
            "hex edits must not write through to the original executable"
        );

        handler.stop().expect("stop failed");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_stop_teardown_parity() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_teardown_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

        // 1. Create a file declared in delete_on_close
        let del_file = game_dir.join("temp_config.tmp");
        std::fs::write(&del_file, b"DELETE_ME_ON_CLOSE").unwrap();
        assert!(del_file.exists());

        // 2. Create a backed up file with _NUCLEUS_BACKUP
        let backup_file = game_dir.join("save_NUCLEUS_BACKUP.dat");
        std::fs::write(&backup_file, b"ORIGINAL_BACKUP_CONTENT").unwrap();
        assert!(backup_file.exists());

        let mut config = HandlerConfig::default();
        config.guid = "teardown-game".to_string();
        config.delete_on_close = Some(vec!["temp_config.tmp".to_string()]);
        config.cmd_batch_close = Some(vec!["echo NucleusCoopTeardownTest".to_string()]);
        config.keep_symlink_on_exit = false;

        let mut handler = GameHandler::new(config, game_dir.clone(), exe_path, staging_dir, 1);
        handler
            .prepare_instances()
            .expect("prepare_instances failed");
        assert_eq!(handler.instance_folders.len(), 1);
        let inst_folder = handler.instance_folders[0].clone();
        assert!(inst_folder.exists());

        // Execute teardown
        handler.stop().expect("stop teardown failed");

        // Verify DeleteOnClose deleted the file
        assert!(
            !del_file.exists(),
            "temp_config.tmp should have been deleted by DeleteOnClose"
        );

        // Verify _NUCLEUS_BACKUP restored original file
        let restored_file = game_dir.join("save.dat");
        assert!(
            restored_file.exists(),
            "save.dat should have been restored from _NUCLEUS_BACKUP"
        );
        assert_eq!(
            std::fs::read(&restored_file).unwrap(),
            b"ORIGINAL_BACKUP_CONTENT"
        );

        // Verify instance folder was cleaned up
        assert!(
            !inst_folder.exists(),
            "Instance folder should have been cleaned up on stop"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_binaries_folder_subpath_preservation() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_binaries_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let bin_dir = game_dir.join("bin");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = bin_dir.join("game.exe");
        std::fs::write(&exe_path, b"GAME_EXE_IN_BIN").unwrap();

        let mut config = HandlerConfig::default();
        config.guid = "binaries-game".to_string();
        config.symlink_game = true;
        config.binaries_folder = "bin".to_string();
        config.max_players = 2;

        let mut handler =
            GameHandler::new(config, game_dir.clone(), exe_path.clone(), staging_dir, 2);
        handler
            .prepare_instances()
            .expect("prepare_instances failed");

        assert_eq!(handler.instance_folders.len(), 2);
        for (i, folder) in handler.instance_folders.iter().enumerate() {
            let expected_exe = folder.join("bin").join("game.exe");
            assert!(
                expected_exe.exists(),
                "game.exe must exist in preserved subpath {:?}",
                expected_exe
            );
            assert_eq!(
                PathBuf::from(&handler.contexts[i].exe_path),
                expected_exe,
                "Context exe_path must match binaries_folder subpath"
            );
        }

        handler.stop().expect("stop failed");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_hardcopy_standalone_staging() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_hardcopy_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        let asset_path = game_dir.join("config.cfg");
        std::fs::write(&exe_path, b"HARDCOPY_EXE").unwrap();
        std::fs::write(&asset_path, b"orig_cfg_value").unwrap();

        let mut config = HandlerConfig::default();
        config.guid = "hardcopy-game".to_string();
        // Crucial test: symlink_game is FALSE, hardcopy_game is TRUE!
        config.symlink_game = false;
        config.hardcopy_game = true;
        config.max_players = 1;

        let mut handler = GameHandler::new(config, game_dir.clone(), exe_path, staging_dir, 1);
        handler
            .prepare_instances()
            .expect("prepare_instances with standalone hardcopy must succeed");

        assert_eq!(handler.instance_folders.len(), 1);
        let inst_folder = &handler.instance_folders[0];
        assert!(
            inst_folder.join("game.exe").exists(),
            "game.exe must be copied"
        );
        assert!(
            inst_folder.join("config.cfg").exists(),
            "config.cfg must be copied"
        );

        // Verify independent hardcopy: modifying instance file must not modify original
        std::fs::write(inst_folder.join("config.cfg"), b"modified_cfg_value").unwrap();
        assert_eq!(
            std::fs::read(&asset_path).unwrap(),
            b"orig_cfg_value",
            "Original file must remain untouched in hardcopy mode"
        );

        handler.stop().expect("stop failed");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_staging_exclusions() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_excl_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"GAME_EXE").unwrap();
        std::fs::write(game_dir.join("xinput1_3.dll"), b"ORIGINAL_CUSTOM_DLL").unwrap();
        std::fs::write(game_dir.join("ncoop.ini"), b"ORIGINAL_NCOOP_INI").unwrap();
        std::fs::write(game_dir.join("launcher.exe"), b"ORIGINAL_LAUNCHER").unwrap();
        std::fs::write(game_dir.join("allowed.dll"), b"ALLOWED_DLL").unwrap();

        let mut config = HandlerConfig::default();
        config.guid = "excl-game".to_string();
        config.symlink_game = true;
        config.hook_custom_dll_enabled = true;
        config.launcher_exe = "launcher.exe".to_string();
        config.max_players = 1;

        let mut handler = GameHandler::new(config, game_dir.clone(), exe_path, staging_dir, 1);
        handler
            .prepare_instances()
            .expect("prepare_instances failed");

        let inst_folder = &handler.instance_folders[0];
        assert!(
            inst_folder.join("allowed.dll").exists(),
            "allowed.dll should be linked"
        );
        assert!(
            !inst_folder.join("xinput1_3.dll").exists(),
            "xinput1_3.dll must be excluded from link staging"
        );
        assert!(
            !inst_folder.join("ncoop.ini").exists(),
            "ncoop.ini must be excluded from link staging"
        );
        assert!(
            !inst_folder.join("launcher.exe").exists(),
            "launcher_exe must be excluded from link staging"
        );

        handler.stop().expect("stop failed");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_keep_symlink_on_exit() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_keep_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"GAME_EXE").unwrap();

        let mut config = HandlerConfig::default();
        config.guid = "keep-symlink-game".to_string();
        config.symlink_game = true;
        config.keep_symlink_on_exit = true; // MUST preserve instance folders!
        config.max_players = 1;

        let mut handler = GameHandler::new(config, game_dir.clone(), exe_path, staging_dir, 1);
        handler
            .prepare_instances()
            .expect("prepare_instances failed");
        let inst_folder = handler.instance_folders[0].clone();
        assert!(inst_folder.exists());

        // Stop handler
        handler.stop().expect("stop failed");

        // Verify folder is PRESERVED when keep_symlink_on_exit is true
        assert!(
            inst_folder.exists(),
            "Instance folder must be preserved when keep_symlink_on_exit is true"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_interactive_prompts() {
        use std::sync::Mutex;

        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_prompt_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        #[cfg(windows)]
        let exe_path = {
            let src = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            let dst = game_dir.join("cmd.exe");
            std::fs::copy(&src, &dst).unwrap();
            dst
        };
        #[cfg(not(windows))]
        let exe_path = {
            let src = PathBuf::from("/bin/true");
            let dst = game_dir.join("true");
            let _ = std::fs::copy(&src, &dst);
            dst
        };

        let mut config = HandlerConfig::default();
        config.guid = "prompt-game".to_string();
        config.prompt_after_first_instance = true;
        config.prompt_between_instances = true;
        config.pause_between_process_grab = 0;
        config.max_players = 2;

        let prompt_messages = Arc::new(Mutex::new(Vec::new()));
        let prompt_messages_clone = prompt_messages.clone();

        let mut handler = GameHandler::new(config, game_dir, exe_path, staging_dir, 2)
            .with_prompt_handler(Arc::new(move |msg| {
                prompt_messages_clone.lock().unwrap().push(msg.to_string());
                true
            }));

        handler
            .prepare_instances()
            .expect("prepare_instances failed");

        // Player 0 launch: no prompt before launch
        let pid0 = handler.launch_instance(0).expect("launch player 0 failed");
        assert_eq!(prompt_messages.lock().unwrap().len(), 0);

        // Player 1 launch: triggers prompt_after_first_instance and prompt_between_instances
        let pid1 = handler.launch_instance(1).expect("launch player 1 failed");
        let msgs = prompt_messages.lock().unwrap().clone();
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].contains("Press OK when ready to launch the rest of the instances."));
        assert!(msgs[1].contains("Press OK when ready to launch instance 2."));

        let _ = ProcessLauncher::terminate(pid0);
        let _ = ProcessLauncher::terminate(pid1);
        handler.stop().expect("stop failed");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_handler_multi_monitor_partitioning() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_multimon_test_{}", std::process::id()));
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        #[cfg(windows)]
        let exe_path = {
            let src = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            let dst = game_dir.join("cmd.exe");
            std::fs::copy(&src, &dst).unwrap();
            dst
        };
        #[cfg(not(windows))]
        let exe_path = {
            let src = PathBuf::from("/bin/true");
            let dst = game_dir.join("true");
            let _ = std::fs::copy(&src, &dst);
            dst
        };

        let mut config = HandlerConfig::default();
        config.guid = "multimon-game".to_string();
        config.max_players = 4;

        // Assign 4 players: player 0 and 1 on monitor 0, player 2 and 3 on monitor 1
        let mut handler = GameHandler::new(config, game_dir, exe_path, staging_dir, 4)
            .with_player_monitor_assignments(vec![0, 0, 1, 1]);

        handler
            .prepare_instances()
            .expect("prepare_instances failed");
        assert_eq!(handler.contexts.len(), 4);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    mod mock_platform {
        use super::*;
        use nucleus_common::types::{
            Display, NetworkInterface, ProcessConfig, ProcessHandle, ProcessInfo, ProcessPriority,
            Rect, WindowHandle, WindowStyle,
        };
        use nucleus_platform::{
            DisplayManager, ElevationManager, FilesystemManager, InputHookManager, MutexManager,
            NetworkManager, OsdManager, ProcessManager, RegistryManager, TaskbarManager,
            UserAccountManager, WindowManager,
        };
        use std::collections::{HashMap, HashSet};
        use std::sync::Mutex;

        #[derive(Default)]
        pub struct MockProcMgr {
            pub killed_pids: Mutex<Vec<u32>>,
            pub running_pids: Mutex<HashSet<u32>>,
        }

        impl ProcessManager for MockProcMgr {
            fn start_process(&self, _config: &ProcessConfig) -> NucleusResult<ProcessHandle> {
                Ok(ProcessHandle::new(9999, "mock.exe"))
            }
            fn kill_process(&self, pid: u32) -> NucleusResult<()> {
                self.killed_pids.lock().unwrap().push(pid);
                self.running_pids.lock().unwrap().remove(&pid);
                Ok(())
            }
            fn get_process_info(&self, pid: u32) -> NucleusResult<ProcessInfo> {
                Ok(ProcessInfo {
                    pid,
                    name: "mock.exe".to_string(),
                    exe_path: PathBuf::new(),
                })
            }
            fn is_process_running(&self, pid: u32) -> bool {
                self.running_pids.lock().unwrap().contains(&pid)
            }
            fn set_affinity(&self, _pid: u32, _mask: u64) -> NucleusResult<()> {
                Ok(())
            }
            fn set_priority(&self, _pid: u32, _priority: ProcessPriority) -> NucleusResult<()> {
                Ok(())
            }
            fn get_processes_by_name(&self, _name: &str) -> NucleusResult<Vec<ProcessInfo>> {
                Ok(Vec::new())
            }
            fn wait_for_process(&self, _pid: u32, _timeout_ms: u32) -> NucleusResult<bool> {
                Ok(true)
            }
            fn suspend_process(&self, _pid: u32) -> NucleusResult<()> {
                Ok(())
            }
            fn resume_process(&self, _pid: u32) -> NucleusResult<()> {
                Ok(())
            }
        }

        #[derive(Default)]
        pub struct MockDispMgr;
        impl DisplayManager for MockDispMgr {
            fn get_displays(&self) -> NucleusResult<Vec<Display>> {
                Ok(vec![Display {
                    id: 0,
                    name: "MockPrimary".to_string(),
                    bounds: Rect::new(0, 0, 1920, 1080),
                    is_primary: true,
                    scale_factor: 1.0,
                }])
            }
            fn get_primary_display(&self) -> NucleusResult<Display> {
                Ok(Display {
                    id: 0,
                    name: "MockPrimary".to_string(),
                    bounds: Rect::new(0, 0, 1920, 1080),
                    is_primary: true,
                    scale_factor: 1.0,
                })
            }
            fn get_dpi_for_display(&self, _display_id: u32) -> NucleusResult<f64> {
                Ok(96.0)
            }
        }

        #[derive(Default)]
        pub struct MockWinMgr {
            pub pid_to_hwnd: Mutex<HashMap<u32, u64>>,
            pub applied_bounds: Mutex<Vec<(u64, Rect)>>,
        }
        impl WindowManager for MockWinMgr {
            fn find_window_by_title(
                &self,
                _title: &str,
                _partial: bool,
            ) -> NucleusResult<Option<WindowHandle>> {
                Ok(None)
            }
            fn find_windows_by_process(&self, pid: u32) -> NucleusResult<Vec<WindowHandle>> {
                Ok(self
                    .pid_to_hwnd
                    .lock()
                    .unwrap()
                    .get(&pid)
                    .map(|&h| vec![WindowHandle(h)])
                    .unwrap_or_default())
            }
            fn set_window_position(&self, handle: WindowHandle, rect: &Rect) -> NucleusResult<()> {
                self.applied_bounds
                    .lock()
                    .unwrap()
                    .push((handle.0, rect.clone()));
                Ok(())
            }
            fn set_window_style(
                &self,
                _handle: WindowHandle,
                _style: &WindowStyle,
            ) -> NucleusResult<()> {
                Ok(())
            }
            fn set_topmost(&self, _handle: WindowHandle, _topmost: bool) -> NucleusResult<()> {
                Ok(())
            }
            fn remove_borders(&self, _handle: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
            fn get_window_rect(&self, _handle: WindowHandle) -> NucleusResult<Rect> {
                Ok(Rect::new(0, 0, 1280, 720))
            }
            fn set_foreground(&self, _handle: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
            fn get_window_title(&self, _handle: WindowHandle) -> NucleusResult<String> {
                Ok(String::new())
            }
            fn set_window_title(&self, _handle: WindowHandle, _title: &str) -> NucleusResult<()> {
                Ok(())
            }
            fn close_window(&self, _handle: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
        }

        #[derive(Default)]
        pub struct MockInputMgr {
            pub raw_input_calls: Mutex<Vec<(u32, bool)>>,
        }
        impl InputHookManager for MockInputMgr {
            fn inject_hook_dll(&self, _pid: u32, _dll_path: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn send_fake_focus(&self, _handle: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
            fn block_raw_input(&self, pid: u32, block: bool) -> NucleusResult<()> {
                self.raw_input_calls.lock().unwrap().push((pid, block));
                Ok(())
            }
        }

        #[derive(Default)]
        pub struct MockMutexMgr;
        impl MutexManager for MockMutexMgr {
            fn kill_mutex(
                &self,
                _pid: u32,
                _mutex_name: &str,
                _mutex_type: &str,
            ) -> NucleusResult<bool> {
                Ok(false)
            }
            fn rename_mutex(
                &self,
                _pid: u32,
                _old_name: &str,
                _new_name: &str,
            ) -> NucleusResult<bool> {
                Ok(false)
            }
            fn find_mutex(
                &self,
                _pid: u32,
                _name: &str,
                _partial: bool,
            ) -> NucleusResult<Vec<String>> {
                Ok(Vec::new())
            }
        }

        #[derive(Default)]
        pub struct MockFsMgr;
        impl FilesystemManager for MockFsMgr {
            fn create_symlink(&self, _src: &Path, _dst: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn create_directory_symlink(&self, _src: &Path, _dst: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn create_hardlink(&self, _src: &Path, _dst: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn create_junction(&self, _src: &Path, _dst: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn is_symlink(&self, _path: &Path) -> bool {
                false
            }
        }

        #[derive(Default)]
        pub struct MockNetMgr;
        impl NetworkManager for MockNetMgr {
            fn get_interfaces(&self) -> NucleusResult<Vec<NetworkInterface>> {
                Ok(Vec::new())
            }
            fn create_virtual_adapter(&self) -> NucleusResult<NetworkInterface> {
                Err(NucleusError::Other("unused".into()))
            }
            fn set_adapter_ip(
                &self,
                _adapter: &str,
                _ip: &str,
                _subnet: &str,
            ) -> NucleusResult<()> {
                Ok(())
            }
        }

        #[derive(Default)]
        pub struct MockTaskbarMgr;
        impl TaskbarManager for MockTaskbarMgr {
            fn hide(&self) -> NucleusResult<()> {
                Ok(())
            }
            fn show(&self) -> NucleusResult<()> {
                Ok(())
            }
        }

        #[derive(Default)]
        pub struct MockElevMgr;
        impl ElevationManager for MockElevMgr {
            fn is_elevated(&self) -> bool {
                false
            }
            fn run_elevated(
                &self,
                _executable: &Path,
                _arguments: &[&str],
                _working_dir: Option<&Path>,
                _wait: bool,
            ) -> NucleusResult<Option<u32>> {
                Ok(Some(0))
            }
            fn ensure_symlink_support(&self) -> NucleusResult<bool> {
                Ok(true)
            }
        }

        #[derive(Default)]
        pub struct MockOsdMgr;
        impl OsdManager for MockOsdMgr {
            fn show(&self, _duration_ms: u32, _text: &str) {}
            fn hide(&self) {}
            fn current_message(&self) -> Option<String> {
                None
            }
            fn set_color(&self, _color: &str) {}
        }

        #[derive(Default)]
        pub struct MockPlatform {
            pub proc_mgr: MockProcMgr,
            pub disp_mgr: MockDispMgr,
            pub win_mgr: MockWinMgr,
            pub input_mgr: MockInputMgr,
            pub mutex_mgr: MockMutexMgr,
            pub fs_mgr: MockFsMgr,
            pub net_mgr: MockNetMgr,
            pub taskbar_mgr: MockTaskbarMgr,
            pub elev_mgr: MockElevMgr,
            pub osd_mgr: MockOsdMgr,
            pub cred_store: nucleus_platform::credentials::InMemoryCredentialStore,
        }

        impl PlatformServices for MockPlatform {
            fn credential_store(&self) -> &dyn nucleus_platform::credentials::CredentialStore {
                &self.cred_store
            }
            fn window_manager(&self) -> &dyn WindowManager {
                &self.win_mgr
            }
            fn process_manager(&self) -> &dyn ProcessManager {
                &self.proc_mgr
            }
            fn display_manager(&self) -> &dyn DisplayManager {
                &self.disp_mgr
            }
            fn input_hook_manager(&self) -> &dyn InputHookManager {
                &self.input_mgr
            }
            fn filesystem_manager(&self) -> &dyn FilesystemManager {
                &self.fs_mgr
            }
            fn registry_manager(&self) -> Option<&dyn RegistryManager> {
                None
            }
            fn taskbar_manager(&self) -> &dyn TaskbarManager {
                &self.taskbar_mgr
            }
            fn mutex_manager(&self) -> &dyn MutexManager {
                &self.mutex_mgr
            }
            fn user_account_manager(&self) -> Option<&dyn UserAccountManager> {
                None
            }
            fn network_manager(&self) -> &dyn NetworkManager {
                &self.net_mgr
            }
            fn elevation_manager(&self) -> &dyn ElevationManager {
                &self.elev_mgr
            }
            fn osd_manager(&self) -> &dyn OsdManager {
                &self.osd_mgr
            }
        }
    }

    #[test]
    fn test_re_tile_active_instances_ordinal_non_contiguous() {
        let platform = Arc::new(mock_platform::MockPlatform::default());
        {
            let mut map = platform.win_mgr.pid_to_hwnd.lock().unwrap();
            map.insert(101, 1010);
            map.insert(103, 1030);
        }

        let mut config = HandlerConfig::default();
        config.guid = "retile-test".to_string();
        config.supports_positioning = true;

        let mut handler = GameHandler::new(
            config,
            PathBuf::from("."),
            PathBuf::from("game.exe"),
            PathBuf::from("."),
            3,
        )
        .with_platform(platform.clone());

        let mut ctx0 = HandlerContext::new(0, "Player1".to_string());
        ctx0.is_active = true;
        ctx0.process_id = Some(101);
        ctx0.window_handle = Some(1010);

        let mut ctx1 = HandlerContext::new(1, "Player2".to_string());
        ctx1.is_active = false;
        ctx1.process_id = None;
        ctx1.window_handle = None;
        ctx1.pos_x = 123;
        ctx1.pos_y = 456;
        ctx1.width = 777;
        ctx1.height = 888;

        let mut ctx2 = HandlerContext::new(2, "Player3".to_string());
        ctx2.is_active = true;
        ctx2.process_id = Some(103);
        ctx2.window_handle = Some(1030);

        handler.contexts = vec![ctx0, ctx1, ctx2];
        handler.sync_launched_pids();

        let pairs = handler
            .re_tile_active_instances(crate::window::SplitMode::Vertical)
            .expect("re_tile_active_instances failed");

        assert_eq!(
            pairs,
            vec![
                (
                    0,
                    WindowBounds {
                        x: 0,
                        y: 0,
                        width: 960,
                        height: 1080
                    }
                ),
                (
                    2,
                    WindowBounds {
                        x: 960,
                        y: 0,
                        width: 960,
                        height: 1080
                    }
                ),
            ]
        );
        assert_eq!(handler.contexts[0].pos_x, 0);
        assert_eq!(handler.contexts[0].width, 960);
        assert_eq!(handler.contexts[2].pos_x, 960);
        assert_eq!(handler.contexts[2].width, 960);
        // Inactive middle slot untouched
        assert!(!handler.contexts[1].is_active);
        assert_eq!(handler.contexts[1].pos_x, 123);
        assert_eq!(handler.contexts[1].pos_y, 456);
        assert_eq!(handler.contexts[1].width, 777);
        assert_eq!(handler.contexts[1].height, 888);

        let applied = platform.win_mgr.applied_bounds.lock().unwrap();
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0].0, 1010);
        assert_eq!(applied[1].0, 1030);
    }

    #[test]
    fn test_drop_out_player_middle_slot_preserves_survivors_and_saves() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_dropout_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

        let platform = Arc::new(mock_platform::MockPlatform::default());
        {
            let mut map = platform.win_mgr.pid_to_hwnd.lock().unwrap();
            map.insert(1001, 2001);
            map.insert(1002, 2002);
            map.insert(1003, 2003);
        }

        let mut config = HandlerConfig::default();
        config.guid = "dropout-test".to_string();
        config.max_players = 4;
        config.keep_symlink_on_exit = true;

        let mut handler = GameHandler::new(config, game_dir, exe_path, staging_dir, 3)
            .with_platform(platform.clone());
        handler.prepare_instances().unwrap();

        for (i, folder) in handler.instance_folders.iter().enumerate() {
            std::fs::write(folder.join("save.dat"), format!("save_p{}", i)).unwrap();
        }
        handler.contexts[0].process_id = Some(1001);
        handler.contexts[0].window_handle = Some(2001);
        handler.contexts[1].process_id = Some(1002);
        handler.contexts[1].window_handle = Some(2002);
        handler.contexts[2].process_id = Some(1003);
        handler.contexts[2].window_handle = Some(2003);
        handler.sync_launched_pids();

        let remaining = handler
            .drop_out_player(1, crate::window::SplitMode::Vertical)
            .expect("drop_out_player should succeed");
        assert_eq!(remaining, 2);

        let killed = platform.proc_mgr.killed_pids.lock().unwrap().clone();
        assert_eq!(killed, vec![1002], "Only slot 1 PID should be killed");

        assert_eq!(handler.active_slot_indices(), vec![0, 2]);
        assert_eq!(handler.active_pids(), vec![1001, 1003]);
        assert_eq!(handler.launched_pids, vec![1001, 1003]);
        assert!(!handler.contexts[1].is_active);
        assert!(handler.contexts[1].process_id.is_none());
        assert!(handler.contexts[1].window_handle.is_none());

        // Instance folders and save files preserved on disk
        assert!(handler.instance_folders[0].join("save.dat").exists());
        assert!(handler.instance_folders[1].join("save.dat").exists());
        assert!(handler.instance_folders[2].join("save.dat").exists());

        // Surviving slots tiled ordinally left/right
        assert_eq!(handler.contexts[0].pos_x, 0);
        assert_eq!(handler.contexts[0].width, 960);
        assert_eq!(handler.contexts[2].pos_x, 960);
        assert_eq!(handler.contexts[2].width, 960);

        // Drop out slot 2 -> 1 survivor left
        let rem2 = handler
            .drop_out_player(2, crate::window::SplitMode::Auto)
            .expect("dropping second player should succeed");
        assert_eq!(rem2, 1);

        // Cannot drop out the last remaining player
        let last_err = handler.drop_out_player(0, crate::window::SplitMode::Auto);
        assert!(last_err.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_drop_in_player_reclaims_vacant_slot_and_runs_js_once() {
        let temp_dir =
            std::env::temp_dir().join(format!("nucleus_dropin_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let game_dir = temp_dir.join("game");
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&staging_dir).unwrap();

        let exe_path = game_dir.join("game.exe");
        std::fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

        let script = r#"
            Game.GUID = "dropin-test";
            Game.GameName = "DropIn Test";
            Game.MaxPlayers = 3;
            Game.Play = function() {
                Context.StartArguments = "played_slot=" + Context.PlayerID + ";players=" + Context.NumberOfPlayers;
                Context.WriteTextFile(Context.RootFolder + "/play_marker.txt", ["slot=" + Context.PlayerID]);
            };
        "#
        .to_string();

        let mut config = HandlerConfig::default();
        config.guid = "dropin-test".to_string();
        config.game_name = "DropIn Test".to_string();
        config.max_players = 3;

        let mut handler =
            GameHandler::new(config, game_dir, exe_path, staging_dir, 3).with_script_source(script);
        handler.prepare_instances().unwrap();

        // Clear initial play markers & args so we prove drop_in executes Game.Play ONLY for reclaimed slot 1
        for i in 0..3 {
            let marker = handler.instance_folders[i].join("play_marker.txt");
            let _ = std::fs::remove_file(marker);
            handler.contexts[i].start_arguments.clear();
        }

        // Drop out middle slot 1 -> [0=active, 1=inactive, 2=active]
        handler
            .drop_out_player(1, crate::window::SplitMode::Vertical)
            .unwrap();
        assert_eq!(handler.active_slot_indices(), vec![0, 2]);

        // Drop in new player -> should reclaim vacant slot 1
        let reclaimed_slot = handler
            .drop_in_player(
                "Gamepad #5",
                crate::window::SplitMode::TopOneBottomTwo,
                "Top 1 / Bottom 2",
            )
            .expect("drop_in_player should reclaim slot 1");
        assert_eq!(reclaimed_slot, 1);
        assert_eq!(handler.active_slot_indices(), vec![0, 1, 2]);
        assert!(handler.contexts[1].is_active);
        assert_eq!(handler.contexts[1].player_index, 1);
        assert_eq!(handler.contexts[1].player_id, 2);
        assert_eq!(handler.contexts[1].gamepad_id, 6);
        assert_eq!(
            handler.contexts[1].start_arguments,
            "played_slot=1;players=3"
        );
        assert_eq!(handler.layout, "Top 1 / Bottom 2");

        // Verify Game.Play ran ONLY for slot 1
        assert!(!handler.instance_folders[0].join("play_marker.txt").exists());
        assert!(handler.instance_folders[1].join("play_marker.txt").exists());
        assert!(!handler.instance_folders[2].join("play_marker.txt").exists());
        assert_eq!(handler.contexts[0].start_arguments, "");
        assert_eq!(handler.contexts[2].start_arguments, "");

        // Verify re-tiling across TopOneBottomTwo
        assert_eq!(handler.contexts[0].pos_x, 0);
        assert_eq!(handler.contexts[0].pos_y, 0);
        assert_eq!(handler.contexts[0].width, 1920);
        assert_eq!(handler.contexts[0].height, 540);
        assert_eq!(handler.contexts[1].pos_x, 0);
        assert_eq!(handler.contexts[1].pos_y, 540);
        assert_eq!(handler.contexts[1].width, 960);
        assert_eq!(handler.contexts[1].height, 540);
        assert_eq!(handler.contexts[2].pos_x, 960);
        assert_eq!(handler.contexts[2].pos_y, 540);
        assert_eq!(handler.contexts[2].width, 960);
        assert_eq!(handler.contexts[2].height, 540);

        // At max_players (3), further drop-in must return error cleanly
        let overflow =
            handler.drop_in_player("Gamepad #6", crate::window::SplitMode::Grid4, "2x2 (Grid)");
        assert!(overflow.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
