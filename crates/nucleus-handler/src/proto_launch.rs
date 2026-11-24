//! Connects the handler's `Game.ProtoInput` options to the ProtoInput runtime.
//!
//! ProtoInput is what gives each instance its own keyboard, mouse and controller. The
//! launcher either starts the game through ProtoInput (`InjectStartup`) or injects it into
//! the already running process (`InjectRuntime_*`), then configures the hooks for that
//! instance.

use nucleus_common::error::{NucleusError, NucleusResult};
use nucleus_hook::proto_input::{
    InjectMethod, PlayerProtoInputConfig, ProcessBitness, ProtoInputConfig, ProtoInputRuntime,
};
use nucleus_input::{RawDevice, RawDeviceKind, nth_device_handle, ordinal_in_assignment};
use nucleus_script::HandlerContext;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Runtime injection method requested by the handler, if any.
///
/// The stealth method wins over EasyHook, which wins over remote `LoadLibrary`, matching the
/// original launcher's precedence.
pub fn runtime_method(cfg: &ProtoInputConfig) -> Option<InjectMethod> {
    if cfg.inject_startup {
        None
    } else if cfg.inject_runtime_easyhook_stealth {
        Some(InjectMethod::EasyHookStealth)
    } else if cfg.inject_runtime_easyhook {
        Some(InjectMethod::EasyHook)
    } else if cfg.inject_runtime_remote_load {
        Some(InjectMethod::RemoteLoad)
    } else {
        None
    }
}

/// Raw keyboard and mouse handles (`-1` when the player has none) for an assignment label.
pub fn raw_handles_for_assignment(assignment: &str, devices: &[RawDevice]) -> (i32, i32) {
    // Pair entries use the short form "K#1 + M#1 (ProtoInput Pair)".
    let lookup = |label: &str, kind: RawDeviceKind| -> i32 {
        ordinal_in_assignment(assignment, label)
            .or_else(|| ordinal_in_assignment(assignment, &label[..1]))
            .and_then(|n| nth_device_handle(devices, kind, n))
            .map(|h| h as u32 as i32)
            .unwrap_or(-1)
    };
    if !assignment.contains("ProtoInput") {
        return (-1, -1);
    }
    (
        lookup("Keyboard", RawDeviceKind::Keyboard),
        lookup("Mouse", RawDeviceKind::Mouse),
    )
}

/// Controller indices (1-based, 0 = none) handed to ProtoInput for one instance.
///
/// With `MultipleProtoControllers` the handler may assign up to four pads itself from
/// `Game.Play` (`Context.SetProtoInputMultipleControllers`); otherwise every slot mirrors the
/// player's own pad. Without it, a raw keyboard/mouse player gets no controller at all.
pub fn controller_indices(
    cfg: &ProtoInputConfig,
    ctx: &HandlerContext,
    manual_setup: bool,
) -> [u32; 4] {
    let own = ctx.gamepad_id.max(0) as u32;
    if cfg.multiple_proto_controllers {
        let chosen = ctx.proto_controllers;
        if manual_setup || chosen.iter().any(|c| *c != 0) {
            return chosen.map(|c| c.max(0) as u32);
        }
        return [own; 4];
    }
    if ctx.is_raw_keyboard || ctx.is_raw_mouse {
        [0; 4]
    } else {
        [own, 0, 0, 0]
    }
}

/// Builds the per-player ProtoInput settings for an instance.
pub fn player_config(
    cfg: &ProtoInputConfig,
    ctx: &HandlerContext,
    assignment: &str,
    devices: &[RawDevice],
    manual_controller_setup: bool,
    instance_handle: u32,
) -> PlayerProtoInputConfig {
    let (keyboard, mouse) = raw_handles_for_assignment(assignment, devices);
    let c = controller_indices(cfg, ctx, manual_controller_setup);
    let mut player = PlayerProtoInputConfig::new(instance_handle)
        .with_bounds(ctx.pos_x, ctx.pos_y, ctx.width, ctx.height)
        .with_raw_handles(
            if ctx.is_raw_keyboard { keyboard } else { -1 },
            if ctx.is_raw_mouse { mouse } else { -1 },
        )
        .with_controllers(c[0], c[1], c[2], c[3]);
    player.gamepad_guid = ctx.gamepad_guid.clone();
    player.raw_hid = ctx.raw_hid.clone();
    player
}

/// Folders searched for the ProtoInput loader, most specific first.
fn candidate_folders(nucleus_root: &Path) -> Vec<PathBuf> {
    let mut out = vec![
        nucleus_root.to_path_buf(),
        nucleus_root.join("utils").join("ProtoInput"),
        nucleus_root.join("ProtoInput"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.to_path_buf());
            out.push(dir.join("utils").join("ProtoInput"));
        }
    }
    out
}

