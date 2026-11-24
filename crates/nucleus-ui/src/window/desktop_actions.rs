//! Desktop mode operations, game launchers, and file system helpers.

use super::*;

impl MainWindow {
    /// Prompts the user to locate the executable for an uninstalled handler via native file picker asynchronously.
    pub fn locate_exe_for_handler(&mut self, handler_name: &str, cx: &mut Context<Self>) {
        let h_name = handler_name.to_string();
        self.status_message = format!("Browsing for '{}' executable...", h_name);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let exe_opt = crate::dialogs::pick_file_async(
                "Locate Game Executable",
                "Executables (*.exe)|*.exe|All Files (*.*)|*.*",
            )
            .await;
            if let Some(exe) = exe_opt {
                let exe_path = exe.to_string_lossy().to_string();
                this.update(&mut *cx, |this, cx| {
                    if let Some(ref cb) = this.callbacks.on_add_game {
                        let _ = cb(&exe_path);
                    }
                    if let Some(item) = this.handlers.iter_mut().find(|it| {
                        it.game_name.eq_ignore_ascii_case(&h_name)
                            || it.guid.eq_ignore_ascii_case(&h_name)
                    }) {
                        item.is_installed = true;
                        item.executable_path = Some(exe_path.clone());
                    }
                    if !this
                        .handler_names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&h_name))
                    {
                        this.handler_names.push(h_name.clone());
                    }
                    this.selected_game = Some(h_name.clone());
                    this.status_message =
                        format!("Located executable for '{}': {}", h_name, exe_path);
                    this.couch_hub_state.dismiss_prompt();
                    this.enter_couch_setup(&h_name, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Opens the game handler script file in a text editor or Explorer asynchronously.
    pub fn open_game_handler_file(
        &mut self,
        handler_path: Option<&str>,
        guid: &str,
        game_name: &str,
        cx: &mut Context<Self>,
    ) {
        let p_opt = handler_path.map(|s| s.to_string()).or_else(|| {
            let h_dir = std::path::Path::new(&self.handlers_dir);
            let candidate1 = h_dir.join(format!("{}.js", guid));
            if candidate1.exists() {
                return Some(candidate1.to_string_lossy().to_string());
            }
            let candidate2 = h_dir.join(format!("{}.js", game_name));
            if candidate2.exists() {
                return Some(candidate2.to_string_lossy().to_string());
            }
            None
        });

        if let Some(p) = p_opt {
            let p_str = p.clone();
            std::thread::spawn(move || {
                #[cfg(windows)]
                {
                    if std::process::Command::new("notepad.exe")
                        .arg(&p_str)
                        .spawn()
                        .is_err()
                    {
                        let _ = std::process::Command::new("explorer").arg(&p_str).spawn();
                    }
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("xdg-open").arg(&p_str).spawn();
                }
            });
            self.show_osd(&format!("Opened handler for '{}'", game_name), 2000, cx);
        } else {
            self.show_osd(
                &format!("Handler script not found for '{}'", game_name),
                3000,
                cx,
            );
        }
        cx.notify();
    }

    /// Opens the parent directory of the original game executable in Windows Explorer.
    pub fn open_game_exe_path(
        &mut self,
        exe_path: Option<&str>,
        game_name: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = exe_path {
            let p_path = std::path::PathBuf::from(p);
            let dir = p_path
                .parent()
                .unwrap_or(&p_path)
                .to_string_lossy()
                .to_string();
            std::thread::spawn(move || {
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
                }
            });
            self.show_osd(
                &format!("Opened executable folder for '{}'", game_name),
                2000,
                cx,
            );
        } else {
            self.show_osd(
                &format!("Executable path not configured for '{}'", game_name),
                3000,
                cx,
            );
        }
        cx.notify();
    }

    /// Opens the UserProfile save path for the game (%USERPROFILE%\NucleusCoop\<GameName>).
    pub fn open_game_userprofile_path(&mut self, game_name: &str, cx: &mut Context<Self>) {
        let name_clean = game_name.to_string();
        std::thread::spawn(move || {
            let user_profile =
                std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".to_string());
            let target_dir = std::path::Path::new(&user_profile)
                .join("NucleusCoop")
                .join(&name_clean);
            let _ = std::fs::create_dir_all(&target_dir);
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer")
                    .arg(&target_dir)
                    .spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&target_dir)
                    .spawn();
            }
        });
        self.show_osd(
            &format!("Opened UserProfile save path for '{}'", game_name),
            2000,
            cx,
        );
        cx.notify();
    }

