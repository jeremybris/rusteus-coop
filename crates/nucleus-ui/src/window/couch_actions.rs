//! Couch & Big Picture spatial navigation and action dispatchers.

use super::*;

impl MainWindow {
    /// Toggles between standard desktop UI and Big Picture couch UI mode.
    pub fn toggle_big_picture(&mut self, cx: &mut Context<Self>) {
        self.is_big_picture = !self.is_big_picture;
        self.carousel_state.set_count(self.handlers.len());
        if self.is_big_picture {
            if let Some(ref sel) = self.selected_game {
                if let Some(pos) = self.handlers.iter().position(|h| {
                    h.game_name.eq_ignore_ascii_case(sel) || h.guid.eq_ignore_ascii_case(sel)
                }) {
                    self.carousel_state.select(pos);
                }
            }
        }
        cx.notify();
    }

    /// Selects a game from the Big Picture carousel by index and opens Game Details.
    pub fn select_carousel_game(&mut self, idx: usize, cx: &mut Context<Self>) {
        let filtered = self
            .carousel_state
            .filter_and_sort_handlers(&self.handlers, &self.carousel_state.search_query);
        if let Some(handler) = filtered.get(idx) {
            self.carousel_state.select(idx);
            let game_name = handler.game_name.clone();
            if !handler.is_installed {
                self.couch_hub_state.prompt_missing_exe(&game_name);
                cx.notify();
                return;
            }
            self.selected_game = Some(game_name);
            self.big_picture_screen = BigPictureScreen::GameDetails;
            cx.notify();
        } else if idx < self.handlers.len() {
            self.carousel_state.select(idx);
            let game_name = self.handlers[idx].game_name.clone();
            if !self.handlers[idx].is_installed {
                self.couch_hub_state.prompt_missing_exe(&game_name);
                cx.notify();
                return;
            }
            self.selected_game = Some(game_name);
            self.big_picture_screen = BigPictureScreen::GameDetails;
            cx.notify();
        }
    }

    /// Enters Couch Splitscreen Setup for the specified game with auto-assignment.
    pub fn enter_couch_setup(&mut self, game_name: &str, cx: &mut Context<Self>) {
        self.selected_game = Some(game_name.to_string());
        let handler_opt = self.handlers.iter().find(|h| {
            h.game_name.eq_ignore_ascii_case(game_name) || h.guid.eq_ignore_ascii_case(game_name)
        });
        let supports_keyboard = handler_opt.map(|h| h.supports_keyboard).unwrap_or(false);

        if let Some(h) = handler_opt {
            for opt in &h.options {
                if !self.selected_handler_options.contains_key(&opt.key) {
                    if let Some(first_val) = opt.values.first() {
                        self.selected_handler_options
                            .insert(opt.key.clone(), first_val.clone());
                    }
                }
            }
        }

        self.couch_setup_state.auto_assign_from_items_with_options(
            &self.detected_devices_list,
            &self.app_settings.ignored_device_guids,
            supports_keyboard,
        );
        self.couch_setup_state.current_step = crate::big_picture::setup::CouchSetupStep::Players;
        self.couch_setup_state.focused_slot = 0;
        self.big_picture_screen = BigPictureScreen::CouchSetup;
        cx.notify();
    }

    /// Enters Couch Community Hub browsing in Big Picture mode.
    pub fn enter_couch_hub(&mut self, cx: &mut Context<Self>) {
        self.big_picture_screen = BigPictureScreen::CouchHub;
        if self.hub_packages.is_empty() {
            self.trigger_hub_search_async(cx);
        }
        cx.notify();
    }

    /// Opens the gamepad-navigable handler custom options modal.
    pub fn open_couch_options_modal(
        &mut self,
        game_name: &str,
        options: Vec<CouchOptionItem>,
        cx: &mut Context<Self>,
    ) {
        self.couch_hub_state.custom_options_modal = Some(
            crate::big_picture::CouchOptionsModalState::new(game_name, options),
        );
        cx.notify();
    }

    /// Confirms current custom options and proceeds with couch session launch.
    pub fn confirm_couch_options_and_launch(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = self.couch_hub_state.custom_options_modal.take() {
            self.selected_handler_options = modal.to_hashmap();
        }
        self.launch_couch_session(cx);
    }

