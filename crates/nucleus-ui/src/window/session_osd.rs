//! Session management, In-Game OSD menus, and hotkey listeners.

use super::*;

impl MainWindow {
    /// Returns true if a splitscreen session is actively executing.
    pub fn is_session_active(&self) -> bool {
        if let Some(ref cb) = self.callbacks.on_check_session_active {
            cb()
        } else {
            self.session_active
        }
    }

    /// Sets visibility of the in-game OSD menu for a specific summoning player slot
    /// and toggles window TOPMOST status on the target monitor.
    pub fn set_osd_visible_for_player(&mut self, summoner_slot: Option<usize>, visible: bool) {
        if visible {
            if !self.is_session_active() {
                return;
            }
            self.in_game_osd.summoner_slot = summoner_slot;
            if let Some(slot) = summoner_slot {
                self.in_game_osd.reassign_slot_idx = slot;
            }
            self.in_game_osd.open();
        } else {
            self.in_game_osd.close();
        }
        if let Some(ref cb) = self.callbacks.on_toggle_osd_topmost {
            cb(visible, summoner_slot);
        }
    }

    /// Sets visibility of the in-game OSD menu and toggles window TOPMOST status.
    pub fn set_osd_visible(&mut self, visible: bool) {
        let slot = self.in_game_osd.summoner_slot;
        self.set_osd_visible_for_player(slot, visible);
    }