/// Locates the folder holding the ProtoInput DLLs for this build of the launcher.
pub fn find_proto_folder(nucleus_root: &Path) -> Option<PathBuf> {
    let loader = if cfg!(target_pointer_width = "64") {
        "ProtoInputLoader64.dll"
    } else {
        "ProtoInputLoader32.dll"
    };
    candidate_folders(nucleus_root)
        .into_iter()
        .find(|dir| dir.join(loader).is_file())
}

/// Instance handle placeholder used while recording a handler callback, before the real
/// ProtoInput instances exist. `PLACEHOLDER_HANDLE_BASE + slot` stands for a player slot.
pub const PLACEHOLDER_HANDLE_BASE: u32 = 0x7000_0000;

/// Rewrites placeholder handles to the live instance handles, dropping operations aimed at
/// players that have no ProtoInput instance (dropped out, or not launched through it).
pub fn resolve_calls(
    calls: &[nucleus_script::proto_script::ProtoCall],
    live_handle: impl Fn(usize) -> Option<u32>,
) -> Vec<nucleus_script::proto_script::ProtoCall> {
    use nucleus_script::proto_script::ProtoCall as C;
    let map = |h: u32| -> Option<u32> {
        if h >= PLACEHOLDER_HANDLE_BASE {
            live_handle((h - PLACEHOLDER_HANDLE_BASE) as usize)
        } else {
            None
        }
    };
    calls
        .iter()
        .filter_map(|call| {
            Some(match *call {
                C::InstallHook { handle, hook } => C::InstallHook {
                    handle: map(handle)?,
                    hook,
                },
                C::UninstallHook { handle, hook } => C::UninstallHook {
                    handle: map(handle)?,
                    hook,
                },
                C::EnableMessageFilter { handle, filter } => C::EnableMessageFilter {
                    handle: map(handle)?,
                    filter,
                },
                C::DisableMessageFilter { handle, filter } => C::DisableMessageFilter {
                    handle: map(handle)?,
                    filter,
                },
                C::EnableMessageBlock { handle, message } => C::EnableMessageBlock {
                    handle: map(handle)?,
                    message,
                },
                C::DisableMessageBlock { handle, message } => C::DisableMessageBlock {
                    handle: map(handle)?,
                    message,
                },
                C::SetDrawFakeCursor { handle, enable } => C::SetDrawFakeCursor {
                    handle: map(handle)?,
                    enable,
                },
                C::SetDrawFakeCursorFix { handle, enable } => C::SetDrawFakeCursorFix {
                    handle: map(handle)?,
                    enable,
                },
                C::SetRawInputBypass { handle, enable } => C::SetRawInputBypass {
                    handle: map(handle)?,
                    enable,
                },
                C::StartFocusMessageLoop {
                    handle,
                    interval_ms,
                    messages,
                } => C::StartFocusMessageLoop {
                    handle: map(handle)?,
                    interval_ms,
                    messages,
                },
                C::StopFocusMessageLoop { handle } => C::StopFocusMessageLoop {
                    handle: map(handle)?,
                },
                C::Sleep { ms } => C::Sleep { ms },
            })
        })
        .collect()
}

/// Carries out resolved operations on the live instances.
fn execute_calls(
    loader: &nucleus_hook::proto_input::ProtoInputLoader,
    calls: &[nucleus_script::proto_script::ProtoCall],
) {
    use nucleus_hook::proto_input::{ProtoHookIDs, ProtoMessageFilterIDs};
    use nucleus_script::proto_script::ProtoCall as C;
    for call in calls {
        match *call {
            C::InstallHook { handle, hook } => {
                if let Some(id) = ProtoHookIDs::from_id(hook) {
                    loader.install_hook(handle, id);
                }
            }
            C::UninstallHook { handle, hook } => {
                if let Some(id) = ProtoHookIDs::from_id(hook) {
                    loader.uninstall_hook(handle, id);
                }
            }
            C::EnableMessageFilter { handle, filter } => {
                if let Some(id) = ProtoMessageFilterIDs::from_id(filter) {
                    loader.enable_message_filter(handle, id);
                }
            }
            C::DisableMessageFilter { handle, filter } => {
                if let Some(id) = ProtoMessageFilterIDs::from_id(filter) {
                    loader.disable_message_filter(handle, id);
                }
            }
            C::EnableMessageBlock { handle, message } => {
                loader.enable_message_block(handle, message)
            }
            C::DisableMessageBlock { handle, message } => {
                loader.disable_message_block(handle, message)
            }
            C::SetDrawFakeCursor { handle, enable } => loader.set_draw_fake_cursor(handle, enable),
            C::SetDrawFakeCursorFix { handle, enable } => {
                loader.set_draw_fake_cursor_fix(handle, enable)
            }
            C::SetRawInputBypass { handle, enable } => loader.set_raw_input_bypass(handle, enable),
            C::StartFocusMessageLoop {
                handle,
                interval_ms,
                messages,
            } => loader.start_focus_message_loop(
                handle,
                interval_ms,
                messages[0],
                messages[1],
                messages[2],
                messages[3],
                messages[4],
            ),
            C::StopFocusMessageLoop { handle } => loader.stop_focus_message_loop(handle),
            C::Sleep { ms } => std::thread::sleep(std::time::Duration::from_millis(ms)),
        }
    }
}