    /// Downloads and installs a package from Couch Hub, then immediately drops into Couch Setup.
    pub fn download_and_play_couch_package(&mut self, package_id: &str, cx: &mut Context<Self>) {
        let pkg = match self
            .hub_packages
            .iter()
            .find(|p| p.id == package_id)
            .cloned()
        {
            Some(p) => p,
            None => return,
        };

        let is_installed = pkg.is_installed || self.installed_package_ids.contains(&pkg.id);
        if is_installed {
            let maybe_handler = self
                .handlers
                .iter()
                .find(|h| h.guid == pkg.id || h.game_name.eq_ignore_ascii_case(&pkg.game_name))
                .cloned();

            let candidate = maybe_handler
                .as_ref()
                .and_then(|h| h.executable_path.as_ref().map(std::path::PathBuf::from));

            match resolve_game_path_or_couch_prompt(candidate) {
                PathResolution::Found(_) => {
                    let game_name = maybe_handler.map(|h| h.game_name).unwrap_or(pkg.game_name);
                    self.enter_couch_setup(&game_name, cx);
                }
                PathResolution::PromptDesktopMode => {
                    self.couch_hub_state.prompt_desktop_mode(&pkg.title);
                    cx.notify();
                }
            }
            return;
        }

        self.installing_handlers.insert(pkg.id.clone(), 0.0);
        self.status_message = format!("Downloading package '{}'...", pkg.title);
        cx.notify();

        if let Some(ref cb) = self.callbacks.on_install_handler {
            let cb = cb.clone();
            let pkg_id_clone = pkg.id.clone();
            let pkg_id_done = pkg.id.clone();
            let pkg_title_done = pkg.title.clone();
            let pkg_game_done = pkg.game_name.clone();

            let (tx, mut rx) =
                tokio::sync::mpsc::unbounded_channel::<crate::models::HubInstallProgressMsg>();
            let tx_prog = tx.clone();

            std::thread::spawn(move || {
                let item = cb(
                    &pkg_id_clone,
                    Box::new(move |progress: f32| {
                        let _ =
                            tx_prog.send(crate::models::HubInstallProgressMsg::Progress(progress));
                    }),
                );
                let _ = tx.send(crate::models::HubInstallProgressMsg::Done(item));
            });

            cx.spawn(async move |this, cx| {
                while let Some(msg) = rx.recv().await {
                    let pkg_id = pkg_id_done.clone();
                    let pkg_title = pkg_title_done.clone();
                    let pkg_game = pkg_game_done.clone();

                    let _ = this.update(&mut *cx, |win, cx| match msg {
                        crate::models::HubInstallProgressMsg::Progress(p) => {
                            win.installing_handlers.insert(pkg_id, p);
                            cx.notify();
                        }
                        crate::models::HubInstallProgressMsg::Done(installed_item) => {
                            win.installing_handlers.remove(&pkg_id);
                            win.installed_package_ids.insert(pkg_id.clone());
                            if let Some(p) =
                                win.hub_packages.iter_mut().find(|item| item.id == pkg_id)
                            {
                                p.is_installed = true;
                                p.has_update = false;
                                p.is_up_to_date = true;
                                p.installed_version = Some(p.version_number);
                            }

                            let target_handler = if let Some(h) = installed_item {
                                let name = h.game_name.clone();
                                if let Some(existing) =
                                    win.handlers.iter_mut().find(|e| e.guid == h.guid)
                                {
                                    *existing = h.clone();
                                } else {
                                    win.handlers.push(h.clone());
                                }
                                if !win.handler_names.contains(&name) {
                                    win.handler_names.push(name.clone());
                                }
                                Some(h)
                            } else {
                                win.handlers
                                    .iter()
                                    .find(|e| {
                                        e.guid == pkg_id
                                            || e.game_name.eq_ignore_ascii_case(&pkg_game)
                                    })
                                    .cloned()
                            };

                            let candidate = target_handler.as_ref().and_then(|h| {
                                h.executable_path.as_ref().map(std::path::PathBuf::from)
                            });

                            match resolve_game_path_or_couch_prompt(candidate) {
                                PathResolution::Found(_) => {
                                    let g_name =
                                        target_handler.map(|h| h.game_name).unwrap_or(pkg_game);
                                    win.status_message =
                                        format!("Installed '{}'. Ready for couch setup!", g_name);
                                    win.enter_couch_setup(&g_name, cx);
                                }
                                PathResolution::PromptDesktopMode => {
                                    win.status_message = format!(
                                        "Installed '{}', but executable was not found.",
                                        pkg_title
                                    );
                                    win.couch_hub_state.prompt_desktop_mode(&pkg_title);
                                    cx.notify();
                                }
                            }
                        }
                    });
                }
            })
            .detach();
        } else {
            // Standalone or testing without live installer callback
            self.installing_handlers.remove(&pkg.id);
            self.installed_package_ids.insert(pkg.id.clone());
            if let Some(p) = self.hub_packages.iter_mut().find(|item| item.id == pkg.id) {
                p.is_installed = true;
            }

            let maybe_handler = self
                .handlers
                .iter()
                .find(|e| e.guid == pkg.id || e.game_name.eq_ignore_ascii_case(&pkg.game_name))
                .cloned();

            let candidate = maybe_handler
                .as_ref()
                .and_then(|h| h.executable_path.as_ref().map(std::path::PathBuf::from));

            match resolve_game_path_or_couch_prompt(candidate) {
                PathResolution::Found(_) => {
                    let g_name = maybe_handler.map(|h| h.game_name).unwrap_or(pkg.game_name);
                    self.enter_couch_setup(&g_name, cx);
                }
                PathResolution::PromptDesktopMode => {
                    self.couch_hub_state.prompt_desktop_mode(&pkg.title);
                    cx.notify();
                }
            }
        }
    }

    /// Restores a previously ignored controller GUID, saving settings and re-adding
    /// the device to active couch setup slots if found in detected devices list.
    pub fn restore_ignored_controller(&mut self, guid: &str, cx: &mut Context<Self>) {
        self.app_settings.ignored_device_guids.retain(|g| g != guid);
        let _ = self.app_settings.save();

        if guid == "keyboard-mouse-0" {
            let kbm = crate::big_picture::setup::create_keyboard_mouse_device();
            self.couch_setup_state
                .restore_device(&kbm, &mut self.app_settings.ignored_device_guids);
        } else if let Some(dev_item) = self
            .detected_devices_list
            .iter()
            .find(|d| d.guid == guid)
            .cloned()
        {
            self.couch_setup_state
                .restore_device_from_item(&dev_item, &mut self.app_settings.ignored_device_guids);
        }

        cx.notify();
    }

