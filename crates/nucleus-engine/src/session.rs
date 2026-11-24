//! Active game session management and runtime execution state.

use nucleus_common::error::NucleusResult;
use nucleus_handler::{GameHandler, IGameHandler};
use nucleus_profile::{GameProfile, PlayerInfo};
use serde::{Deserialize, Serialize};

/// Represents an actively running or staged split-screen game session.
pub struct GameSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Handler GUID for the running game.
    pub game_guid: String,
    /// Game profile configuration used to launch this session.
    pub profile: GameProfile,
    /// Player slots and device assignments for this session.
    pub players: Vec<PlayerInfo>,
    /// Underlying game handler orchestrator.
    pub handler: Option<GameHandler>,
    /// Whether the game processes are currently active.
    pub is_running: bool,
    /// Whether the game processes are currently frozen/suspended.
    pub is_frozen: bool,
    /// Whether the In-Game Couch OSD overlay is currently visible.
    pub is_osd_visible: bool,
    /// Instant when session started.
    pub started_at: Option<std::time::Instant>,
    /// Timestamp of last playtime flush to profile/metaInfo.
    pub last_playtime_flush: Option<std::time::Instant>,
    /// Accumulated runtime seconds already flushed in this session.
    pub flushed_seconds: u64,
    /// Uncommitted delta playtime seconds waiting to be saved by GameManager.
    pub pending_playtime_seconds: u64,
}

/// Snapshot summary of session status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatus {
    /// Unique session identifier.
    pub session_id: String,
    /// Game handler GUID.
    pub game_guid: String,
    /// Total number of active player instances.
    pub player_count: usize,
    /// Whether the session is currently active.
    pub is_running: bool,
}

/// Snapshot of a single active player slot for the In-Game Couch OSD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSessionSlotSnapshot {
    pub slot_idx: usize,
    pub nickname: String,
    pub device_assignment: String,
    pub pid: Option<u32>,
    pub hwnd: Option<usize>,
}

/// Snapshot of the currently active session for the In-Game Couch OSD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSessionSnapshot {
    pub game_guid: String,
    pub game_name: String,
    pub layout: String,
    pub max_players: usize,
    pub is_frozen: bool,
    pub elapsed_seconds: u64,
    pub slots: Vec<ActiveSessionSlotSnapshot>,
}