/// ProtoInput state for one running session.
pub struct ProtoSession {
    runtime: ProtoInputRuntime,
    dll_folder: PathBuf,
    /// ProtoInput instance handle per player slot.
    handles: HashMap<usize, u32>,
}

impl ProtoSession {
    /// Opens a session, failing with an actionable message when the DLLs are not installed.
    pub fn open(nucleus_root: &Path) -> NucleusResult<Self> {
        let dll_folder = find_proto_folder(nucleus_root).ok_or_else(|| {
            NucleusError::Platform(format!(
                "ProtoInput is required by this handler but its DLLs were not found. Copy the \
                 ProtoInput release files (ProtoInputLoader64.dll, ProtoInputHooks32/64.dll, \
                 ProtoInputUtilDynamic64.dll, ...) into '{}'.",
                nucleus_root.display()
            ))
        })?;
        Ok(Self {
            runtime: ProtoInputRuntime::new(Some(dll_folder.clone())),
            dll_folder,
            handles: HashMap::new(),
        })
    }

    pub fn dll_folder(&self) -> &Path {
        &self.dll_folder
    }

    pub fn instance_handle(&self, player_index: usize) -> Option<u32> {
        self.handles.get(&player_index).copied()
    }

    /// Starts `exe` suspended with ProtoInput injected, configures the hooks, then lets the
    /// process run. Returns the new process ID.
    pub fn start_with_injection(
        &mut self,
        player_index: usize,
        exe: &Path,
        arguments: &str,
        environment: &HashMap<String, String>,
        cfg: &ProtoInputConfig,
        make_player: impl FnOnce(u32) -> PlayerProtoInputConfig,
    ) -> NucleusResult<u32> {
        let _env = ScopedEnvironment::apply(environment);
        let (handle, pid) = self
            .runtime
            .inject_startup(exe, arguments, 0, &self.dll_folder)?;
        let player = make_player(handle);
        self.runtime
            .setup_instance(handle, player_index + 1, cfg, &player)?;
        self.runtime.wake_up_process(handle)?;
        self.handles.insert(player_index, handle);
        Ok(pid)
    }

    /// Injects ProtoInput into the running process `pid` and configures the hooks.
    pub fn inject_running(
        &mut self,
        player_index: usize,
        pid: u32,
        method: InjectMethod,
        cfg: &ProtoInputConfig,
        make_player: impl FnOnce(u32) -> PlayerProtoInputConfig,
    ) -> NucleusResult<()> {
        let handle = self.runtime.inject_runtime(pid, method, &self.dll_folder)?;
        let player = make_player(handle);
        self.runtime
            .setup_instance(handle, player_index + 1, cfg, &player)?;
        self.handles.insert(player_index, handle);
        Ok(())
    }

    /// Tells ProtoInput which window belongs to the instance once it exists.
    pub fn set_main_window(&self, player_index: usize, hwnd: u64) {
        let Some(handle) = self.instance_handle(player_index) else {
            return;
        };
        if let Ok(loader) = self.host_loader() {
            loader.update_main_window_handle(handle, hwnd);
        }
    }

    /// Applies the input-lock state: locking hands the physical keyboards and mice to the
    /// instances (and keeps them away from the desktop); unlocking gives them back.
    pub fn set_input_lock(&self, locked: bool) {
        let bitness = if cfg!(target_pointer_width = "64") {
            ProcessBitness::X64
        } else {
            ProcessBitness::X86
        };
        match self.runtime.util_for_bitness(bitness) {
            Ok(util) => {
                util.lock_input(locked);
            }
            Err(e) => tracing::warn!("ProtoInput input lock unavailable: {e}"),
        }
        let _ = self.runtime.set_freeze_external_input_all(locked);
    }