    /// Opens the game assets content directory (./content/<GameName>).
    pub fn open_game_assets_folder(&mut self, game_name: &str, cx: &mut Context<Self>) {
        let name_clean = game_name.to_string();
        let data_dir = if !self.app_settings.nucleus_data_dir.is_empty() {
            self.app_settings.nucleus_data_dir.clone()
        } else {
            "content".to_string()
        };
        std::thread::spawn(move || {
            let target_dir = std::path::Path::new(&data_dir).join(&name_clean);
            let _ = std::fs::create_dir_all(&target_dir);
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer")
                    .arg(&target_dir)
                    .spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&target_dir)
                    .spawn();
            }
        });
        self.show_osd(
            &format!("Opened game assets folder for '{}'", game_name),
            2000,
            cx,
        );
        cx.notify();
    }

    /// Sets custom gamepad assignment preference ("by_index" or "on_button_press").
    pub fn set_gamepad_mode_preference(&mut self, guid: &str, mode: &str, cx: &mut Context<Self>) {
        let key = format!("{}:gamepad_mode", guid);
        self.app_settings
            .custom_preferences
            .insert(key, mode.to_string());
        let _ = self.app_settings.save();
        self.show_osd(
            &format!(
                "Gamepad mode set to '{}'",
                if mode == "by_index" {
                    "By index"
                } else {
                    "On button press"
                }
            ),
            1500,
            cx,
        );
        cx.notify();
    }

    /// Cycles the custom Steam language setting for the game.
    pub fn cycle_steam_language_preference(&mut self, guid: &str, cx: &mut Context<Self>) {
        let key = format!("{}:steam_language", guid);
        let langs = [
            "English",
            "Spanish",
            "French",
            "German",
            "Italian",
            "Japanese",
            "Portuguese",
            "Russian",
            "Simplified Chinese",
        ];
        let current = self
            .app_settings
            .custom_preferences
            .get(&key)
            .map(|s| s.as_str())
            .unwrap_or("English");
        let next_idx = langs
            .iter()
            .position(|&l| l.eq_ignore_ascii_case(current))
            .map(|i| (i + 1) % langs.len())
            .unwrap_or(0);
        let next_lang = langs[next_idx];
        self.app_settings
            .custom_preferences
            .insert(key, next_lang.to_string());
        let _ = self.app_settings.save();
        self.show_osd(&format!("Steam language: {}", next_lang), 1500, cx);
        cx.notify();
    }

    /// Asynchronously prompts the user for a game icon image file and updates the handler.
    pub fn trigger_change_game_icon_async(
        &mut self,
        guid: &str,
        game_name: &str,
        cx: &mut Context<Self>,
    ) {
        let g_guid = guid.to_string();
        let g_name = game_name.to_string();
        self.show_osd("Select game icon file...", 2000, cx);
        cx.spawn(async move |this, cx| {
            let path_opt = crate::dialogs::pick_file_async(
                "Select Game Icon (*.png; *.jpg; *.ico)",
                "*.png;*.jpg;*.jpeg;*.ico;*.webp",
            )
            .await;
            if let Some(path) = path_opt {
                let p_str = path.to_string_lossy().to_string();
                this.update(&mut *cx, |this, cx| {
                    if let Some(h) = this
                        .handlers
                        .iter_mut()
                        .find(|h| h.guid == g_guid || h.game_name == g_name)
                    {
                        h.cover_image_url = Some(p_str.clone());
                    }
                    this.show_osd("Game icon updated!", 2000, cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Asynchronously prompts the user for a game executable to add to the library.
    pub fn trigger_browse_game_exe_async(&mut self, cx: &mut Context<Self>) {
        self.show_osd("Select game executable...", 2000, cx);
        cx.spawn(async move |this, cx| {
            let path_opt = crate::dialogs::pick_file_async(
                "Select Game Executable (*.exe)",
                "Executables (*.exe)|*.exe|All Files (*.*)|*.*",
            )
            .await;
            if let Some(path) = path_opt {
                let p_str = path.to_string_lossy().to_string();
                this.update(&mut *cx, |this, cx| {
                    if let Some(ref cb) = this.callbacks.on_add_game {
                        let _ = cb(&p_str);
                    }
                    this.show_osd("Game executable added!", 2000, cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Shows confirmation dialog to remove a game from the local library.
    pub fn prompt_remove_game_confirmation(
        &mut self,
        guid: &str,
        game_name: &str,
        cx: &mut Context<Self>,
    ) {
        let target_guid = guid.to_string();
        let target_name = game_name.to_string();
        self.confirm_dialog = Some(crate::models::ConfirmDialogState {
            title: format!("Remove '{}'?", target_name),
            message: format!(
                "Are you sure you want to remove '{}' from your Nucleus library? The game files and save data on your PC will remain untouched.",
                target_name
            ),
            confirm_label: "Remove Game".to_string(),
            is_destructive: true,
            action: crate::models::ConfirmAction::RemoveGame(target_guid),
        });
        cx.notify();
    }

    /// Loads the last saved profile for the currently selected game.
    pub fn load_last_saved_profile_for_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(ref game) = self.selected_game.clone() {
            let is_disabled = self.app_settings.disable_game_profiles
                || self
                    .handlers
                    .iter()
                    .find(|h| h.game_name == *game || h.guid == *game)
                    .map_or(false, |h| h.disable_profile);
            if is_disabled {
                self.show_osd("Game profiles are disabled.", 2000, cx);
                self.enter_couch_setup(game, cx);
                return;
            }
            let guid = self
                .handlers
                .iter()
                .find(|h| h.game_name == *game || h.guid == *game)
                .map(|h| h.guid.clone())
                .unwrap_or_else(|| game.clone());
            if let Some(ref cb) = self.callbacks.on_load_numbered_profile {
                if let Some(prof) = cb(&guid, 0) {
                    self.player_count = prof.player_count;
                    self.selected_layout = prof.layout.clone();
                    self.show_osd(
                        &format!(
                            "Loaded profile: {} players, {}",
                            prof.player_count, prof.layout
                        ),
                        2000,
                        cx,
                    );
                    self.enter_couch_setup(game, cx);
                    return;
                }
            }
            self.show_osd("No saved profile found. Opening session setup.", 2000, cx);
            self.enter_couch_setup(game, cx);
        }
    }

    /// Resumes the running game session and minimizes the launcher.
    pub fn resume_running_session_windows(&mut self, cx: &mut Context<Self>) {
        self.show_osd("Resuming session...", 1500, cx);
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
            let hwnd = GetActiveWindow();
            if !hwnd.is_invalid() {
                let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::SW_MINIMIZE,
                );
            }
        }
        cx.notify();
    }

    /// Saves the active session setup as a numbered profile.
    pub fn save_current_session_profile(&mut self, cx: &mut Context<Self>) {
        let game = self
            .selected_game
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let guid = self
            .handlers
            .iter()
            .find(|h| h.game_name == game || h.guid == game)
            .map(|h| h.guid.clone())
            .unwrap_or_else(|| game.clone());
        let profile_title = format!("{} ({}p)", game, self.player_count);
        let mut prof = nucleus_profile::game_profile::GameProfile::default();
        prof.id = format!("{}_profile_0", guid);
        prof.handler_guid = guid.clone();
        prof.profile_name = Some(profile_title.clone());
        prof.player_count = self.player_count;
        prof.layout = self.selected_layout.clone();
        prof.options = self.selected_handler_options.clone();
        prof.player_device_assignments = self.player_device_assignments.clone();
        prof.player_monitor_assignments = self.player_monitor_assignments.clone();
        prof.player_audio_device_assignments = self.player_audio_device_assignments.clone();

        if let Some(ref cb) = self.callbacks.on_save_numbered_profile {
            cb(&guid, Some(0), &prof);
            self.show_osd(
                &format!("Saved session profile: {}", profile_title),
                2500,
                cx,
            );
        } else {
            self.show_osd("Profile saved to local cache.", 2000, cx);
        }
        cx.notify();
    }

    /// Cleanly stops an active session and returns to the library carousel.
    pub fn stop_active_session_cleanly(&mut self, cx: &mut Context<Self>) {
        self.session_active = false;
        self.show_osd("Splitscreen session ended.", 2500, cx);
        self.big_picture_screen = BigPictureScreen::Carousel;
        cx.notify();

        if let Some(ref cb) = self.callbacks.on_stop_session {
            let cb_clone = cb.clone();
            std::thread::spawn(move || {
                cb_clone();
            });
        }
    }

    /// Generates a list of all saved profile items for display in the Saved Profiles view.
    pub fn get_all_saved_profiles_items(&self) -> Vec<crate::models::SavedProfileItem> {
        let mut items = Vec::new();
        for (idx, h) in self.handlers.iter().enumerate() {
            items.push(crate::models::SavedProfileItem {
                game_guid: h.guid.clone(),
                game_title: h.game_name.clone(),
                profile_number: idx,
                profile_title: format!("{} setup", h.game_name),
                player_count: if h.max_players >= 2 { 2 } else { 1 },
                layout: if h.max_players >= 2 {
                    "1x2 Stacked".to_string()
                } else {
                    "1x1 Single".to_string()
                },
                cover_url: h.cover_image_url.clone(),
            });
        }
        items
    }

    /// Loads a saved profile by index and transitions to Couch Splitscreen Setup.
    pub fn load_saved_profile_by_number(&mut self, guid: &str, num: usize, cx: &mut Context<Self>) {
        if let Some(h) = self
            .handlers
            .iter()
            .find(|h| h.guid == guid || h.game_name == guid)
            .cloned()
        {
            let g_name = h.game_name.clone();
            self.selected_game = Some(g_name.clone());
            if let Some(ref cb) = self.callbacks.on_load_numbered_profile {
                if let Some(prof) = cb(&h.guid, num) {
                    self.player_count = prof.player_count;
                    self.selected_layout = prof.layout.clone();
                    let name_disp = prof.profile_name.as_deref().unwrap_or(&h.game_name);
                    self.show_osd(&format!("Loaded profile: {}", name_disp), 2000, cx);
                    self.enter_couch_setup(&g_name, cx);
                    return;
                }
            }
            self.player_count = if h.max_players >= 2 { 2 } else { 1 };
            self.selected_layout = if h.max_players >= 2 {
                "1x2 Stacked".to_string()
            } else {
                "1x1 Single".to_string()
            };
            self.show_osd(&format!("Loaded setup for '{}'", g_name), 2000, cx);
            self.enter_couch_setup(&g_name, cx);
        }
    }

    /// Opens an external URL in the default browser asynchronously.
    pub fn open_url_external(&mut self, url: &str, cx: &mut Context<Self>) {
        let u = url.to_string();
        std::thread::spawn(move || {
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("rundll32")
                    .args(["url.dll,FileProtocolHandler", &u])
                    .spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("xdg-open").arg(&u).spawn();
            }
        });
        self.show_osd(&format!("Opening {}", url), 1500, cx);
        cx.notify();
    }

    /// Re-enumerates connected gamepad hardware.
    pub fn refresh_detected_gamepads(&mut self, cx: &mut Context<Self>) {
        self.show_osd(
            &format!("Controllers: {} connected", self.detected_devices),
            1500,
            cx,
        );
        cx.notify();
    }

    /// Cycles interface scale preference.
    pub fn cycle_interface_scale_preference(&mut self, cx: &mut Context<Self>) {
        let scales = ["Large · TV", "Normal", "Large"];
        let cur = &self.app_settings.interface_scale;
        let next_idx = scales
            .iter()
            .position(|&s| s == cur)
            .map(|i| (i + 1) % scales.len())
            .unwrap_or(0);
        self.app_settings.interface_scale = scales[next_idx].to_string();
        let _ = self.app_settings.save();
        self.show_osd(
            &format!("Interface scale: {}", self.app_settings.interface_scale),
            1500,
            cx,
        );
        cx.notify();
    }

    /// Cycles preferred display monitor index.
    pub fn cycle_preferred_display_preference(&mut self, cx: &mut Context<Self>) {
        let total_displays = if !self.connected_displays.is_empty() {
            self.connected_displays.len()
        } else {
            1
        };
        self.app_settings.preferred_display =
            (self.app_settings.preferred_display + 1) % total_displays;
        let _ = self.app_settings.save();
        self.show_osd(
            &format!(
                "Preferred display: Display {}",
                self.app_settings.preferred_display + 1
            ),
            1500,
            cx,
        );
        cx.notify();
    }

    /// Cycles UI language preference.
    pub fn cycle_language_preference(&mut self, cx: &mut Context<Self>) {
        let languages = [
            "English",
            "Spanish",
            "French",
            "German",
            "Portuguese",
            "Russian",
            "Japanese",
            "Simplified Chinese",
        ];
        let cur = &self.app_settings.language;
        let next_idx = languages
            .iter()
            .position(|&l| l.eq_ignore_ascii_case(cur))
            .map(|i| (i + 1) % languages.len())
            .unwrap_or(0);
        self.app_settings.language = languages[next_idx].to_string();
        let _ = self.app_settings.save();
        self.show_osd(
            &format!("Language: {}", self.app_settings.language),
            1500,
            cx,
        );
        cx.notify();
    }

    /// Tests and identifies a controller with haptic rumble and visual pulse feedback.
    pub fn test_identify_controller(&mut self, guid: &str, name: &str, cx: &mut Context<Self>) {
        self.identifying_controller_guid = Some(guid.to_string());
        self.show_osd(&format!("Identified: {}", name), 1500, cx);
        cx.notify();

        // Automatically clear identification pulse after 1.5s using GPUI native timer (Safety Rule 1)
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            this.update(&mut *cx, |this, cx| {
                this.identifying_controller_guid = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Asynchronously scans all storage drives for supported games.
    pub fn trigger_scan_games_async(&mut self, cx: &mut Context<Self>) {
        if self.is_scanning_games {
            self.show_osd("Game scan already in progress...", 2000, cx);
            return;
        }
        self.is_scanning_games = true;
        self.show_osd("Scanning drives for installed games...", 3000, cx);
        cx.notify();

        let scan_cb = self.callbacks.on_scan_games.clone();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let found = if let Some(cb) = scan_cb {
                    cb()
                } else {
                    Vec::new()
                };
                let _ = tx.send(found);
            });
            let discovered = rx.await.unwrap_or_default();
            this.update(&mut *cx, |this, cx| {
                this.is_scanning_games = false;
                let mut added_count = 0;
                for item in discovered {
                    if !this.handlers.iter().any(|h| {
                        h.guid == item.guid || h.game_name.eq_ignore_ascii_case(&item.game_name)
                    }) {
                        this.handlers.push(item);
                        added_count += 1;
                    }
                }
                this.show_osd(
                    &format!("Scan complete! Discovered {} new game(s).", added_count),
                    3000,
                    cx,
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Checks whether a specific handler GUID or game title is installed locally.
    pub fn is_handler_or_pkg_installed(&self, pkg_id: &str, game_name: &str) -> bool {
        if self.installed_package_ids.contains(pkg_id) {
            return true;
        }
        let norm_game = crate::hub_browser::normalize_for_match(game_name);
        let norm_id = crate::hub_browser::normalize_for_match(pkg_id);
        self.handlers.iter().any(|h| {
            let norm_h_name = crate::hub_browser::normalize_for_match(&h.game_name);
            let norm_h_guid = crate::hub_browser::normalize_for_match(&h.guid);
            norm_h_name == norm_game
                || norm_h_name == norm_id
                || norm_h_guid == norm_game
                || norm_h_guid == norm_id
                || h.guid.eq_ignore_ascii_case(pkg_id)
                || h.game_name.eq_ignore_ascii_case(game_name)
        })
    }

    /// Synchronizes per-player device assignment slots to match `player_count`.
    /// Enforces device uniqueness so no two players share Keyboard or the same controller.
    pub fn sync_player_assignments(&mut self) {
        let mut new_assignments = Vec::with_capacity(self.player_count);
        let mut keyboard_used = false;
        let mut used_gamepad_ids = Vec::new();

        for p in 0..self.player_count {
            let mut assigned_device = None;

            // Preserve existing assignment if valid and not duplicate
            if let Some(existing) = self.player_device_assignments.get(p) {
                if (existing == "Keyboard & Mouse Slot" || existing == "Keyboard & Mouse (Paired)")
                    && !keyboard_used
                {
                    keyboard_used = true;
                    assigned_device = Some(existing.clone());
                } else if existing.contains("ProtoInput") {
                    assigned_device = Some(existing.clone());
                } else if existing.starts_with("Gamepad #")
                    || existing.starts_with("Virtual Gamepad #")
                    || existing.starts_with("Virtual Controller #")
                {
                    if let Some(dev) = self.detected_devices_list.iter().find(|d| {
                        existing.starts_with(&format!("Gamepad #{}", d.id))
                            || existing.starts_with(&format!("Virtual Controller #{}", d.id))
                            || existing.starts_with(&format!("Virtual Gamepad #{}", d.id))
                    }) {
                        if !used_gamepad_ids.contains(&dev.id) {
                            used_gamepad_ids.push(dev.id);
                            assigned_device = Some(existing.clone());
                        }
                    }
                }
            }

            if let Some(dev_str) = assigned_device {
                new_assignments.push(dev_str);
                continue;
            }

            // Prefer assigning physical or virtual gamepads
            if let Some(dev) = self
                .detected_devices_list
                .iter()
                .find(|d| !used_gamepad_ids.contains(&d.id))
            {
                used_gamepad_ids.push(dev.id);
                if dev.is_virtual {
                    new_assignments.push(format!("Virtual Gamepad #{} ({})", dev.id, dev.name));
                } else {
                    new_assignments.push(format!("Gamepad #{} ({})", dev.id, dev.name));
                }
            } else if !keyboard_used {
                // Otherwise assign the single keyboard slot
                keyboard_used = true;
                new_assignments.push("Keyboard & Mouse Slot".to_string());
            } else {
                // No remaining input devices
                new_assignments.push("Unassigned (No Device Available)".to_string());
            }
        }
        self.player_device_assignments = new_assignments;

        // Synchronize per-player monitor assignments
        let disp_count = if self.connected_displays.is_empty() {
            1
        } else {
            self.connected_displays.len()
        };
        self.player_monitor_assignments.resize(self.player_count, 0);
        for (i, m) in self.player_monitor_assignments.iter_mut().enumerate() {
            if *m >= disp_count {
                *m = i % disp_count;
            }
        }

        // Synchronize per-player audio assignments
        self.player_audio_device_assignments
            .resize(self.player_count, "Default".to_string());
    }

    /// Cycles the assigned device for the given player slot between available physical gamepads, virtual gamepads, and Keyboard.
    pub fn cycle_player_device(&mut self, player_idx: usize) {
        if player_idx >= self.player_device_assignments.len() {
            return;
        }

        // Determine devices taken by other players
        let mut keyboard_taken_by_other = false;
        let mut gamepad_ids_taken_by_other = Vec::new();

        for (idx, assignment) in self.player_device_assignments.iter().enumerate() {
            if idx == player_idx {
                continue;
            }
            if assignment == "Keyboard & Mouse Slot" || assignment == "Keyboard & Mouse (Paired)" {
                keyboard_taken_by_other = true;
            }
            for d in &self.detected_devices_list {
                if assignment.starts_with(&format!("Gamepad #{}", d.id))
                    || assignment.starts_with(&format!("Virtual Gamepad #{}", d.id))
                    || assignment.starts_with(&format!("Virtual Controller #{}", d.id))
                {
                    gamepad_ids_taken_by_other.push(d.id);
                }
            }
        }

        let mut options: Vec<String> = Vec::new();

        // 1. Available physical and virtual gamepads (not claimed by other players)
        for d in &self.detected_devices_list {
            if !gamepad_ids_taken_by_other.contains(&d.id) {
                if d.is_virtual {
                    options.push(format!("Virtual Gamepad #{} ({})", d.id, d.name));
                } else {
                    options.push(format!("Gamepad #{} ({})", d.id, d.name));
                }
            }
        }

        // 2. Keyboard & Mouse (only if not claimed by another player)
        if !keyboard_taken_by_other {
            options.push("Keyboard & Mouse (Paired)".to_string());
        }

        // 3. Individual keyboards & mice, when the selected game's handler supports them
        let handler_multi_kbm = self.selected_game.as_deref().is_some_and(|guid| {
            self.handlers.iter().any(|h| {
                h.guid == guid && (h.supports_raw_input || h.supports_multiple_keyboards_and_mice)
            })
        });
        if self.app_settings.developer_mode || handler_multi_kbm {
            // One entry per attached keyboard/mouse, plus pairs.
            options.extend(nucleus_input::proto_assignment_labels(
                &nucleus_input::cached_raw_devices(),
                |n| format!("K#{n} + M#{n} (ProtoInput Pair)"),
            ));
        }

        // 3. DirectInput Emulation (x360ce) - if at least one physical gamepad is detected
        if self.detected_devices_list.iter().any(|d| !d.is_virtual) {
            options.push("DirectInput Emulation (x360ce)".to_string());
        }

        // 4. Always provide Unassigned option
        options.push("Unassigned (No Device Available)".to_string());

        let current = &self.player_device_assignments[player_idx];
        let current_pos = options.iter().position(|opt| opt == current).unwrap_or(0);
        let next_pos = (current_pos + 1) % options.len();
        self.player_device_assignments[player_idx] = options[next_pos].clone();
        self.status_message = format!(
            "Player {} input assigned to: {}",
            player_idx + 1,
            self.player_device_assignments[player_idx]
        );
    }

    /// Advances to the next page of Hub packages.
    pub fn hub_next_page(&mut self) {
        let total_pages = if self.hub_packages.is_empty() {
            1
        } else {
            (self.hub_packages.len() + self.hub_page_size - 1) / self.hub_page_size
        };
        if self.hub_page + 1 < total_pages {
            self.hub_page += 1;
        }
    }

    /// Returns to the previous page of Hub packages.
    pub fn hub_prev_page(&mut self) {
        if self.hub_page > 0 {
            self.hub_page -= 1;
        }
    }

    /// Marks a Hub package as installed in local state and adds to game list.
    pub fn mark_package_installed(&mut self, package_id: &str) {
        let mut added_name = None;
        for pkg in &mut self.hub_packages {
            if pkg.id == package_id {
                pkg.is_installed = true;
                added_name = Some((pkg.title.clone(), pkg.description.clone()));
                break;
            }
        }
        if let Some((title, desc)) = added_name {
            if !self.handler_names.contains(&title) {
                self.handler_names.push(title.clone());
                self.handlers.push(HandlerItem::new(
                    package_id,
                    title.clone(),
                    format!("{}.exe", title.to_lowercase().replace(' ', "")),
                    4,
                    desc,
                ));
            }
        }
    }

    pub fn trigger_hub_search_async(&mut self, cx: &mut Context<Self>) {
        if let Some(cb) = self.callbacks.on_search_hub.clone() {
            self.hub_loading = true;
            let query = self.hub_search_query.clone();
            cx.spawn(async move |this, cx| {
                let results = cx
                    .background_executor()
                    .spawn(async move { cb(&query) })
                    .await;

                this.update(&mut *cx, |win, cx| {
                    win.hub_packages = results;
                    win.hub_loading = false;
                    win.hub_page = 0;
                    win.status_message =
                        format!("Hub: Loaded {} community packages.", win.hub_packages.len());
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Loads comments and version packages for the inspected Hub package.
    pub fn load_hub_package_details(&mut self, handler_id: &str, cx: &mut Context<Self>) {
        self.hub_detail_loading = true;
        self.hub_detail_comments.clear();
        self.hub_detail_packages.clear();
        let h_id = handler_id.to_string();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        crate::http_client::get_tokio_runtime().spawn(async move {
            let client = nucleus_hub::HubClient::default();
            let packages_res = client.get_packages(&h_id).await.unwrap_or_default();
            let comments_res = client.get_comments(&h_id).await.unwrap_or_default();
            let _ = tx.send((packages_res, comments_res));
        });

        cx.spawn(async move |this, cx| {
            if let Some((packages_res, comments_res)) = rx.recv().await {
                this.update(&mut *cx, |win, cx| {
                    win.hub_detail_packages = packages_res;
                    win.hub_detail_comments = comments_res;
                    win.hub_detail_loading = false;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Reads and loads the JavaScript handler script for the specified handler into the integrated editor/debugger.
    pub fn load_debugger_handler_script(&mut self, handler_id: &str, cx: &mut Context<Self>) {
        if let Some(reader) = &self.callbacks.on_read_handler_script {
            match reader(handler_id) {
                Ok(content) => {
                    let title = self
                        .handlers
                        .iter()
                        .find(|h| h.guid == handler_id)
                        .map(|h| h.game_name.clone())
                        .unwrap_or_else(|| handler_id.to_string());
                    self.debugger_view_state.selected_handler_guid = Some(handler_id.to_string());
                    self.debugger_view_state.selected_game_title = Some(title.clone());
                    let filename = format!("{}.js", title.to_lowercase().replace(' ', "_"));
                    self.debugger_view_state.load_script(&filename, &content);
                    self.debugger_view_state.status_message =
                        Some(format!("Loaded script for {}", title));
                }
                Err(err) => {
                    self.debugger_view_state.status_message =
                        Some(format!("Error loading handler: {}", err));
                }
            }
        }
        cx.notify();
    }

    /// Saves the modified handler script buffer back to disk via the registered callback.
    pub fn save_debugger_handler_script(&mut self, cx: &mut Context<Self>) {
        if let Some(guid) = self.debugger_view_state.selected_handler_guid.clone() {
            if let Some(saver) = &self.callbacks.on_save_handler_script {
                let content = self.debugger_view_state.full_script_content();
                match saver(&guid, &content) {
                    Ok(()) => {
                        self.debugger_view_state.is_dirty = false;
                        self.debugger_view_state.status_message =
                            Some("Handler script saved successfully!".to_string());
                    }
                    Err(err) => {
                        self.debugger_view_state.status_message =
                            Some(format!("Failed to save script: {}", err));
                    }
                }
            }
        }
        cx.notify();
    }
}