impl GameSession {
    /// Creates a new `GameSession` with the specified parameters.
    pub fn new(
        session_id: impl Into<String>,
        game_guid: impl Into<String>,
        profile: GameProfile,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            game_guid: game_guid.into(),
            profile,
            players: Vec::new(),
            handler: None,
            is_running: false,
            is_frozen: false,
            is_osd_visible: false,
            started_at: None,
            last_playtime_flush: None,
            flushed_seconds: 0,
            pending_playtime_seconds: 0,
        }
    }

    /// Sets the player slot configuration for this session.
    pub fn set_players(&mut self, players: Vec<PlayerInfo>) {
        self.players = players;
    }

    /// Sets the underlying game handler.
    pub fn set_handler(&mut self, handler: GameHandler) {
        self.handler = Some(handler);
    }

    /// Returns true if PeerParty P2P multiplayer is active for this session.
    pub fn is_peerparty_active(&self) -> bool {
        self.handler
            .as_ref()
            .map(|h| h.config.use_peerparty)
            .unwrap_or(false)
    }

    /// Toggles or sets PeerParty active status for this session.
    pub fn set_peerparty_active(&mut self, active: bool) {
        if let Some(ref mut handler) = self.handler {
            handler.config.use_peerparty = active;
        }
    }

    /// Returns the PeerParty room code if configured for this session.
    pub fn peerparty_room_code(&self) -> Option<&str> {
        self.handler
            .as_ref()
            .and_then(|h| h.config.peerparty_room.as_deref())
    }

    /// Starts all configured game instances in this session.
    pub fn start(&mut self) -> NucleusResult<()> {
        if let Some(ref mut handler) = self.handler {
            handler.prepare_instances()?;
            let count = handler.player_count;
            for i in 0..count {
                if i > 0 && handler.config.pause_between_starts > 0 {
                    let pause_secs = if handler.config.pause_between_starts > 100 {
                        (handler.config.pause_between_starts / 1000) as u64
                    } else {
                        handler.config.pause_between_starts as u64
                    };
                    tracing::info!(
                        "Pausing for {}s before launching player {} (PauseBetweenStarts)",
                        pause_secs,
                        i
                    );
                    let deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(pause_secs);
                    let mut last_announced_sec = u64::MAX;
                    while std::time::Instant::now() < deadline {
                        let remaining = deadline
                            .saturating_duration_since(std::time::Instant::now())
                            .as_secs()
                            + 1;
                        if remaining != last_announced_sec {
                            last_announced_sec = remaining;
                            if let Some(ref platform) = handler.platform {
                                platform.osd_manager().show(
                                    1100,
                                    &format!("Launching Player {} in {}s...", i + 1, remaining),
                                );
                            }
                        }

                        // Continuously monitor and kill mutexes of all previous instances during the pause
                        if let Some(ref platform) = handler.platform {
                            if let Some(ref patterns) = handler.config.kill_mutex {
                                for prev in 0..i {
                                    if let Some(prev_pid) =
                                        handler.contexts.get(prev).and_then(|c| c.process_id)
                                    {
                                        let _ =
                                            nucleus_handler::MutexManager::kill_matching_mutexes(
                                                platform.mutex_manager(),
                                                prev_pid,
                                                patterns,
                                            );
                                    }
                                }
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }

                if let Some(ref platform) = handler.platform {
                    platform
                        .osd_manager()
                        .show(2000, &format!("Launching Player {}...", i + 1));
                }
                handler.launch_instance(i)?;
                let _ = handler.setup_window(i);
            }

            let _ = handler.finalize_instances();
            if let Some(ref platform) = handler.platform {
                platform
                    .osd_manager()
                    .show(3000, "All Players Ready! Press F10 for In-Game Menu");
            }
            self.is_running = true;
            self.started_at = Some(std::time::Instant::now());
        }
        Ok(())
    }

    /// Returns elapsed run time in seconds for this session.
    pub fn elapsed_seconds(&self) -> u64 {
        self.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }

    /// Flushes newly accumulated playtime into the profile and updates last played timestamp.
    /// Returns the newly added playtime delta in seconds.
    pub fn flush_playtime(&mut self) -> u64 {
        let current_elapsed = self.elapsed_seconds();
        if current_elapsed > self.flushed_seconds {
            let delta = current_elapsed - self.flushed_seconds;
            self.profile.total_play_time_seconds += delta;
            self.profile.last_played_timestamp = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            );
            self.flushed_seconds = current_elapsed;
            self.pending_playtime_seconds += delta;
            delta
        } else {
            0
        }
    }

    /// Stops and terminates all instances belonging to this session.
    pub fn stop(&mut self) -> NucleusResult<()> {
        let _ = self.flush_playtime();
        if let Some(ref mut handler) = self.handler {
            handler.stop()?;
        }
        self.is_running = false;
        self.is_frozen = false;
        Ok(())
    }

    /// Performs periodic maintenance or heartbeat checks on instance processes.
    pub fn tick(&mut self) -> NucleusResult<()> {
        if let Some(ref handler) = self.handler {
            let active_pids = handler.active_pids();
            if active_pids.is_empty() {
                self.is_running = false;
            } else {
                let any_running = if let Some(ref platform) = handler.platform {
                    let proc_mgr = platform.process_manager();
                    active_pids
                        .iter()
                        .any(|&pid| proc_mgr.is_process_running(pid))
                } else {
                    true
                };

                if !any_running {
                    self.is_running = false;
                    let _ = self.flush_playtime();
                    return Ok(());
                }

                // Re-enforce window positions for active instances only (skip while In-Game OSD is active to prevent z-order occlusion)
                if !handler.windows_minimized && !handler.cutscenes_active && !self.is_osd_visible {
                    for slot_idx in handler.active_slot_indices() {
                        let _ = handler.reposition_instance_window(slot_idx);
                    }
                }
            }
        }

        // Periodic playtime auto-flush every 60 seconds (parity with C# SaveUserProfile/SaveMetaInfo timer)
        if self.is_running {
            let now = std::time::Instant::now();
            let should_flush = match self.last_playtime_flush {
                Some(last) => now.duration_since(last).as_secs() >= 60,
                None => self.elapsed_seconds() >= 60,
            };
            if should_flush {
                let _ = self.flush_playtime();
                self.last_playtime_flush = Some(now);
            }
        }

        // Re-enforce active Cheat Engine parity frozen memory values across all instances
        nucleus_hook::memory_engine::MemoryEngine::apply_frozen_slots();

        // Hand keyboards/mice to (or back from) the instances when the lock key was pressed.
        if let Some(ref mut handler) = self.handler {
            handler.sync_input_lock();
        }

        Ok(())
    }

    /// Dynamically switches/cycles splitscreen layout partitions for active instances.
    pub fn switch_layout(&mut self) -> NucleusResult<Option<String>> {
        if let Some(ref mut handler) = self.handler {
            let res = handler.switch_layout()?;
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    /// Toggles minimizing or restoring all active game windows (Ctrl+T parity).
    pub fn toggle_minimize_windows(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut handler) = self.handler {
            let res = handler.toggle_minimize_windows()?;
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    /// Toggles cutscenes presentation mode (Ctrl+M parity).
    pub fn toggle_cutscenes_mode(&mut self) -> NucleusResult<Option<bool>> {
        if let Some(ref mut handler) = self.handler {
            let res = handler.toggle_cutscenes_mode()?;
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    /// Returns whether the session instances are currently suspended / frozen.
    pub fn is_frozen(&self) -> bool {
        self.is_frozen
    }

    /// Suspends (freezes) all active running instance processes in this session.
    pub fn freeze_all(&mut self) -> NucleusResult<()> {
        if let Some(ref handler) = self.handler {
            if let Some(ref platform) = handler.platform {
                let proc_mgr = platform.process_manager();
                for pid in handler.active_pids() {
                    let _ = proc_mgr.suspend_process(pid);
                }
            }
        }
        self.is_frozen = true;
        Ok(())
    }

    /// Resumes (unfreezes) all active suspended instance processes in this session.
    pub fn unfreeze_all(&mut self) -> NucleusResult<()> {
        if let Some(ref handler) = self.handler {
            if let Some(ref platform) = handler.platform {
                let proc_mgr = platform.process_manager();
                for pid in handler.active_pids() {
                    let _ = proc_mgr.resume_process(pid);
                }
            }
        }
        self.is_frozen = false;
        Ok(())
    }

    /// Hot-swaps the assigned controller device ID for an active player slot mid-session.
    pub fn reassign_controller(
        &mut self,
        slot_idx: usize,
        new_device_id: usize,
    ) -> NucleusResult<()> {
        let is_slot_active = self
            .handler
            .as_ref()
            .and_then(|h| h.contexts.get(slot_idx))
            .map(|c| c.is_active)
            .unwrap_or(true);
        if !is_slot_active {
            return Ok(());
        }
        if let Some(player) = self.players.get_mut(slot_idx) {
            player.device_id = new_device_id;
        }
        if let Some(ref mut handler) = self.handler {
            if let Some(ctx) = handler.contexts.get_mut(slot_idx) {
                if ctx.is_active {
                    ctx.gamepad_id = new_device_id as i32;
                }
            }
        }
        Ok(())
    }

    /// Re-enforces window positions and bounds across all active player instances.
    pub fn re_enforce_windows(&mut self) -> NucleusResult<()> {
        if let Some(ref handler) = self.handler {
            for slot_idx in handler.active_slot_indices() {
                let _ = handler.reposition_instance_window(slot_idx);
            }
        }
        Ok(())
    }

    /// Drops out an arbitrary active player slot and re-tiles survivors cleanly using `new_layout_name`.
    pub fn drop_out_player(
        &mut self,
        slot_idx: usize,
        new_layout_name: &str,
    ) -> NucleusResult<usize> {
        let mode = nucleus_handler::window::split_mode_from_name(new_layout_name);
        let remaining = if let Some(ref mut handler) = self.handler {
            let count = handler.drop_out_player(slot_idx, mode)?;
            handler.layout = new_layout_name.to_string();
            count
        } else {
            return Err(nucleus_common::error::NucleusError::Other(
                "No active GameHandler attached to session".to_string(),
            ));
        };
        self.profile.layout = new_layout_name.to_string();
        Ok(remaining)
    }

    /// Drops in a new player on the lowest vacant slot (or next slot), runs its local setup,
    /// and re-tiles all active instances under `new_layout_name`.
    pub fn drop_in_player(
        &mut self,
        device_assignment: &str,
        new_layout_name: &str,
    ) -> NucleusResult<usize> {
        let mode = nucleus_handler::window::split_mode_from_name(new_layout_name);
        let slot_idx = if let Some(ref mut handler) = self.handler {
            handler.drop_in_player(device_assignment, mode, new_layout_name)?
        } else {
            return Err(nucleus_common::error::NucleusError::Other(
                "No active GameHandler attached to session".to_string(),
            ));
        };
        let assigned_dev_id = self
            .handler
            .as_ref()
            .and_then(|h| h.contexts.get(slot_idx))
            .map(|c| c.gamepad_id.max(0) as usize)
            .unwrap_or(0);
        if let Some(existing_p) = self.players.get_mut(slot_idx) {
            existing_p.nickname = format!("Player {}", slot_idx + 1);
            existing_p.is_keyboard = device_assignment.contains("Keyboard");
            existing_p.device_id = assigned_dev_id;
        } else {
            while self.players.len() <= slot_idx {
                let idx = self.players.len();
                self.players.push(PlayerInfo {
                    player_index: idx,
                    nickname: format!("Player {}", idx + 1),
                    device_id: if idx == slot_idx { assigned_dev_id } else { 0 },
                    is_keyboard: idx == slot_idx && device_assignment.contains("Keyboard"),
                    ..Default::default()
                });
            }
        }
        self.profile.layout = new_layout_name.to_string();
        Ok(slot_idx)
    }

    /// Explicitly changes the active splitscreen layout and re-tiles all active instances.
    pub fn change_layout(&mut self, new_layout_name: &str) -> NucleusResult<String> {
        let mode = nucleus_handler::window::split_mode_from_name(new_layout_name);
        if let Some(ref mut handler) = self.handler {
            handler.split_mode = mode;
            handler.layout = new_layout_name.to_string();
            handler.is_layout_switched = true;
            handler.re_tile_active_instances(mode)?;
        }
        self.profile.layout = new_layout_name.to_string();
        Ok(new_layout_name.to_string())
    }

    /// Returns a live snapshot of all active slots and current session state for the Couch OSD.
    pub fn active_snapshot(&self, game_name: &str) -> ActiveSessionSnapshot {
        let (layout, max_players, slots) = if let Some(ref handler) = self.handler {
            let max_p = if handler.config.max_players > 0 {
                handler.config.max_players as usize
            } else {
                handler.contexts.len().max(1)
            };
            let active_slots = handler
                .contexts
                .iter()
                .enumerate()
                .filter(|(_, ctx)| ctx.is_active)
                .map(|(slot_idx, ctx)| {
                    let assignment = handler
                        .player_device_assignments
                        .get(slot_idx)
                        .cloned()
                        .unwrap_or_else(|| {
                            if ctx.is_keyboard_player {
                                "Keyboard & Mouse (Paired)".to_string()
                            } else if ctx.gamepad_id > 0 {
                                format!("Gamepad #{}", ctx.gamepad_id - 1)
                            } else {
                                format!("Gamepad #{}", slot_idx)
                            }
                        });
                    ActiveSessionSlotSnapshot {
                        slot_idx,
                        nickname: ctx.nickname.clone(),
                        device_assignment: assignment,
                        pid: ctx.process_id,
                        hwnd: ctx.window_handle,
                    }
                })
                .collect();
            (handler.layout.clone(), max_p, active_slots)
        } else {
            let active_slots = self
                .players
                .iter()
                .enumerate()
                .map(|(slot_idx, p)| ActiveSessionSlotSnapshot {
                    slot_idx,
                    nickname: p.nickname.clone(),
                    device_assignment: if p.is_keyboard {
                        "Keyboard & Mouse (Paired)".to_string()
                    } else {
                        format!("Gamepad #{}", p.device_id)
                    },
                    pid: None,
                    hwnd: None,
                })
                .collect();
            (
                self.profile.layout.clone(),
                self.players.len().max(1),
                active_slots,
            )
        };

        ActiveSessionSnapshot {
            game_guid: self.game_guid.clone(),
            game_name: game_name.to_string(),
            layout,
            max_players,
            is_frozen: self.is_frozen,
            elapsed_seconds: self.elapsed_seconds(),
            slots,
        }
    }

    /// Creates a mock 2-player active session for tests.
    pub fn mock_two_player_session() -> Self {
        let profile = GameProfile::new("mock_session", "mock_game", "game.exe");
        let mut session = Self::new("mock_session_1", "mock_game", profile);
        session.is_running = true;
        session.started_at = Some(std::time::Instant::now());
        session.players = vec![
            PlayerInfo {
                player_index: 0,
                nickname: "Player 1".to_string(),
                device_id: 0,
                ..Default::default()
            },
            PlayerInfo {
                player_index: 1,
                nickname: "Player 2".to_string(),
                device_id: 1,
                ..Default::default()
            },
        ];
        session
    }

    /// Returns a status summary for this session.
    pub fn status(&self) -> SessionStatus {
        SessionStatus {
            session_id: self.session_id.clone(),
            game_guid: self.game_guid.clone(),
            player_count: self.players.len(),
            is_running: self.is_running,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_session_status() {
        let profile = GameProfile::new("p1", "guid123", "game.exe");
        let mut session = GameSession::new("sess_1", "guid123", profile);

        assert!(!session.is_running);
        assert_eq!(session.status().session_id, "sess_1");
        assert_eq!(session.status().player_count, 0);

        session.set_players(vec![PlayerInfo::new(0, "Alice"), PlayerInfo::new(1, "Bob")]);
        assert_eq!(session.status().player_count, 2);

        session.stop().unwrap();
        assert!(!session.is_running);
    }

    #[test]
    fn test_game_session_playtime_flush() {
        let profile = GameProfile::new("p1", "guid123", "game.exe");
        let mut session = GameSession::new("sess_1", "guid123", profile);
        session.is_running = true;
        // Mock started_at 15 seconds in the past
        session.started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(15));

        let delta1 = session.flush_playtime();
        assert!(delta1 >= 15);
        assert_eq!(session.profile.total_play_time_seconds, delta1);
        assert!(session.profile.last_played_timestamp.is_some());
        assert_eq!(session.pending_playtime_seconds, delta1);
        assert_eq!(session.flushed_seconds, delta1);

        // Immediate subsequent flush should yield 0 delta
        let delta2 = session.flush_playtime();
        assert_eq!(delta2, 0);

        // Stop session flushes again cleanly
        session.stop().unwrap();
        assert!(!session.is_running);
    }

    #[test]
    fn test_session_tick_non_contiguous_active_slots() {
        use nucleus_common::types::{
            Display, NetworkInterface, ProcessConfig, ProcessHandle, ProcessInfo, ProcessPriority,
            Rect, WindowHandle, WindowStyle,
        };
        use nucleus_platform::{
            DisplayManager, ElevationManager, FilesystemManager, InputHookManager, MutexManager,
            NetworkManager, OsdManager, PlatformServices, ProcessManager, RegistryManager,
            TaskbarManager, UserAccountManager, WindowManager,
        };
        use nucleus_script::{HandlerConfig, HandlerContext};
        use std::collections::{HashMap, HashSet};
        use std::path::Path;
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct MockProcMgr(Mutex<HashSet<u32>>);
        impl ProcessManager for MockProcMgr {
            fn start_process(&self, _: &ProcessConfig) -> NucleusResult<ProcessHandle> {
                Ok(ProcessHandle::new(1, "m.exe"))
            }
            fn kill_process(&self, pid: u32) -> NucleusResult<()> {
                self.0.lock().unwrap().remove(&pid);
                Ok(())
            }
            fn get_process_info(&self, pid: u32) -> NucleusResult<ProcessInfo> {
                Ok(ProcessInfo {
                    pid,
                    name: "m.exe".into(),
                    exe_path: Default::default(),
                })
            }
            fn is_process_running(&self, pid: u32) -> bool {
                self.0.lock().unwrap().contains(&pid)
            }
            fn set_affinity(&self, _: u32, _: u64) -> NucleusResult<()> {
                Ok(())
            }
            fn set_priority(&self, _: u32, _: ProcessPriority) -> NucleusResult<()> {
                Ok(())
            }
            fn get_processes_by_name(&self, _: &str) -> NucleusResult<Vec<ProcessInfo>> {
                Ok(vec![])
            }
            fn wait_for_process(&self, _: u32, _: u32) -> NucleusResult<bool> {
                Ok(true)
            }
            fn suspend_process(&self, _: u32) -> NucleusResult<()> {
                Ok(())
            }
            fn resume_process(&self, _: u32) -> NucleusResult<()> {
                Ok(())
            }
        }

        #[derive(Default)]
        struct MockWinMgr {
            pid_to_hwnd: Mutex<HashMap<u32, u64>>,
            repositioned_hwnds: Mutex<Vec<u64>>,
        }
        impl WindowManager for MockWinMgr {
            fn find_window_by_title(
                &self,
                _: &str,
                _: bool,
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
            fn set_window_position(&self, handle: WindowHandle, _: &Rect) -> NucleusResult<()> {
                self.repositioned_hwnds.lock().unwrap().push(handle.0);
                Ok(())
            }
            fn set_window_style(&self, _: WindowHandle, _: &WindowStyle) -> NucleusResult<()> {
                Ok(())
            }
            fn set_topmost(&self, _: WindowHandle, _: bool) -> NucleusResult<()> {
                Ok(())
            }
            fn remove_borders(&self, _: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
            fn get_window_rect(&self, _: WindowHandle) -> NucleusResult<Rect> {
                Ok(Rect::new(0, 0, 1280, 720))
            }
            fn set_foreground(&self, _: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
            fn get_window_title(&self, _: WindowHandle) -> NucleusResult<String> {
                Ok(String::new())
            }
            fn set_window_title(&self, _: WindowHandle, _: &str) -> NucleusResult<()> {
                Ok(())
            }
            fn close_window(&self, _: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
        }

        #[derive(Default)]
        struct MockDispMgr;
        impl DisplayManager for MockDispMgr {
            fn get_displays(&self) -> NucleusResult<Vec<Display>> {
                Ok(vec![Display {
                    id: 0,
                    name: "P".into(),
                    bounds: Rect::new(0, 0, 1920, 1080),
                    is_primary: true,
                    scale_factor: 1.0,
                }])
            }
            fn get_primary_display(&self) -> NucleusResult<Display> {
                Ok(Display {
                    id: 0,
                    name: "P".into(),
                    bounds: Rect::new(0, 0, 1920, 1080),
                    is_primary: true,
                    scale_factor: 1.0,
                })
            }
            fn get_dpi_for_display(&self, _: u32) -> NucleusResult<f64> {
                Ok(96.0)
            }
        }

        #[derive(Default)]
        struct DummyHook;
        impl InputHookManager for DummyHook {
            fn inject_hook_dll(&self, _: u32, _: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn send_fake_focus(&self, _: WindowHandle) -> NucleusResult<()> {
                Ok(())
            }
            fn block_raw_input(&self, _: u32, _: bool) -> NucleusResult<()> {
                Ok(())
            }
        }
        #[derive(Default)]
        struct DummyFs;
        impl FilesystemManager for DummyFs {
            fn create_symlink(&self, _: &Path, _: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn create_directory_symlink(&self, _: &Path, _: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn create_hardlink(&self, _: &Path, _: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn create_junction(&self, _: &Path, _: &Path) -> NucleusResult<()> {
                Ok(())
            }
            fn is_symlink(&self, _: &Path) -> bool {
                false
            }
        }
        #[derive(Default)]
        struct DummyMutex;
        impl MutexManager for DummyMutex {
            fn kill_mutex(&self, _: u32, _: &str, _: &str) -> NucleusResult<bool> {
                Ok(false)
            }
            fn rename_mutex(&self, _: u32, _: &str, _: &str) -> NucleusResult<bool> {
                Ok(false)
            }
            fn find_mutex(&self, _: u32, _: &str, _: bool) -> NucleusResult<Vec<String>> {
                Ok(vec![])
            }
        }
        #[derive(Default)]
        struct DummyNet;
        impl NetworkManager for DummyNet {
            fn get_interfaces(&self) -> NucleusResult<Vec<NetworkInterface>> {
                Ok(vec![])
            }
            fn create_virtual_adapter(&self) -> NucleusResult<NetworkInterface> {
                unreachable!()
            }
            fn set_adapter_ip(&self, _: &str, _: &str, _: &str) -> NucleusResult<()> {
                Ok(())
            }
        }
        #[derive(Default)]
        struct DummyTaskbar;
        impl TaskbarManager for DummyTaskbar {
            fn hide(&self) -> NucleusResult<()> {
                Ok(())
            }
            fn show(&self) -> NucleusResult<()> {
                Ok(())
            }
        }
        #[derive(Default)]
        struct DummyElev;
        impl ElevationManager for DummyElev {
            fn is_elevated(&self) -> bool {
                false
            }
            fn run_elevated(
                &self,
                _: &Path,
                _: &[&str],
                _: Option<&Path>,
                _: bool,
            ) -> NucleusResult<Option<u32>> {
                Ok(None)
            }
            fn ensure_symlink_support(&self) -> NucleusResult<bool> {
                Ok(true)
            }
        }
        #[derive(Default)]
        struct DummyOsd;
        impl OsdManager for DummyOsd {
            fn show(&self, _: u32, _: &str) {}
            fn hide(&self) {}
            fn current_message(&self) -> Option<String> {
                None
            }
            fn set_color(&self, _: &str) {}
        }

        #[derive(Default)]
        struct MockPlat {
            proc_mgr: MockProcMgr,
            win_mgr: MockWinMgr,
            disp_mgr: MockDispMgr,
            hook: DummyHook,
            fs: DummyFs,
            mtx: DummyMutex,
            net: DummyNet,
            tb: DummyTaskbar,
            elev: DummyElev,
            osd: DummyOsd,
            cred: nucleus_platform::credentials::InMemoryCredentialStore,
        }
        impl PlatformServices for MockPlat {
            fn credential_store(&self) -> &dyn nucleus_platform::credentials::CredentialStore {
                &self.cred
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
                &self.hook
            }
            fn filesystem_manager(&self) -> &dyn FilesystemManager {
                &self.fs
            }
            fn registry_manager(&self) -> Option<&dyn RegistryManager> {
                None
            }
            fn taskbar_manager(&self) -> &dyn TaskbarManager {
                &self.tb
            }
            fn mutex_manager(&self) -> &dyn MutexManager {
                &self.mtx
            }
            fn user_account_manager(&self) -> Option<&dyn UserAccountManager> {
                None
            }
            fn network_manager(&self) -> &dyn NetworkManager {
                &self.net
            }
            fn elevation_manager(&self) -> &dyn ElevationManager {
                &self.elev
            }
            fn osd_manager(&self) -> &dyn OsdManager {
                &self.osd
            }
        }

        let plat = Arc::new(MockPlat::default());
        {
            let mut set = plat.proc_mgr.0.lock().unwrap();
            set.insert(101);
            set.insert(103);
            let mut hw = plat.win_mgr.pid_to_hwnd.lock().unwrap();
            hw.insert(101, 1010);
            hw.insert(102, 1020);
            hw.insert(103, 1030);
        }

        let mut cfg = HandlerConfig::default();
        cfg.guid = "tick-noncontig".to_string();
        cfg.game_name = "NonContig Game".to_string();
        cfg.max_players = 4;

        let mut handler = GameHandler::new(cfg, ".".into(), "g.exe".into(), ".".into(), 3)
            .with_platform(plat.clone())
            .with_layout("2x1 (Side-by-Side)")
            .with_player_device_assignments(vec![
                "Gamepad #0".to_string(),
                "Gamepad #1".to_string(),
                "Gamepad #2".to_string(),
            ]);

        let mut c0 = HandlerContext::new(0, "Player1".into());
        c0.is_active = true;
        c0.process_id = Some(101);
        c0.window_handle = Some(1010);

        let mut c1 = HandlerContext::new(1, "Player2".into());
        c1.is_active = false;
        c1.process_id = None;
        c1.window_handle = None;

        let mut c2 = HandlerContext::new(2, "Player3".into());
        c2.is_active = true;
        c2.process_id = Some(103);
        c2.window_handle = Some(1030);

        handler.contexts = vec![c0, c1, c2];
        handler.sync_launched_pids();

        let profile = GameProfile::new("p1", "tick-noncontig", "g.exe");
        let mut session = GameSession::new("s1", "tick-noncontig", profile);
        session.set_handler(handler);
        session.is_running = true;
        session.started_at = Some(std::time::Instant::now());

        session.tick().unwrap();
        assert!(
            session.is_running,
            "Session must stay running while active slots 0 & 2 live"
        );
        assert_eq!(
            plat.win_mgr.repositioned_hwnds.lock().unwrap().clone(),
            vec![1010, 1030],
            "Inactive slot 1 must not be repositioned"
        );

        let snap = session.active_snapshot("NonContig Game");
        assert_eq!(snap.slots.len(), 2);
        assert_eq!(snap.slots[0].slot_idx, 0);
        assert_eq!(snap.slots[1].slot_idx, 2);

        // When both surviving PIDs exit, session stops
        plat.proc_mgr.0.lock().unwrap().clear();
        session.tick().unwrap();
        assert!(!session.is_running);
    }
}