    /// Replays operations recorded from a handler callback on the live instances.
    ///
    /// Handlers put pauses between operations, so the replay runs on its own thread and
    /// never stalls the session heartbeat.
    pub fn replay(&self, recorded: &[nucleus_script::proto_script::ProtoCall]) {
        let calls = resolve_calls(recorded, |slot| self.instance_handle(slot));
        if calls.is_empty() {
            return;
        }
        match self.host_loader() {
            Ok(loader) => {
                std::thread::spawn(move || execute_calls(&loader, &calls));
            }
            Err(e) => tracing::warn!("Cannot apply handler ProtoInput callback: {e}"),
        }
    }

    /// Forgets a player's instance (after drop-out or process exit).
    pub fn forget(&mut self, player_index: usize) {
        self.handles.remove(&player_index);
    }

    fn host_loader(
        &self,
    ) -> NucleusResult<std::sync::Arc<nucleus_hook::proto_input::ProtoInputLoader>> {
        let bitness = if cfg!(target_pointer_width = "64") {
            ProcessBitness::X64
        } else {
            ProcessBitness::X86
        };
        self.runtime.loader_for_bitness(bitness)
    }
}

impl Drop for ProtoSession {
    fn drop(&mut self) {
        self.runtime.clear_tracked_handles();
    }
}

/// Temporarily applies a player's environment to this process.
///
/// ProtoInput's startup injector creates the game process itself and lets it inherit the
/// launcher's environment, so the per-player variables are set for the duration of the call
/// and restored afterwards.
struct ScopedEnvironment {
    previous: Vec<(String, Option<std::ffi::OsString>)>,
}

impl ScopedEnvironment {
    fn apply(vars: &HashMap<String, String>) -> Self {
        let mut previous = Vec::with_capacity(vars.len());
        for (key, value) in vars {
            if key.is_empty() || key.contains('=') || key.contains('\0') || value.contains('\0') {
                continue;
            }
            previous.push((key.clone(), std::env::var_os(key)));
            // SAFETY: on Windows the process environment block is guarded by the OS; the
            // variables are restored in `drop` before the launch step continues.
            unsafe { std::env::set_var(key, value) };
        }
        Self { previous }
    }
}