    /// Starts an asynchronous listener on the GPUI executor that toggles the in-game OSD
    /// whenever a hotkey or controller guide signal is received from the background engine thread.
    pub fn listen_for_osd_signals(
        &mut self,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<crate::callbacks::EngineToGui>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.recv().await {
                let _ = this.update(&mut *cx, |win, cx| match msg {
                    crate::callbacks::EngineToGui::ToggleInGameOsd { summoner_slot } => {
                        win.toggle_in_game_osd_for_player(summoner_slot, cx);
                    }
                    crate::callbacks::EngineToGui::LaunchPrompt(req) => {
                        win.active_launch_prompt = Some(req);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    /// Toggles the In-Game Couch OSD overlay menu for a specific summoning player slot.
    pub fn toggle_in_game_osd_for_player(
        &mut self,
        summoner_slot: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if !self.is_session_active() {
            return;
        }
        let now_visible = !self.in_game_osd.is_visible;
        self.set_osd_visible_for_player(summoner_slot, now_visible);
        cx.notify();
    }

    /// Toggles the In-Game Couch OSD overlay menu.
    pub fn toggle_in_game_osd(&mut self, cx: &mut Context<Self>) {
        self.toggle_in_game_osd_for_player(None, cx);
    }

    /// Freezes or resumes the running game session.
    pub fn toggle_freeze_session(&mut self, cx: &mut Context<Self>) {
        let is_now_frozen = if let Some(ref cb) = self.callbacks.on_toggle_freeze {
            cb().unwrap_or(!self.in_game_osd.is_frozen)
        } else {
            !self.in_game_osd.is_frozen
        };
        self.in_game_osd.is_frozen = is_now_frozen;
        self.status_message = if is_now_frozen {
            "Instances Frozen (NtSuspendProcess)".to_string()
        } else {
            "Instances Resumed (NtResumeProcess)".to_string()
        };
        cx.notify();
    }

    /// Reassigns controller device ID for a player slot.
    pub fn reassign_controller(
        &mut self,
        slot_idx: usize,
        new_device_id: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref cb) = self.callbacks.on_reassign_controller {
            cb(slot_idx, new_device_id);
        }
        if let Some(slot) = self.couch_setup_state.slots.get_mut(slot_idx) {
            slot.device_id = Some(new_device_id);
            if let Some(dev) = self
                .detected_devices_list
                .iter()
                .find(|d| d.id == new_device_id)
            {
                slot.device_name = dev.name.clone();
                slot.device_guid = dev.guid.clone();
            }
        }
        if slot_idx < self.player_device_assignments.len() {
            let name = self
                .detected_devices_list
                .iter()
                .find(|d| d.id == new_device_id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| format!("Gamepad #{}", new_device_id));
            self.player_device_assignments[slot_idx] =
                format!("Gamepad #{} ({})", new_device_id, name);
        }
        self.status_message = format!(
            "Player {} controller hot-swapped to Device #{}",
            slot_idx + 1,
            new_device_id
        );
        self.in_game_osd.close_reassign();
        cx.notify();
    }

    /// Re-enforces window layout boundaries for active instances.
    pub fn re_enforce_windows(&mut self, cx: &mut Context<Self>) {
        if let Some(ref cb) = self.callbacks.on_re_enforce_windows {
            let cb_clone = cb.clone();
            std::thread::spawn(move || {
                cb_clone();
            });
        }
        self.status_message = "Window bounds re-enforced".to_string();
        cx.notify();
    }

    /// Toggles raw input lock status.
    pub fn toggle_input_lock(&mut self, cx: &mut Context<Self>) {
        let is_locked = if let Some(ref cb) = self.callbacks.on_toggle_input_lock {
            cb()
        } else {
            !self.in_game_osd.input_locked
        };
        self.in_game_osd.input_locked = is_locked;
        self.status_message = if is_locked {
            "Raw Input Locked (VK_END)".to_string()
        } else {
            "Raw Input Unlocked (VK_END)".to_string()
        };
        cx.notify();
    }

    /// Stops the currently active splitscreen session and returns to carousel.
    pub fn stop_session(&mut self, cx: &mut Context<Self>) {
        self.session_active = false;
        self.set_osd_visible(false);
        self.big_picture_screen = BigPictureScreen::Carousel;
        self.status_message = "Session stopped. Returned to Big Picture.".to_string();
        cx.notify();

        if let Some(ref cb) = self.callbacks.on_stop_session {
            let cb_clone = cb.clone();
            std::thread::spawn(move || {
                cb_clone();
            });
        }
    }

    /// Handles confirmation of the selected OSD action.
    pub fn handle_osd_confirm(&mut self, cx: &mut Context<Self>) {
        match self.in_game_osd.selected_action() {
            crate::big_picture::OsdAction::ResumeGame => {
                self.set_osd_visible(false);
                cx.notify();
            }
            crate::big_picture::OsdAction::ToggleFreeze => {
                self.toggle_freeze_session(cx);
            }
            crate::big_picture::OsdAction::ReassignControllers => {
                let slot = self
                    .couch_setup_state
                    .focused_slot
                    .min(self.player_count.saturating_sub(1));
                self.in_game_osd.open_reassign(slot);
                cx.notify();
            }
            crate::big_picture::OsdAction::DropInPlayer => {
                if let Some(ref querier) = self.callbacks.on_query_active_session {
                    if let Some(snap) = querier() {
                        self.in_game_osd.sync_from_snapshot(&snap);
                    }
                }
                let active_count = if self.in_game_osd.active_players.is_empty() {
                    self.player_count.max(1)
                } else {
                    self.in_game_osd.active_players.len()
                };
                let max_p = self.in_game_osd.max_players.max(active_count).max(4);
                self.in_game_osd.open_drop_in(active_count, max_p);
                cx.notify();
            }
            crate::big_picture::OsdAction::DropOutPlayer => {
                if let Some(ref querier) = self.callbacks.on_query_active_session {
                    if let Some(snap) = querier() {
                        self.in_game_osd.sync_from_snapshot(&snap);
                    }
                }
                let summoner = self.in_game_osd.summoner_slot;
                self.in_game_osd.open_drop_out(summoner);
                cx.notify();
            }
            crate::big_picture::OsdAction::ReEnforceWindows => {
                self.re_enforce_windows(cx);
                self.set_osd_visible(false);
                cx.notify();
            }
            crate::big_picture::OsdAction::ToggleInputLock => {
                self.toggle_input_lock(cx);
            }
            crate::big_picture::OsdAction::StopSession => {
                self.stop_session(cx);
            }
        }
    }

    /// Refreshes the list of saved numbered profiles for the currently selected game.
    pub fn refresh_saved_profiles(&mut self) {
        if let Some(ref game_name) = self.selected_game {
            let guid = self
                .handlers
                .iter()
                .find(|h| &h.game_name == game_name)
                .map(|h| h.guid.clone())
                .unwrap_or_else(|| game_name.clone());
            if let Some(ref lister) = self.callbacks.on_list_profiles {
                self.saved_profiles = lister(&guid);
            }
        } else {
            self.saved_profiles.clear();
        }
    }

    /// Refreshes the list of installed extensions from the engine.
    pub fn refresh_extensions(&mut self) {
        if let Some(ref rescanner) = self.callbacks.on_rescan_extensions {
            self.installed_extensions = rescanner();
        }
    }

    /// Opens file dialog to install an extension package archive (*.nucplugin, *.nucaddon, *.zip).
    pub fn install_extension_dialog(&mut self, cx: &mut Context<Self>) {
        self.status_message = "Browsing for extension package archive...".to_string();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let pkg_opt = crate::dialogs::pick_file_async(
                "Select Extension Package",
                "Extension Packages (*.nucplugin;*.nucaddon;*.zip)|*.nucplugin;*.nucaddon;*.zip|All Files (*.*)|*.*",
            )
            .await;
            if let Some(pkg) = pkg_opt {
                let pkg_path = pkg.to_string_lossy().to_string();
                let _ = this.update(&mut *cx, |this, cx| {
                    if let Some(ref cb) = this.callbacks.on_install_extension {
                        match cb(&pkg_path) {
                            Ok(id) => {
                                this.status_message = format!("Successfully installed extension '{}'", id);
                            }
                            Err(e) => {
                                this.status_message = format!("Failed installing extension: {}", e);
                            }
                        }
                    }
                    this.refresh_extensions();
                    cx.notify();
                });
            }
        }).detach();
    }

    /// Attaches persistent application settings to the window.
    pub fn with_app_settings(mut self, settings: nucleus_profile::AppSettings) -> Self {
        self.handlers_dir = settings.handlers_directory.clone();
        self.theme_name = settings.theme.clone();
        self.auto_scan = settings.auto_scan_on_start;
        crate::theme::set_active_theme(&self.theme_name);
        self.app_settings = settings;
        self
    }

    /// Displays a prominent On-Screen Display (OSD) banner over the interface (parity with C# `Globals.MainOSD.Show(timing, text)`).
    pub fn show_osd(&mut self, text: &str, duration_ms: u32, cx: &mut Context<Self>) {
        let dur = std::time::Duration::from_millis(duration_ms as u64);
        self.osd_message = Some((text.to_string(), std::time::Instant::now(), dur));
        self.status_message = text.to_string();
        cx.notify();

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(dur).await;
            this.update(&mut *cx, |win, cx| {
                if let Some((_, start, dur)) = win.osd_message {
                    if start.elapsed() >= dur {
                        win.osd_message = None;
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Renders the floating On-Screen Display (OSD) overlay badge when active.
    pub(super) fn render_osd_overlay(&self) -> Option<AnyElement> {
        if let Some((ref text, start, dur)) = self.osd_message {
            if start.elapsed() < dur {
                let color_val = parse_osd_color(&self.app_settings.osd_color);
                return Some(crate::osd::render_osd_overlay(text, color_val));
            }
        }
        None
    }

    /// Cycles the screen partition layout to the next available option (parity with Ctrl+N shortcut).
    pub fn cycle_layout(&mut self, cx: &mut Context<Self>) {
        let layout_options = [
            "1x1 (Single / Fullscreen)",
            "2x1 (Side-by-Side)",
            "1x2 (Stacked)",
            "2x2 (Grid)",
            "Top 1 / Bottom 2",
            "Top 2 / Bottom 1",
            "Multi-Monitor Span",
        ];
        let current = self.selected_layout.as_str();
        let idx = layout_options
            .iter()
            .position(|&l| l.eq_ignore_ascii_case(current))
            .unwrap_or(0);
        let next_idx = (idx + 1) % layout_options.len();
        let chosen = layout_options[next_idx].to_string();
        self.selected_layout = chosen.clone();
        self.status_message = format!("Partition layout set to: {}", chosen);
        self.show_osd(&chosen, 1200, cx);
        cx.notify();
    }
}