    /// Returns the filtered and sorted list of community hub packages for Couch Big Picture.
    pub fn get_filtered_couch_hub_packages(&self) -> Vec<&HubPackageItem> {
        crate::big_picture::hub::get_filtered_packages(
            &self.hub_packages,
            self.couch_hub_state.selected_category,
            &self.couch_hub_state.search_query,
            self.couch_hub_state.filter_verified,
            self.couch_hub_state.filter_gamepads,
            self.couch_hub_state.filter_km,
        )
    }

    /// Dispatches a canonical couch UI action by name across Big Picture screens.
    pub fn dispatch_couch_action_by_name(&mut self, action_name: &str, cx: &mut Context<Self>) {
        match action_name {
            "ToggleBigPicture" => {
                self.toggle_big_picture(cx);
            }
            "ToggleOsd" => {
                self.toggle_in_game_osd(cx);
            }
            "NavigateUp" => {
                if self.in_game_osd.is_visible {
                    self.in_game_osd.move_up();
                } else if let Some(ref mut m) = self.couch_hub_state.custom_options_modal {
                    m.move_up();
                } else if self.big_picture_screen == BigPictureScreen::Carousel {
                    self.carousel_state.move_up();
                } else if self.big_picture_screen == BigPictureScreen::GameDetails {
                    self.details_focus_index = self.details_focus_index.saturating_sub(1);
                } else if self.big_picture_screen == BigPictureScreen::GameManagement {
                    self.management_focus_index = match self.management_focus_index {
                        0 | 1 => 8,
                        2 => 0,
                        3 => 1,
                        4 => 2,
                        5 => 3,
                        6 => 4,
                        7 => 5,
                        _ => 0,
                    };
                } else if self.big_picture_screen == BigPictureScreen::Sessions {
                    if self.is_session_active() {
                        self.sessions_focus_index = self.sessions_focus_index.saturating_sub(1);
                    } else {
                        self.sessions_focus_index = 0;
                    }
                } else if self.big_picture_screen == BigPictureScreen::Settings {
                    if self.settings_focus_zone == 0 {
                        self.big_picture_settings_tab =
                            self.big_picture_settings_tab.saturating_sub(1);
                    } else {
                        self.settings_item_index = self.settings_item_index.saturating_sub(1);
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchHub {
                    if self.hub_focus_zone == 3 {
                        self.hub_focus_zone = 1;
                    } else if self.hub_focus_zone == 2 {
                        if self.couch_hub_state.selected_index == 0 {
                            self.hub_focus_zone = 1;
                        } else {
                            self.couch_hub_state.selected_index -= 1;
                        }
                    } else if self.hub_focus_zone == 1 {
                        self.hub_focus_zone = 0;
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    if self.couch_setup_state.focused_slot > 0 {
                        self.couch_setup_state.focused_slot -= 1;
                    }
                    if self.couch_setup_state.current_step
                        == crate::big_picture::setup::CouchSetupStep::ScreenLayout
                        && self.couch_setup_state.focused_slot
                            < crate::big_picture::setup::AVAILABLE_LAYOUTS.len()
                    {
                        self.couch_setup_state.selected_layout =
                            crate::big_picture::setup::AVAILABLE_LAYOUTS
                                [self.couch_setup_state.focused_slot]
                                .to_string();
                        self.selected_layout = self.couch_setup_state.selected_layout.clone();
                    }
                }
                cx.notify();
            }
            "NavigateDown" => {
                if self.in_game_osd.is_visible {
                    self.in_game_osd.move_down();
                } else if let Some(ref mut m) = self.couch_hub_state.custom_options_modal {
                    m.move_down();
                } else if self.big_picture_screen == BigPictureScreen::Carousel {
                    self.carousel_state.move_down();
                } else if self.big_picture_screen == BigPictureScreen::GameDetails {
                    self.details_focus_index = (self.details_focus_index + 1).min(2);
                } else if self.big_picture_screen == BigPictureScreen::GameManagement {
                    self.management_focus_index = match self.management_focus_index {
                        8 => 1,
                        0 => 2,
                        1 => 3,
                        2 => 4,
                        3 => 5,
                        4 => 6,
                        5 => 7,
                        _ => 7,
                    };
                } else if self.big_picture_screen == BigPictureScreen::Sessions {
                    if self.is_session_active() {
                        self.sessions_focus_index = (self.sessions_focus_index + 1).min(4);
                    } else if self.sessions_focus_index == 0 {
                        self.sessions_focus_index = 1;
                    }
                } else if self.big_picture_screen == BigPictureScreen::Settings {
                    if self.settings_focus_zone == 0 {
                        self.big_picture_settings_tab = (self.big_picture_settings_tab + 1).min(1);
                    } else {
                        let max_items = if self.big_picture_settings_tab == 0 {
                            6
                        } else {
                            self.detected_devices_list.len() + 1
                        };
                        self.settings_item_index = (self.settings_item_index + 1).min(max_items);
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchHub {
                    if self.hub_focus_zone == 0 {
                        self.hub_focus_zone = 1;
                    } else if self.hub_focus_zone == 1 {
                        self.hub_focus_zone = 2;
                    } else if self.hub_focus_zone == 2 {
                        let count = self.get_filtered_couch_hub_packages().len();
                        if count > 0 && self.couch_hub_state.selected_index + 1 < count {
                            self.couch_hub_state.selected_index += 1;
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    let max_slots = match self.couch_setup_state.current_step {
                        crate::big_picture::setup::CouchSetupStep::Players => {
                            self.couch_setup_state.slots.len()
                        }
                        crate::big_picture::setup::CouchSetupStep::ScreenLayout => {
                            crate::big_picture::setup::AVAILABLE_LAYOUTS.len()
                        }
                        crate::big_picture::setup::CouchSetupStep::GameOptions => self
                            .selected_game
                            .as_ref()
                            .and_then(|g| {
                                self.handlers
                                    .iter()
                                    .find(|h| &h.game_name == g || &h.guid == g)
                            })
                            .map(|h| h.options.len())
                            .unwrap_or(0),
                        crate::big_picture::setup::CouchSetupStep::Launch => 0,
                    };
                    if self.couch_setup_state.focused_slot < max_slots {
                        self.couch_setup_state.focused_slot += 1;
                    }
                    if self.couch_setup_state.current_step
                        == crate::big_picture::setup::CouchSetupStep::ScreenLayout
                        && self.couch_setup_state.focused_slot
                            < crate::big_picture::setup::AVAILABLE_LAYOUTS.len()
                    {
                        self.couch_setup_state.selected_layout =
                            crate::big_picture::setup::AVAILABLE_LAYOUTS
                                [self.couch_setup_state.focused_slot]
                                .to_string();
                        self.selected_layout = self.couch_setup_state.selected_layout.clone();
                    }
                }
                cx.notify();
            }
            "NavigateLeft" => {
                if let Some(ref mut m) = self.couch_hub_state.custom_options_modal {
                    m.cycle_choice(false);
                } else if self.big_picture_screen == BigPictureScreen::Carousel {
                    match self.carousel_state.focus_zone {
                        crate::big_picture::carousel::CarouselFocusZone::HeroBanner(btn_idx) => {
                            if btn_idx > 0 {
                                self.carousel_state.focus_zone =
                                    crate::big_picture::carousel::CarouselFocusZone::HeroBanner(
                                        btn_idx - 1,
                                    );
                            }
                        }
                        crate::big_picture::carousel::CarouselFocusZone::HeaderTabs => {
                            self.big_picture_screen = self.big_picture_screen.prev();
                        }
                        crate::big_picture::carousel::CarouselFocusZone::Cards => {
                            self.carousel_state.move_left();
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::GameDetails {
                    self.big_picture_screen = BigPictureScreen::Carousel;
                } else if self.big_picture_screen == BigPictureScreen::GameManagement {
                    self.management_focus_index = match self.management_focus_index {
                        1 => 0,
                        3 => 2,
                        5 => 4,
                        7 => 6,
                        8 => 0,
                        _ => self.management_focus_index,
                    };
                } else if self.big_picture_screen == BigPictureScreen::Sessions {
                    if !self.is_session_active() {
                        let total_profiles = self.get_all_saved_profiles_items().len();
                        if self.sessions_focus_index > 1 {
                            self.sessions_focus_index -= 1;
                        } else if self.sessions_focus_index == 1 && total_profiles > 0 {
                            self.sessions_focus_index = total_profiles;
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::Settings {
                    self.settings_focus_zone = 0;
                } else if self.big_picture_screen == BigPictureScreen::CouchHub {
                    if self.hub_focus_zone == 0 {
                        self.hub_top_card_index = 0;
                    } else if self.hub_focus_zone == 3 {
                        self.hub_focus_zone = 2;
                    } else {
                        self.couch_hub_state.prev_category();
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    match self.couch_setup_state.current_step {
                        crate::big_picture::setup::CouchSetupStep::Players => {
                            if self.couch_setup_state.focused_slot
                                == self.couch_setup_state.slots.len()
                            {
                                self.couch_setup_state.focused_slot = 0;
                            } else {
                                self.couch_setup_state.navigate_slot_left();
                            }
                        }
                        crate::big_picture::setup::CouchSetupStep::ScreenLayout => {
                            self.couch_setup_state.focused_slot = 0;
                        }
                        crate::big_picture::setup::CouchSetupStep::GameOptions => {
                            let cur_slot = self.couch_setup_state.focused_slot;
                            if let Some(h) = self.selected_game.as_ref().and_then(|g| {
                                self.handlers
                                    .iter()
                                    .find(|h| &h.game_name == g || &h.guid == g)
                            }) {
                                if let Some(opt) = h.options.get(cur_slot) {
                                    let cur_val = self
                                        .selected_handler_options
                                        .get(&opt.key)
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            opt.values.first().cloned().unwrap_or_default()
                                        });
                                    if let Some(pos) = opt.values.iter().position(|v| v == &cur_val)
                                    {
                                        let prev_idx =
                                            (pos + opt.values.len() - 1) % opt.values.len();
                                        if let Some(nv) = opt.values.get(prev_idx) {
                                            self.selected_handler_options
                                                .insert(opt.key.clone(), nv.clone());
                                        }
                                    }
                                }
                            }
                        }
                        crate::big_picture::setup::CouchSetupStep::Launch => {}
                    }
                } else {
                    self.carousel_state.move_left();
                }
                cx.notify();
            }
            "NavigateRight" => {
                if let Some(ref mut m) = self.couch_hub_state.custom_options_modal {
                    m.cycle_choice(true);
                } else if self.big_picture_screen == BigPictureScreen::Carousel {
                    match self.carousel_state.focus_zone {
                        crate::big_picture::carousel::CarouselFocusZone::HeroBanner(btn_idx) => {
                            if btn_idx < 2 {
                                self.carousel_state.focus_zone =
                                    crate::big_picture::carousel::CarouselFocusZone::HeroBanner(
                                        btn_idx + 1,
                                    );
                            }
                        }
                        crate::big_picture::carousel::CarouselFocusZone::HeaderTabs => {
                            self.big_picture_screen = self.big_picture_screen.next();
                        }
                        crate::big_picture::carousel::CarouselFocusZone::Cards => {
                            self.carousel_state.move_right();
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::GameDetails {
                    // Retain focus on right actions panel
                } else if self.big_picture_screen == BigPictureScreen::GameManagement {
                    self.management_focus_index = match self.management_focus_index {
                        0 => 1,
                        2 => 3,
                        4 => 5,
                        6 => 7,
                        _ => self.management_focus_index,
                    };
                } else if self.big_picture_screen == BigPictureScreen::Sessions {
                    if !self.is_session_active() {
                        let total_profiles = self.get_all_saved_profiles_items().len();
                        if self.sessions_focus_index == 0 {
                            self.sessions_focus_index = 1;
                        } else if self.sessions_focus_index < total_profiles {
                            self.sessions_focus_index += 1;
                        } else if total_profiles > 0 {
                            self.sessions_focus_index = 1;
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::Settings {
                    self.settings_focus_zone = 1;
                } else if self.big_picture_screen == BigPictureScreen::CouchHub {
                    if self.hub_focus_zone == 0 {
                        self.hub_top_card_index = 1;
                    } else if self.hub_focus_zone == 2 {
                        self.hub_focus_zone = 3;
                    } else {
                        self.couch_hub_state.next_category();
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    match self.couch_setup_state.current_step {
                        crate::big_picture::setup::CouchSetupStep::Players => {
                            self.couch_setup_state.focused_slot =
                                self.couch_setup_state.slots.len();
                        }
                        crate::big_picture::setup::CouchSetupStep::ScreenLayout => {
                            self.couch_setup_state.focused_slot =
                                crate::big_picture::setup::AVAILABLE_LAYOUTS.len();
                        }
                        crate::big_picture::setup::CouchSetupStep::GameOptions => {
                            let cur_slot = self.couch_setup_state.focused_slot;
                            let opts_len = self
                                .selected_game
                                .as_ref()
                                .and_then(|g| {
                                    self.handlers
                                        .iter()
                                        .find(|h| &h.game_name == g || &h.guid == g)
                                })
                                .map(|h| h.options.len())
                                .unwrap_or(0);
                            if cur_slot >= opts_len {
                                // Already on next button
                            } else if let Some(h) = self.selected_game.as_ref().and_then(|g| {
                                self.handlers
                                    .iter()
                                    .find(|h| &h.game_name == g || &h.guid == g)
                            }) {
                                if let Some(opt) = h.options.get(cur_slot) {
                                    let cur_val = self
                                        .selected_handler_options
                                        .get(&opt.key)
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            opt.values.first().cloned().unwrap_or_default()
                                        });
                                    if let Some(pos) = opt.values.iter().position(|v| v == &cur_val)
                                    {
                                        let next_idx = (pos + 1) % opt.values.len();
                                        if let Some(nv) = opt.values.get(next_idx) {
                                            self.selected_handler_options
                                                .insert(opt.key.clone(), nv.clone());
                                        }
                                    }
                                }
                            }
                        }
                        crate::big_picture::setup::CouchSetupStep::Launch => {}
                    }
                } else {
                    self.carousel_state.move_right();
                }
                cx.notify();
            }
            "Confirm" => {
                if self.in_game_osd.is_visible {
                    self.handle_osd_confirm(cx);
                    return;
                }
                if self.couch_hub_state.custom_options_modal.is_some() {
                    self.confirm_couch_options_and_launch(cx);
                    return;
                }
                if let Some(h_name) = self.couch_hub_state.missing_exe_modal_handler.clone() {
                    self.locate_exe_for_handler(&h_name, cx);
                    return;
                }
                if self.big_picture_screen == BigPictureScreen::Carousel {
                    match self.carousel_state.focus_zone {
                        crate::big_picture::carousel::CarouselFocusZone::HeroBanner(btn_idx) => {
                            if btn_idx == 2 {
                                self.carousel_state.search_input_open = true;
                                cx.notify();
                            } else {
                                let filtered = self.carousel_state.filter_and_sort_handlers(
                                    &self.handlers,
                                    &self.carousel_state.search_query,
                                );
                                let target = filtered
                                    .get(self.carousel_state.selected_index)
                                    .map(|h| h.game_name.clone())
                                    .or_else(|| self.selected_game.clone());
                                if let Some(name) = target {
                                    if btn_idx == 0 {
                                        self.enter_couch_setup(&name, cx);
                                    } else {
                                        self.selected_game = Some(name);
                                        self.big_picture_screen = BigPictureScreen::GameDetails;
                                        cx.notify();
                                    }
                                }
                            }
                        }
                        crate::big_picture::carousel::CarouselFocusZone::HeaderTabs => {}
                        crate::big_picture::carousel::CarouselFocusZone::Cards => {
                            self.select_carousel_game(self.carousel_state.selected_index, cx);
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::GameDetails {
                    match self.details_focus_index {
                        0 => {
                            if let Some(ref g) = self.selected_game.clone() {
                                self.enter_couch_setup(g, cx);
                            }
                        }
                        1 => {
                            self.load_last_saved_profile_for_selected(cx);
                        }
                        2 => {
                            self.big_picture_screen = BigPictureScreen::GameManagement;
                            cx.notify();
                        }
                        3 => {
                            self.big_picture_screen = BigPictureScreen::Carousel;
                            cx.notify();
                        }
                        _ => {
                            if let Some(ref g) = self.selected_game.clone() {
                                self.enter_couch_setup(g, cx);
                            }
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::GameManagement {
                    let target_game = self.selected_game.clone().unwrap_or_default();
                    let handler_opt = self
                        .handlers
                        .iter()
                        .find(|h| h.game_name == target_game || h.guid == target_game)
                        .cloned();
                    let h_path = handler_opt.as_ref().and_then(|h| h.handler_path.clone());
                    let h_guid = handler_opt
                        .as_ref()
                        .map(|h| h.guid.clone())
                        .unwrap_or_default();
                    let exe_path = handler_opt.as_ref().and_then(|h| h.executable_path.clone());
                    match self.management_focus_index {
                        0 => self.open_game_handler_file(
                            h_path.as_deref(),
                            &h_guid,
                            &target_game,
                            cx,
                        ),
                        1 => {
                            let is_on_press = self
                                .app_settings
                                .custom_preferences
                                .get(&format!("{}:gamepad_mode", h_guid))
                                .map(|v| v == "on_button_press")
                                .unwrap_or(true);
                            self.set_gamepad_mode_preference(
                                &h_guid,
                                if is_on_press {
                                    "by_index"
                                } else {
                                    "on_button_press"
                                },
                                cx,
                            );
                        }
                        2 => self.open_game_exe_path(exe_path.as_deref(), &target_game, cx),
                        3 => self.cycle_steam_language_preference(&h_guid, cx),
                        4 => self.open_game_userprofile_path(&target_game, cx),
                        5 => self.open_game_assets_folder(&target_game, cx),
                        6 => self.trigger_change_game_icon_async(&h_guid, &target_game, cx),
                        7 => self.prompt_remove_game_confirmation(&h_guid, &target_game, cx),
                        8 => {
                            self.big_picture_screen = BigPictureScreen::GameDetails;
                            cx.notify();
                        }
                        _ => {}
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    let total_slots = self.couch_setup_state.slots.len();
                    match self.couch_setup_state.current_step {
                        crate::big_picture::setup::CouchSetupStep::Players => {
                            if self.couch_setup_state.focused_slot == total_slots {
                                self.couch_setup_state.next_step();
                                self.couch_setup_state.focused_slot = 0;
                            } else {
                                let focused = self.couch_setup_state.focused_slot;
                                if self.couch_setup_state.grabbed_slot.is_some() {
                                    self.couch_setup_state.drop_swap_target(focused);
                                } else {
                                    self.couch_setup_state.grab_slot(focused);
                                }
                            }
                            cx.notify();
                        }
                        crate::big_picture::setup::CouchSetupStep::ScreenLayout => {
                            self.couch_setup_state.next_step();
                            self.couch_setup_state.focused_slot = 0;
                            cx.notify();
                        }
                        crate::big_picture::setup::CouchSetupStep::GameOptions => {
                            let cur_slot = self.couch_setup_state.focused_slot;
                            let opts_len = self
                                .selected_game
                                .as_ref()
                                .and_then(|g| {
                                    self.handlers
                                        .iter()
                                        .find(|h| &h.game_name == g || &h.guid == g)
                                })
                                .map(|h| h.options.len())
                                .unwrap_or(0);
                            if cur_slot >= opts_len {
                                self.couch_setup_state.next_step();
                                self.couch_setup_state.focused_slot = 0;
                            } else if let Some(h) = self.selected_game.as_ref().and_then(|g| {
                                self.handlers
                                    .iter()
                                    .find(|h| &h.game_name == g || &h.guid == g)
                            }) {
                                if let Some(opt) = h.options.get(cur_slot) {
                                    let cur_val = self
                                        .selected_handler_options
                                        .get(&opt.key)
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            opt.values.first().cloned().unwrap_or_default()
                                        });
                                    if let Some(pos) = opt.values.iter().position(|v| v == &cur_val)
                                    {
                                        let next_idx = (pos + 1) % opt.values.len();
                                        if let Some(nv) = opt.values.get(next_idx) {
                                            self.selected_handler_options
                                                .insert(opt.key.clone(), nv.clone());
                                        }
                                    }
                                }
                            }
                            cx.notify();
                        }
                        crate::big_picture::setup::CouchSetupStep::Launch => {
                            self.launch_couch_session(cx);
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::CouchHub {
                    if self.hub_focus_zone == 0 {
                        if self.hub_top_card_index == 0 {
                            self.trigger_scan_games_async(cx);
                        } else {
                            self.trigger_browse_game_exe_async(cx);
                        }
                    } else if self.hub_focus_zone == 1 {
                        self.couch_hub_state.search_input_open = true;
                        cx.notify();
                    } else {
                        let pkgs = self.get_filtered_couch_hub_packages();
                        if self.couch_hub_state.selected_index < pkgs.len() {
                            let pkg = pkgs[self.couch_hub_state.selected_index];
                            let is_installed =
                                self.is_handler_or_pkg_installed(&pkg.id, &pkg.game_name);
                            if is_installed {
                                self.selected_game = Some(pkg.game_name.clone());
                                self.big_picture_screen = BigPictureScreen::GameDetails;
                                cx.notify();
                            } else {
                                let pid = pkg.id.clone();
                                self.download_and_play_couch_package(&pid, cx);
                            }
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::Sessions {
                    if self.is_session_active() {
                        match self.sessions_focus_index {
                            0 => self.resume_running_session_windows(cx),
                            1 => {
                                self.in_game_osd.open_reassign(0);
                                self.set_osd_visible(true);
                                cx.notify();
                            }
                            2 => {
                                self.big_picture_screen = BigPictureScreen::GameDetails;
                                cx.notify();
                            }
                            3 => self.save_current_session_profile(cx),
                            4 => self.stop_active_session_cleanly(cx),
                            _ => self.resume_running_session_windows(cx),
                        }
                    } else {
                        if self.sessions_focus_index == 0 {
                            self.save_current_session_profile(cx);
                        } else {
                            let prof_idx = self.sessions_focus_index.saturating_sub(1);
                            let saved = self.get_all_saved_profiles_items();
                            if let Some(prof) = saved.get(prof_idx) {
                                self.load_saved_profile_by_number(
                                    &prof.game_guid,
                                    prof.profile_number,
                                    cx,
                                );
                            }
                        }
                    }
                } else if self.big_picture_screen == BigPictureScreen::Settings {
                    if self.settings_focus_zone == 0 {
                        cx.notify();
                    } else if self.big_picture_settings_tab == 0 {
                        match self.settings_item_index {
                            0 => {
                                self.app_settings.start_in_big_picture =
                                    !self.app_settings.start_in_big_picture;
                                let _ = self.app_settings.save();
                                self.show_osd("Setting saved", 1200, cx);
                                cx.notify();
                            }
                            1 => self.cycle_interface_scale_preference(cx),
                            2 => self.cycle_preferred_display_preference(cx),
                            3 => {
                                self.show_osd("Sound effects toggled", 1200, cx);
                                cx.notify();
                            }
                            4 => {
                                self.show_osd("English (Default)", 1200, cx);
                                cx.notify();
                            }
                            5 => {
                                self.show_osd("Auto-updates enabled", 1200, cx);
                                cx.notify();
                            }
                            6 => {
                                self.app_settings.developer_mode =
                                    !self.app_settings.developer_mode;
                                let _ = self.app_settings.save();
                                self.show_osd(
                                    if self.app_settings.developer_mode {
                                        "Developer mode enabled"
                                    } else {
                                        "Developer mode disabled"
                                    },
                                    1200,
                                    cx,
                                );
                                cx.notify();
                            }
                            _ => {}
                        }
                    } else {
                        if self.settings_item_index == 0 {
                            self.show_osd("Assignment method: On button press", 1500, cx);
                            cx.notify();
                        } else if self.settings_item_index <= self.detected_devices_list.len() {
                            let dev_idx = self.settings_item_index - 1;
                            if let Some(dev) = self.detected_devices_list.get(dev_idx) {
                                self.identifying_controller_guid = Some(dev.guid.clone());
                                self.show_osd(&format!("Testing {}", dev.name), 1500, cx);
                                cx.notify();
                            }
                        } else {
                            self.refresh_detected_gamepads(cx);
                        }
                    }
                }
            }
            "Cancel" => {
                if self.in_game_osd.is_visible {
                    if self.in_game_osd.reassign_modal_open {
                        self.in_game_osd.close_reassign();
                    } else {
                        self.set_osd_visible(false);
                    }
                    cx.notify();
                    return;
                }
                if self.couch_hub_state.custom_options_modal.is_some() {
                    self.couch_hub_state.custom_options_modal = None;
                    cx.notify();
                    return;
                }
                if self.couch_hub_state.missing_exe_modal_handler.is_some() {
                    self.couch_hub_state.dismiss_prompt();
                    cx.notify();
                    return;
                }
                if self.big_picture_screen == BigPictureScreen::GameManagement {
                    self.big_picture_screen = BigPictureScreen::GameDetails;
                    cx.notify();
                } else if self.big_picture_screen == BigPictureScreen::GameDetails {
                    self.big_picture_screen = BigPictureScreen::Carousel;
                    cx.notify();
                } else if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    if self.couch_setup_state.grabbed_slot.is_some() {
                        self.couch_setup_state.cancel_grab();
                    } else if self.couch_setup_state.current_step
                        != crate::big_picture::setup::CouchSetupStep::Players
                    {
                        self.couch_setup_state.prev_step();
                    } else {
                        self.big_picture_screen = BigPictureScreen::GameDetails;
                    }
                    cx.notify();
                } else if self.big_picture_screen == BigPictureScreen::CouchHub
                    || self.big_picture_screen == BigPictureScreen::Sessions
                    || self.big_picture_screen == BigPictureScreen::Settings
                {
                    self.big_picture_screen = BigPictureScreen::Carousel;
                    cx.notify();
                } else {
                    self.toggle_big_picture(cx);
                }
            }
            "QuickLaunch" => {
                if self.in_game_osd.is_visible {
                    self.set_osd_visible(false);
                    cx.notify();
                    return;
                }
                if self.couch_hub_state.custom_options_modal.is_some() {
                    self.confirm_couch_options_and_launch(cx);
                    return;
                }
                if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    self.launch_couch_session(cx);
                } else if self.big_picture_screen == BigPictureScreen::CouchHub {
                    let pkgs = self.get_filtered_couch_hub_packages();
                    if self.couch_hub_state.selected_index < pkgs.len() {
                        let pid = pkgs[self.couch_hub_state.selected_index].id.clone();
                        self.download_and_play_couch_package(&pid, cx);
                    }
                } else {
                    self.select_carousel_game(self.carousel_state.selected_index, cx);
                }
            }
            "Context1" => {
                if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    let focused = self.couch_setup_state.focused_slot;
                    self.couch_setup_state.remove_slot(focused);
                    cx.notify();
                }
            }
            "Context2" => {
                if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    let focused = self.couch_setup_state.focused_slot;
                    if focused < self.couch_setup_state.slots.len() {
                        let guid = self.couch_setup_state.slots[focused].device_guid.clone();
                        self.couch_setup_state
                            .ignore_device(&guid, &mut self.app_settings.ignored_device_guids);
                        let _ = self.app_settings.save();
                    }
                    cx.notify();
                }
            }
            "NextTab" => {
                if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    self.couch_setup_state.next_step();
                } else {
                    self.big_picture_screen = self.big_picture_screen.next();
                    if self.big_picture_screen == BigPictureScreen::CouchHub
                        && self.hub_packages.is_empty()
                    {
                        self.trigger_hub_search_async(cx);
                    }
                }
                cx.notify();
            }
            "PrevTab" => {
                if self.big_picture_screen == BigPictureScreen::CouchSetup {
                    self.couch_setup_state.prev_step();
                } else {
                    self.big_picture_screen = self.big_picture_screen.prev();
                    if self.big_picture_screen == BigPictureScreen::CouchHub
                        && self.hub_packages.is_empty()
                    {
                        self.trigger_hub_search_async(cx);
                    }
                }
                cx.notify();
            }
            _ => {}
        }
    }

    /// Launches a splitscreen session immediately from Couch Setup.
    pub fn launch_couch_session(&mut self, cx: &mut Context<Self>) {
        let target = match self.selected_game {
            Some(ref g) => g.clone(),
            None => return,
        };
        let player_count = self.couch_setup_state.slots.len();
        if player_count == 0 {
            self.status_message = "Cannot launch: No player slots assigned.".to_string();
            cx.notify();
            return;
        }

        // Ensure default options are populated for any unconfigured keys
        if let Some(handler) = self.handlers.iter().find(|h| {
            h.game_name.eq_ignore_ascii_case(&target) || h.guid.eq_ignore_ascii_case(&target)
        }) {
            for opt in &handler.options {
                if !self.selected_handler_options.contains_key(&opt.key) {
                    if let Some(first_val) = opt.values.first() {
                        self.selected_handler_options
                            .insert(opt.key.clone(), first_val.clone());
                    }
                }
            }
        }
        self.player_count = player_count;
        self.selected_layout = self.couch_setup_state.selected_layout.clone();
        self.player_device_assignments = self
            .couch_setup_state
            .slots
            .iter()
            .map(|s| {
                if s.is_complete_raw_kbm() {
                    let kb = s.raw_keyboard_name.as_deref().unwrap_or("Keyboard");
                    let m = s.raw_mouse_name.as_deref().unwrap_or("Mouse");
                    format!("{} + {} (ProtoInput)", kb, m)
                } else if s.has_raw_keyboard() {
                    let kb = s.raw_keyboard_name.as_deref().unwrap_or("Keyboard");
                    format!("{} (ProtoInput)", kb)
                } else if s.has_raw_mouse() {
                    let m = s.raw_mouse_name.as_deref().unwrap_or("Mouse");
                    format!("{} (ProtoInput)", m)
                } else if s.is_paired_kbm() {
                    "Keyboard & Mouse (Paired)".to_string()
                } else if let Some(id) = s.device_id {
                    format!("Gamepad #{} ({})", id, s.device_name)
                } else {
                    "Keyboard & Mouse Slot".to_string()
                }
            })
            .collect();

        if let Some(ref launcher) = self.callbacks.on_launch {
            launcher(
                &target,
                player_count,
                &self.selected_layout,
                &self.selected_handler_options,
                &self.player_device_assignments,
                &self.player_monitor_assignments,
                &self.player_audio_device_assignments,
            );
        }
        self.status_message = format!(
            "Splitscreen session launched for '{}' ({} players)!",
            target, player_count
        );
        cx.notify();
    }

    /// Advances disconnect resilience timers for all couch player slots and repaints.
    pub fn tick_disconnect_resilience(
        &mut self,
        delta: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        self.couch_setup_state.tick_disconnect(delta);
        cx.notify();
    }

    /// Advances disconnect resilience timers on state without notifying a GPUI context.
    pub fn tick_disconnect_resilience_state(&mut self, delta: std::time::Duration) {
        self.couch_setup_state.tick_disconnect(delta);
    }

    /// Synchronizes detected devices and notifies CouchSetupState of disconnects and reconnects.
    pub fn sync_devices(&mut self, devices: Vec<DeviceItem>) {
        // Detect newly disconnected device IDs:
        for prev in &self.detected_devices_list {
            let still_connected = devices.iter().any(|d| d.id == prev.id && d.is_connected);
            if prev.is_connected && !still_connected {
                self.couch_setup_state
                    .handle_device_disconnect_by_id(prev.id);
            }
        }
        // Detect reconnected devices:
        for new_dev in &devices {
            if new_dev.is_connected {
                let was_connected = self
                    .detected_devices_list
                    .iter()
                    .any(|d| d.id == new_dev.id && d.is_connected);
                if !was_connected {
                    let fp = nucleus_input::gamepad::DeviceFingerprint::new(
                        &new_dev.guid,
                        &new_dev.name,
                        &nucleus_common::types::DeviceType::XInputGamepad,
                    );
                    self.couch_setup_state.handle_device_reconnect(
                        new_dev.id,
                        &new_dev.guid,
                        &new_dev.name,
                        Some(&fp),
                    );
                }
            }
        }
        self.detected_devices = devices.len();
        self.detected_devices_list = devices;
        self.sync_player_assignments();
    }
}