impl Drop for ScopedEnvironment {
    fn drop(&mut self) {
        for (key, old) in self.previous.drain(..) {
            // SAFETY: see `ScopedEnvironment::apply`.
            unsafe {
                match old {
                    Some(v) => std::env::set_var(&key, v),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(handle: u64, name: &str, kind: RawDeviceKind) -> RawDevice {
        RawDevice {
            handle,
            name: name.to_string(),
            kind,
        }
    }

    fn devices() -> Vec<RawDevice> {
        vec![
            dev(0x1001, "kbd-a", RawDeviceKind::Keyboard),
            dev(0x1002, "kbd-b", RawDeviceKind::Keyboard),
            dev(0x2001, "mouse-a", RawDeviceKind::Mouse),
            dev(0x2002, "mouse-b", RawDeviceKind::Mouse),
        ]
    }

    #[test]
    fn recorded_calls_are_resolved_to_live_instances() {
        use nucleus_script::proto_script::ProtoCall as C;
        let recorded = vec![
            C::InstallHook {
                handle: PLACEHOLDER_HANDLE_BASE,
                hook: 3,
            },
            C::Sleep { ms: 5 },
            C::SetDrawFakeCursor {
                handle: PLACEHOLDER_HANDLE_BASE + 1,
                enable: true,
            },
            C::StopFocusMessageLoop {
                handle: PLACEHOLDER_HANDLE_BASE + 2,
            },
            C::InstallHook {
                handle: 42,
                hook: 3,
            },
        ];
        // Slot 1 has no ProtoInput instance (for example it dropped out).
        let live = |slot: usize| match slot {
            0 => Some(900),
            2 => Some(902),
            _ => None,
        };
        assert_eq!(
            resolve_calls(&recorded, live),
            vec![
                C::InstallHook {
                    handle: 900,
                    hook: 3
                },
                C::Sleep { ms: 5 },
                C::StopFocusMessageLoop { handle: 902 },
            ],
            "placeholders map to live handles; unknown players and raw handles are dropped"
        );
    }

    #[test]
    fn runtime_method_follows_original_precedence() {
        let mut cfg = ProtoInputConfig::default();
        assert_eq!(runtime_method(&cfg), None);
        cfg.inject_runtime_remote_load = true;
        assert_eq!(runtime_method(&cfg), Some(InjectMethod::RemoteLoad));
        cfg.inject_runtime_easyhook = true;
        assert_eq!(runtime_method(&cfg), Some(InjectMethod::EasyHook));
        cfg.inject_runtime_easyhook_stealth = true;
        assert_eq!(runtime_method(&cfg), Some(InjectMethod::EasyHookStealth));
        cfg.inject_startup = true;
        assert_eq!(
            runtime_method(&cfg),
            None,
            "startup injection replaces runtime injection"
        );
    }

    #[test]
    fn assignments_map_to_real_device_handles() {
        let d = devices();
        assert_eq!(
            raw_handles_for_assignment("Keyboard #2 + Mouse #1 (ProtoInput)", &d),
            (0x1002, 0x2001)
        );
        assert_eq!(
            raw_handles_for_assignment("Keyboard #1 (ProtoInput)", &d),
            (0x1001, -1)
        );
        assert_eq!(
            raw_handles_for_assignment("Keyboard #9 + Mouse #2 (ProtoInput)", &d),
            (-1, 0x2002)
        );
        assert_eq!(
            raw_handles_for_assignment("K#2 + M#2 (ProtoInput Pair)", &d),
            (0x1002, 0x2002)
        );
        assert_eq!(
            raw_handles_for_assignment("Mouse #2 (ProtoInput)", &d),
            (-1, 0x2002)
        );
        assert_eq!(raw_handles_for_assignment("Gamepad #0", &d), (-1, -1));
        assert_eq!(
            raw_handles_for_assignment("Keyboard & Mouse (Paired)", &d),
            (-1, -1)
        );
    }

    #[test]
    fn controller_indices_cover_every_mode() {
        let mut cfg = ProtoInputConfig::default();
        let mut pad = HandlerContext::new(1, "P2".into());
        pad.gamepad_id = 2;
        assert_eq!(controller_indices(&cfg, &pad, false), [2, 0, 0, 0]);

        let mut kb = HandlerContext::new(0, "P1".into());
        kb.is_raw_keyboard = true;
        kb.gamepad_id = -1;
        assert_eq!(controller_indices(&cfg, &kb, false), [0, 0, 0, 0]);

        cfg.multiple_proto_controllers = true;
        assert_eq!(controller_indices(&cfg, &pad, false), [2, 2, 2, 2]);
        pad.proto_controllers = [1, 3, 0, 0];
        assert_eq!(controller_indices(&cfg, &pad, false), [1, 3, 0, 0]);
        pad.proto_controllers = [0, 0, 0, 0];
        assert_eq!(
            controller_indices(&cfg, &pad, true),
            [0, 0, 0, 0],
            "manual setup trusts the handler even when it assigns nothing"
        );
    }

    #[test]
    fn player_config_carries_bounds_devices_and_controllers() {
        let cfg = ProtoInputConfig::default();
        let mut ctx = HandlerContext::new(0, "P1".into());
        ctx.pos_x = 960;
        ctx.pos_y = 0;
        ctx.width = 960;
        ctx.height = 1080;
        ctx.is_raw_keyboard = true;
        ctx.is_raw_mouse = true;
        ctx.gamepad_id = -1;
        let p = player_config(
            &cfg,
            &ctx,
            "Keyboard #1 + Mouse #2 (ProtoInput)",
            &devices(),
            false,
            7,
        );
        assert_eq!(p.instance_handle, 7);
        assert_eq!((p.keyboard_handle, p.mouse_handle), (0x1001, 0x2002));
        assert_eq!(
            (
                p.monitor_bounds_x,
                p.monitor_bounds_width,
                p.monitor_bounds_height
            ),
            (960, 960, 1080)
        );
        assert_eq!(p.controller_index, 0);
    }

    #[test]
    fn missing_dlls_produce_an_actionable_error() {
        let dir = std::env::temp_dir().join(format!("nucleus_no_proto_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Only meaningful when the test binary's own folder has no ProtoInput install.
        if find_proto_folder(&dir).is_none() {
            let err = match ProtoSession::open(&dir) {
                Ok(_) => panic!("expected an error without DLLs"),
                Err(e) => e.to_string(),
            };
            assert!(err.contains("ProtoInputLoader64.dll"), "{err}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scoped_environment_restores_previous_values() {
        let key = format!("NUCLEUS_PROTO_ENV_TEST_{}", std::process::id());
        let mut vars = HashMap::new();
        vars.insert(key.clone(), "player2".to_string());
        vars.insert("BAD=KEY".to_string(), "ignored".to_string());
        {
            let _guard = ScopedEnvironment::apply(&vars);
            assert_eq!(std::env::var(&key).as_deref(), Ok("player2"));
        }
        assert!(std::env::var_os(&key).is_none());
    }
}
