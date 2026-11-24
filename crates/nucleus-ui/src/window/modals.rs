//! Modal dialogs, popup overlays, context menus, and controller remapping.

use super::*;
use std::sync::Arc;

impl MainWindow {
    /// Handles physical keypress recording for lock key or shortcuts.
    pub fn handle_key_recording(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let key_raw = event.keystroke.key.as_str();
        if key_raw.eq_ignore_ascii_case("escape") {
            self.settings_view.recording_hotkey = None;
            self.status_message = "Key recording cancelled.".to_string();
            cx.notify();
            return true;
        }

        // Ignore lonely modifier presses
        if matches!(
            key_raw.to_lowercase().as_str(),
            "control" | "shift" | "alt" | "meta" | "ctrl"
        ) {
            return true;
        }

        if let Some(target) = self.settings_view.recording_hotkey.clone() {
            if target == "lock" {
                let normalized = crate::settings_view::normalize_key_input(key_raw);
                self.app_settings.lock_inputs_key = normalized.clone();
                self.settings_view.recording_hotkey = None;
                self.status_message =
                    format!("Lock Inputs Key successfully updated to: {}", normalized);
                self.show_osd(&format!("Lock Key: {}", normalized), 1200, cx);
            } else {
                let key_clean = crate::settings_view::normalize_key_input(key_raw);
                let combo = crate::settings_view::format_shortcut_combo(
                    &event.keystroke.modifiers,
                    &key_clean,
                );
                crate::settings_view::set_hotkey_value(
                    &mut self.app_settings,
                    &target,
                    combo.clone(),
                );
                self.settings_view.recording_hotkey = None;
                self.status_message = format!("Shortcut successfully updated to: {}", combo);
                self.show_osd(&format!("Shortcut: {}", combo), 1200, cx);
            }
            cx.notify();
            return true;
        }
        false
    }

    /// Renders the interactive physical keypress recording modal dialog when active.
    pub(super) fn render_hotkey_recording_modal(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(ref target) = self.settings_view.recording_hotkey {
            let target_name = match target.as_str() {
                "lock" => "Lock Inputs Toggle Key (Default: End)".to_string(),
                other => crate::settings_view::APP_SHORTCUTS
                    .iter()
                    .find(|s| s.id == other)
                    .map(|s| format!("{} (Default: {})", s.name, s.default_val))
                    .unwrap_or_else(|| "Custom Shortcut".to_string()),
            };

            let focus_handle = self
                .hotkey_record_focus
                .clone()
                .unwrap_or_else(|| cx.focus_handle());

            let on_cancel = cx.listener(|this: &mut MainWindow, _, _, cx| {
                this.settings_view.recording_hotkey = None;
                this.status_message = "Key recording cancelled.".to_string();
                cx.notify();
            });

            let on_key_down = cx.listener(
                |this: &mut MainWindow, event: &gpui::KeyDownEvent, _window, cx| {
                    this.handle_key_recording(event, cx);
                },
            );

            return Some(crate::osd::render_hotkey_recording_modal(
                &target_name,
                &focus_handle,
                on_cancel,
                on_key_down,
            ));
        }
        None
    }

    /// Executes the action confirmed by the user in `CouchConfirmDialog`.
    pub fn execute_confirm_action(
        &mut self,
        action: &crate::models::ConfirmAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            crate::models::ConfirmAction::ResetDefaults => {
                self.app_settings = nucleus_profile::AppSettings::default();
                self.status_message = "Preferences restored to defaults".to_string();
                self.show_osd("Defaults Restored", 800, cx);
            }
            crate::models::ConfirmAction::DeleteNumberedProfile(guid, num) => {
                if let Some(ref deleter) = self.callbacks.on_delete_numbered_profile {
                    let _ = deleter(guid, *num);
                    self.status_message = format!("Deleted Profile {}!", num);
                    self.show_osd(&format!("Profile #{} Deleted", num), 800, cx);
                    if let Some(ref list_cb) = self.callbacks.on_list_profiles {
                        self.saved_profiles = list_cb(guid);
                    }
                    if self.active_profile_id == Some(*num) {
                        self.active_profile_id = None;
                    }
                }
            }
            crate::models::ConfirmAction::RemoveGame(guid) => {
                let g = guid.clone();
                self.handlers.retain(|h| &h.guid != &g);
                self.handler_names.retain(|n| n != &g);
                if let Some(ref cb) = self.callbacks.on_remove_game {
                    cb(&g);
                }
                if self.is_big_picture {
                    self.selected_game = None;
                    self.big_picture_screen = BigPictureScreen::Carousel;
                }
                self.status_message = "Game removed from library".to_string();
                self.show_osd("Game Removed", 1500, cx);
            }
            crate::models::ConfirmAction::DeleteSavePath(guid) => {
                if let Some(ref cb) = self.callbacks.on_delete_save_path {
                    cb(guid);
                }
                if let Some(h) = self.handlers.iter().find(|it| &it.guid == guid) {
                    if let Some(home) = dirs::home_dir() {
                        let p = home.join("NucleusCoop").join(&h.game_name);
                        if p.exists() {
                            let _ = std::fs::remove_dir_all(&p);
                        }
                    }
                }
                self.status_message = "Deleted UserProfile Save Path.".to_string();
                self.show_osd("Save Data Deleted", 800, cx);
            }
        }
    }

    /// Renders the right-click contextual menu for game cards (delegated to `CouchContextMenu`).
    pub(super) fn render_context_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let h = self.context_menu_handler.as_ref()?;
        let guid = h.guid.clone();
        let game_name = h.game_name.clone();
        let exe_path_opt = h.executable_path.clone();
        let handler_path_opt = h.handler_path.clone();
        let handlers_dir = self.handlers_dir.clone();
        let text_editor_path = self.app_settings.text_editor_path.clone();
        let is_picker_open = self.context_menu_language_picker;

        Some(
            CouchContextMenu::new(h.clone())
                .handlers_dir(handlers_dir.clone())
                .text_editor_path(text_editor_path)
                .language_picker_open(is_picker_open)
                .on_close(cx.listener(|this, _, _, cx| {
                    this.context_menu_handler = None;
                    cx.notify();
                }))
                .on_open_handler({
                    let h_path = handler_path_opt.clone();
                    let h_dir = handlers_dir.clone();
                    let g_name = game_name.clone();
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        let candidate = h_path.as_deref().and_then(|p| {
                            let path = std::path::Path::new(p);
                            if path.is_file() {
                                Some(path.to_path_buf())
                            } else {
                                None
                            }
                        }).or_else(|| {
                            let p1 = std::path::Path::new(&h_dir).join(format!("{}.js", g_name));
                            if p1.is_file() {
                                Some(p1)
                            } else {
                                let p2 = std::path::Path::new(&h_dir).join(format!("{}.js", g_id));
                                if p2.is_file() {
                                    Some(p2)
                                } else {
                                    None
                                }
                            }
                        });

                        if let Some(target_file) = candidate {
                            let target_str = target_file.to_string_lossy().to_string();
                            let editor = if !this.app_settings.text_editor_path.is_empty() && this.app_settings.text_editor_path != "Default" {
                                &this.app_settings.text_editor_path
                            } else {
                                "notepad++.exe"
                            };
                            if std::process::Command::new(editor).arg(&target_str).spawn().is_err() {
                                let _ = std::process::Command::new("notepad.exe").arg(&target_str).spawn();
                            }
                            this.status_message = format!("Opened handler script: {}", target_file.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
                        } else {
                            let _ = std::process::Command::new("explorer").arg(&h_dir).spawn();
                            this.status_message = format!("Opened handlers folder: {}", h_dir);
                        }
                        this.context_menu_handler = None;
                        cx.notify();
                    })
                })
                .on_open_exe({
                    let exe_p = exe_path_opt.clone();
                    let g_name = game_name.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(ref p) = exe_p {
                            let p_path = std::path::Path::new(p);
                            let dir_to_open = if p_path.is_file() {
                                p_path.parent().map(|d| d.to_path_buf())
                            } else if p_path.is_dir() {
                                Some(p_path.to_path_buf())
                            } else if let Some(parent) = p_path.parent() {
                                if !parent.as_os_str().is_empty() && parent.is_dir() {
                                    Some(parent.to_path_buf())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some(dir) = dir_to_open {
                                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                this.status_message = format!("Opened game directory: {}", dir.display());
                            } else {
                                this.status_message = format!("Unable to open original executable path for '{}': directory not found.", g_name);
                            }
                        } else {
                            this.status_message = format!("No executable located for '{}' yet.", g_name);
                        }
                        this.context_menu_handler = None;
                        cx.notify();
                    })
                })
                .on_open_userprofile({
                    let g_name = game_name.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(home) = dirs::home_dir() {
                            let p = home.join("NucleusCoop").join(&g_name);
                            if !p.exists() {
                                let _ = std::fs::create_dir_all(&p);
                            }
                            let _ = std::process::Command::new("explorer").arg(&p).spawn();
                        }
                        this.context_menu_handler = None;
                        cx.notify();
                    })
                })
                .on_delete_userprofile({
                    let g_id = guid.clone();
                    let g_name = game_name.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu_handler = None;
                        this.confirm_dialog = Some(crate::ConfirmDialogState {
                            title: format!("Delete Saves for '{}'?", g_name),
                            message: format!("Are you sure you want to permanently delete the UserProfile save data for '{}'? All saved games and configs under %USERPROFILE%\\NucleusCoop will be deleted.", g_name),
                            confirm_label: "Delete Saves".to_string(),
                            is_destructive: true,
                            action: crate::ConfirmAction::DeleteSavePath(g_id.clone()),
                        });
                        cx.notify();
                    })
                })
                .on_toggle_assign_by_index({
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == g_id) {
                            item.assign_gamepads_by_index = !item.assign_gamepads_by_index;
                            if item.assign_gamepads_by_index {
                                item.assign_gamepads_on_button_press = false;
                            }
                            if let Some(ref cb) = this.callbacks.on_save_profile {
                                cb(item);
                            }
                            this.context_menu_handler = Some(item.clone());
                        }
                        cx.notify();
                    })
                })
                .on_toggle_assign_on_press({
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == g_id) {
                            item.assign_gamepads_on_button_press = !item.assign_gamepads_on_button_press;
                            if item.assign_gamepads_on_button_press {
                                item.assign_gamepads_by_index = false;
                            }
                            if let Some(ref cb) = this.callbacks.on_save_profile {
                                cb(item);
                            }
                            this.context_menu_handler = Some(item.clone());
                        }
                        cx.notify();
                    })
                })
                .on_toggle_keep_content({
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == g_id) {
                            item.keep_instances_content_folder = !item.keep_instances_content_folder;
                            if let Some(ref cb) = this.callbacks.on_save_profile {
                                cb(item);
                            }
                            this.context_menu_handler = Some(item.clone());
                        }
                        cx.notify();
                    })
                })
                .on_toggle_disable_profile({
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == g_id) {
                            item.disable_profile = !item.disable_profile;
                            if let Some(ref cb) = this.callbacks.on_save_profile {
                                cb(item);
                            }
                            this.context_menu_handler = Some(item.clone());
                        }
                        cx.notify();
                    })
                })
                .on_toggle_disable_update({
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == g_id) {
                            item.disable_handler_update = !item.disable_handler_update;
                            if let Some(ref cb) = this.callbacks.on_save_profile {
                                cb(item);
                            }
                            this.context_menu_handler = Some(item.clone());
                        }
                        cx.notify();
                    })
                })
                .on_change_icon({
                    let g_id = guid.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.status_message = "Browsing for game icon...".to_string();
                        cx.notify();
                        let target_guid = g_id.clone();
                        cx.spawn(async move |this, cx| {
                            let icon_opt = crate::dialogs::pick_file_async(
                                "Select Game Icon",
                                "Icons & Images (*.ico;*.png;*.exe)|*.ico;*.png;*.exe|All Files (*.*)|*.*",
                            )
                            .await;
                            if let Some(icon_path) = icon_opt {
                                let path_str = icon_path.to_string_lossy().to_string();
                                let _ = this.update(&mut *cx, |this, cx| {
                                    if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == target_guid) {
                                        item.custom_game_icon = Some(path_str.clone());
                                        if let Some(ref cb) = this.callbacks.on_save_profile {
                                            cb(item);
                                        }
                                        this.context_menu_handler = Some(item.clone());
                                    }
                                    this.status_message = format!("Set custom icon: {}", path_str);
                                    cx.notify();
                                });
                            } else {
                                let _ = this.update(&mut *cx, |this, cx| {
                                    this.status_message = "Game icon selection cancelled.".to_string();
                                    cx.notify();
                                });
                            }
                        })
                        .detach();
                    })
                })
                .on_open_assets(cx.listener(|this, _, _, cx| {
                    let p = nucleus_common::Globals::try_get()
                        .map(|g| g.install_root.join("gui").join("covers"))
                        .unwrap_or_else(|| std::path::PathBuf::from("gui/covers"));
                    if !p.exists() {
                        let _ = std::fs::create_dir_all(&p);
                    }
                    if let Some(g) = nucleus_common::Globals::try_get() {
                        let _ = std::fs::create_dir_all(g.install_root.join("gui").join("screenshots"));
                        let _ = std::fs::create_dir_all(g.install_root.join("gui").join("icons"));
                    }
                    let _ = std::process::Command::new("explorer").arg(&p).spawn();
                    this.context_menu_handler = None;
                    this.status_message = format!("Opened game assets folder: {}", p.display());
                    cx.notify();
                }))
                .on_toggle_language_picker(cx.listener(|this, _, _, cx| {
                    this.context_menu_language_picker = !this.context_menu_language_picker;
                    cx.notify();
                }))
                .on_select_language({
                    let gid = guid.clone();
                    let sel_cb = cx.listener(move |this, lang: &str, _, cx| {
                        let lang_str = lang.to_string();
                        if let Some(item) = this.handlers.iter_mut().find(|it| it.guid == gid) {
                            item.steam_language = Some(lang_str.clone());
                            if let Some(ref cb) = this.callbacks.on_save_profile {
                                cb(item);
                            }
                            this.context_menu_handler = Some(item.clone());
                        }
                        this.context_menu_language_picker = false;
                        this.status_message = format!("Steam Language set to: {}", lang_str);
                        cx.notify();
                    });
                    move |lang: &str, win, app| {
                        sel_cb(lang, win, app);
                    }
                })
                .on_remove_game({
                    let g_id = guid.clone();
                    let g_name = game_name.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.context_menu_handler = None;
                        this.confirm_dialog = Some(crate::ConfirmDialogState {
                            title: format!("Remove '{}' from Library?", g_name),
                            message: format!("Are you sure you want to remove '{}' from your game library? Your local game installation and save files will remain untouched.", g_name),
                            confirm_label: "Remove Game".to_string(),
                            is_destructive: true,
                            action: crate::ConfirmAction::RemoveGame(g_id.clone()),
                        });
                        cx.notify();
                    })
                })
                .into_any_element(),
        )
    }

    /// Renders the modal confirmation dialog for destructive/irreversible actions (delegated to `CouchConfirmDialog`).
    pub(super) fn render_confirm_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dlg = self.confirm_dialog.as_ref()?;
        let action = dlg.action.clone();

        Some(
            CouchConfirmDialog::new(dlg.clone())
                .on_confirm(cx.listener(move |this, _, _, cx| {
                    this.execute_confirm_action(&action, cx);
                    this.confirm_dialog = None;
                    cx.notify();
                }))
                .on_cancel(cx.listener(|this, _, _, cx| {
                    this.confirm_dialog = None;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }
    /// Remaps a controller action to a target button name.
    pub fn remap_controller_action(
        &mut self,
        action: &str,
        button_name: &str,
        cx: &mut Context<Self>,
    ) {
        let is_single = matches!(action, "drag_drop" | "right_click" | "left_click");
        let new_binding = if is_single {
            button_name.to_string()
        } else {
            let cur = self.app_settings.get_gamepad_binding(action);
            if let Some((modifier, _)) = cur.split_once('+') {
                format!("{}+{}", modifier, button_name)
            } else {
                format!("Guide+{}", button_name)
            }
        };

        self.app_settings
            .set_gamepad_binding(action, new_binding.clone());
        let _ = self.app_settings.save();
        self.remapping_controller_shortcut = None;
        let action_display = match action {
            "unfocus" => "Toggle Unfocus",
            "close" => "Close Nucleus",
            "stop" => "Stop Session",
            "topmost" => "Toggle Top Most",
            "reset_windows" => "Reset Windows",
            "cutscenes" => "Cutscenes Mode",
            "switch_layout" => "Switch Layouts",
            "lock_inputs" => "Lock Input Key",
            "release_cursor" => "ALT+TAB",
            "lock_ui" => "Lock UI Control",
            "osk" => "On screen KB",
            "drag_drop" => "Drag and Drop",
            "right_click" => "Right Click",
            "left_click" => "Left Click",
            other => other,
        };
        self.status_message = format!("Remapped {} to {}", action_display, new_binding);
        self.show_osd(&format!("{}: {}", action_display, new_binding), 1500, cx);
        cx.notify();
    }

    /// Helper to render a shortcut combo row that can be clicked to remap.
    fn render_shortcut_row(
        &self,
        action: &'static str,
        label: &'static str,
        is_ps: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let binding = self.app_settings.get_gamepad_binding(action);
        let (mod_part, btn_part) = if let Some((m, b)) = binding.split_once('+') {
            (m, b)
        } else {
            ("Guide", binding)
        };

        let is_active = self.remapping_controller_shortcut.as_deref() == Some(action);
        let guide_glyph = format_gamepad_button_glyph(mod_part, is_ps);
        let btn_glyph = format_gamepad_button_glyph(btn_part, is_ps);

        div()
            .id(ElementId::Name(format!("shortcut-row-{}", action).into()))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(if is_active {
                theme::accent()
            } else {
                theme::border_color()
            })
            .bg(if is_active {
                theme::accent().opacity(0.18)
            } else {
                theme::bg_panel()
            })
            .hover(|s| {
                if is_active {
                    s
                } else {
                    s.bg(theme::bg_hover())
                }
            })
            .tooltip(crate::make_tooltip(if is_active {
                "Listening for replacement button... (Click to cancel)"
            } else {
                "Click to remap this controller shortcut"
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.remapping_controller_shortcut.as_deref() == Some(action) {
                    this.remapping_controller_shortcut = None;
                } else {
                    this.remapping_controller_shortcut = Some(action.to_string());
                    this.show_osd(&format!("Remapping {}... Press button", label), 1500, cx);
                }
                cx.notify();
            }))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_active {
                        theme::accent()
                    } else {
                        theme::text_primary()
                    })
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .size(px(18.0))
                            .rounded_full()
                            .bg(rgb(0xffffff))
                            .text_color(rgb(0x000000))
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(guide_glyph),
                    )
                    .child(div().text_xs().text_color(theme::text_muted()).child("+"))
                    .child(
                        div()
                            .min_w(px(24.0))
                            .h(px(20.0))
                            .px_1p5()
                            .rounded_sm()
                            .bg(if is_active {
                                theme::accent()
                            } else {
                                theme::bg_subtle()
                            })
                            .border_1()
                            .border_color(if is_active {
                                theme::accent()
                            } else {
                                theme::border_color()
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if is_active { "..." } else { btn_glyph }),
                    ),
            )
            .into_any_element()
    }

    /// Helper to render a single navigation action row that can be clicked to remap.
    fn render_nav_single_row(
        &self,
        action: &'static str,
        label: &'static str,
        is_ps: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let binding = self.app_settings.get_gamepad_binding(action);
        let is_active = self.remapping_controller_shortcut.as_deref() == Some(action);
        let btn_glyph = format_gamepad_button_glyph(binding, is_ps);

        div()
            .id(ElementId::Name(format!("nav-row-{}", action).into()))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(if is_active {
                theme::accent()
            } else {
                theme::border_color()
            })
            .bg(if is_active {
                theme::accent().opacity(0.18)
            } else {
                theme::bg_panel()
            })
            .hover(|s| {
                if is_active {
                    s
                } else {
                    s.bg(theme::bg_hover())
                }
            })
            .tooltip(crate::make_tooltip(if is_active {
                "Listening for replacement button... (Click to cancel)"
            } else {
                "Click to remap this navigation button"
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.remapping_controller_shortcut.as_deref() == Some(action) {
                    this.remapping_controller_shortcut = None;
                } else {
                    this.remapping_controller_shortcut = Some(action.to_string());
                    this.show_osd(&format!("Remapping {}... Press button", label), 1500, cx);
                }
                cx.notify();
            }))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_active {
                        theme::accent()
                    } else {
                        theme::text_primary()
                    })
                    .child(label),
            )
            .child(
                div()
                    .min_w(px(28.0))
                    .h(px(20.0))
                    .px_2()
                    .rounded_sm()
                    .bg(if is_active {
                        theme::accent()
                    } else {
                        theme::bg_subtle()
                    })
                    .border_1()
                    .border_color(if is_active {
                        theme::accent()
                    } else {
                        theme::border_color()
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_active { "..." } else { btn_glyph }),
            )
            .into_any_element()
    }

    /// Renders the legacy controller shortcuts setup modal matching XInputShortcutsSetup (media_1793137385955_044e590e.png).
    pub(super) fn render_controller_shortcuts_modal(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.show_controller_shortcuts_modal {
            return None;
        }

        let is_ps = self
            .app_settings
            .gamepad_type
            .eq_ignore_ascii_case("Playstation");
        let deadzone_val = self.app_settings.gamepad_deadzone;
        let is_enabled = self.app_settings.gamepad_shortcuts_enabled;

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(theme::overlay_scrim())
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .child(
                    div()
                        .id("controller-shortcuts-modal-dialog")
                        .flex()
                        .flex_col()
                        .w_full()
                        .max_w(px(880.0))
                        .max_h(px(600.0))
                        .p_5()
                        .gap_3()
                        .bg(rgb(0x111113))
                        .rounded_xl()
                        .border_1()
                        .border_color(theme::border_color())
                        .shadow_lg()
                        // Header: Title, Enable Checkbox & Close '✕'
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb_2()
                                .border_b_1()
                                .border_color(theme::border_color())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Icon::new(IconName::Gamepad2)
                                                .size_4()
                                                .text_color(theme::accent()),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child("Controller Shortcuts & UI Navigation Setup"),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_4()
                                        // Enabled Checkbox toggle (parity with XInputShortcutsSetup.cs enabled_chk)
                                        .child(
                                            div()
                                                .id("chk-controller-shortcuts-enabled")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .tooltip(crate::make_tooltip("Toggle controller navigation and shortcuts system"))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.app_settings.gamepad_shortcuts_enabled = !this.app_settings.gamepad_shortcuts_enabled;
                                                    let _ = this.app_settings.save();
                                                    cx.notify();
                                                }))
                                                .child(
                                                    div()
                                                        .size(px(16.0))
                                                        .rounded_sm()
                                                        .border_1()
                                                        .border_color(if is_enabled { theme::accent() } else { theme::border_color() })
                                                        .bg(if is_enabled { theme::accent() } else { theme::bg_main() })
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_xs()
                                                        .text_color(theme::text_on_accent())
                                                        .child(if is_enabled { "✓" } else { "" }),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(theme::text_primary())
                                                        .child("Enable Shortcuts"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("btn-close-controller-shortcuts")
                                                .cursor_pointer()
                                                .size(px(24.0))
                                                .rounded_md()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_muted())
                                                .hover(|s| s.bg(theme::bg_hover()).text_color(theme::text_primary()))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.show_controller_shortcuts_modal = false;
                                                    this.remapping_controller_shortcut = None;
                                                    cx.notify();
                                                }))
                                                .child(Icon::new(IconName::X).size(px(14.0))),
                                        ),
                                ),
                        )
                        // Content Body: 3-column layout matching media_1793137385955_044e590e.png
                        .child(
                            div()
                                .flex()
                                .items_start()
                                .gap_4()
                                .w_full()
                                .flex_1()
                                .min_h_0()
                                // Column 1: Gamepad Type, Reminder & UI Navigation Settings
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(px(250.0))
                                        .flex_shrink_0()
                                        .gap_3()
                                        // Row with Gamepad Type & Reminder
                                        .child(
                                            div()
                                                .flex()
                                                .items_start()
                                                .gap_2()
                                                .w_full()
                                                // GroupBox: Gamepad Type
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .p_2p5()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(theme::border_color())
                                                        .bg(theme::bg_panel())
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1p5()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::text_muted())
                                                                .child("Gamepad Type"),
                                                        )
                                                        .child(
                                                            div()
                                                                .id("rad-type-xbox")
                                                                .cursor_pointer()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1p5()
                                                                .text_xs()
                                                                .text_color(if !is_ps { theme::text_primary() } else { theme::text_muted() })
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.app_settings.gamepad_type = "Xbox".to_string();
                                                                    let _ = this.app_settings.save();
                                                                    this.status_message = "Gamepad type set to Xbox".to_string();
                                                                    cx.notify();
                                                                }))
                                                                .child(
                                                                    div()
                                                                        .size(px(14.0))
                                                                        .rounded_sm()
                                                                        .border_1()
                                                                        .border_color(if !is_ps { theme::accent() } else { theme::border_color() })
                                                                        .bg(if !is_ps { theme::accent() } else { theme::bg_main() })
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .text_color(theme::text_on_accent())
                                                                        .child(if !is_ps { "✓" } else { "" }),
                                                                )
                                                                .child("Xbox"),
                                                        )
                                                        .child(
                                                            div()
                                                                .id("rad-type-ps")
                                                                .cursor_pointer()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1p5()
                                                                .text_xs()
                                                                .text_color(if is_ps { theme::text_primary() } else { theme::text_muted() })
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.app_settings.gamepad_type = "Playstation".to_string();
                                                                    let _ = this.app_settings.save();
                                                                    this.status_message = "Gamepad type set to PlayStation".to_string();
                                                                    cx.notify();
                                                                }))
                                                                .child(
                                                                    div()
                                                                        .size(px(14.0))
                                                                        .rounded_sm()
                                                                        .border_1()
                                                                        .border_color(if is_ps { theme::accent() } else { theme::border_color() })
                                                                        .bg(if is_ps { theme::accent() } else { theme::bg_main() })
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .text_color(theme::text_on_accent())
                                                                        .child(if is_ps { "✓" } else { "" }),
                                                                )
                                                                .child("Playstation"),
                                                        ),
                                                )
                                                // GroupBox: Reminder
                                                .child(
                                                    div()
                                                        .w(px(80.0))
                                                        .px_2()
                                                        .py_2p5()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(theme::border_color())
                                                        .bg(theme::bg_panel())
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::text_muted())
                                                                .whitespace_nowrap()
                                                                .child("Reminder"),
                                                        )
                                                        .child(
                                                            div()
                                                                .size(px(28.0))
                                                                .rounded_full()
                                                                .bg(rgb(0xffffff))
                                                                .text_color(rgb(0x000000))
                                                                .font_weight(FontWeight::BOLD)
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .child(if is_ps { "🅟" } else { "⨂" }),
                                                        ),
                                                ),
                                        )
                                        // GroupBox: UI Navigation Settings (remappable)
                                        .child(
                                            div()
                                                .p_3()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(theme::border_color())
                                                .bg(theme::bg_panel())
                                                .flex()
                                                .flex_col()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(theme::text_muted())
                                                        .child("UI Navigation Settings"),
                                                )
                                                .child(self.render_shortcut_row("osk", "On screen KB:", is_ps, cx))
                                                .child(self.render_nav_single_row("drag_drop", "Drag and Drop:", is_ps, cx))
                                                .child(self.render_nav_single_row("right_click", "Right Click:", is_ps, cx))
                                                .child(self.render_nav_single_row("left_click", "Left Click:", is_ps, cx))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .pt_1()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme::text_muted())
                                                                .child("Deadzone:"),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .id("btn-deadzone-dec")
                                                                        .cursor_pointer()
                                                                        .size(px(20.0))
                                                                        .rounded_sm()
                                                                        .bg(theme::bg_active())
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(theme::text_primary())
                                                                        .hover(|s| s.bg(theme::bg_hover()))
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.app_settings.gamepad_deadzone = (this.app_settings.gamepad_deadzone - 1000).max(1000);
                                                                            let _ = this.app_settings.save();
                                                                            cx.notify();
                                                                        }))
                                                                        .child("-"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .px_2()
                                                                        .py_0p5()
                                                                        .rounded_sm()
                                                                        .border_1()
                                                                        .border_color(theme::border_color())
                                                                        .bg(theme::bg_main())
                                                                        .text_xs()
                                                                        .text_color(theme::accent())
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .child(deadzone_val.to_string()),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .id("btn-deadzone-inc")
                                                                        .cursor_pointer()
                                                                        .size(px(20.0))
                                                                        .rounded_sm()
                                                                        .bg(theme::bg_active())
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(theme::text_primary())
                                                                        .hover(|s| s.bg(theme::bg_hover()))
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.app_settings.gamepad_deadzone = (this.app_settings.gamepad_deadzone + 1000).min(30000);
                                                                            let _ = this.app_settings.save();
                                                                            cx.notify();
                                                                        }))
                                                                        .child("+"),
                                                                ),
                                                        ),
                                                ),
                                        ),
                                )
                                // Column 2: Shortcuts List (remappable, matching media_1793137385955_044e590e.png center box)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(px(260.0))
                                        .flex_shrink_0()
                                        .p_3()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme::border_color())
                                        .bg(theme::bg_panel())
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_muted())
                                                .child("Shortcuts (Click to Remap)"),
                                        )
                                        .child(self.render_shortcut_row("unfocus", "Toggle Unfocus :", is_ps, cx))
                                        .child(self.render_shortcut_row("close", "Close Nucleus :", is_ps, cx))
                                        .child(self.render_shortcut_row("stop", "Stop Session :", is_ps, cx))
                                        .child(self.render_shortcut_row("topmost", "Toggle Top Most :", is_ps, cx))
                                        .child(self.render_shortcut_row("reset_windows", "Reset Windows :", is_ps, cx))
                                        .child(self.render_shortcut_row("cutscenes", "Cutscenes Mode :", is_ps, cx))
                                        .child(self.render_shortcut_row("switch_layout", "Switch Layouts :", is_ps, cx))
                                        .child(self.render_shortcut_row("lock_inputs", "Lock Input Key :", is_ps, cx))
                                        .child(self.render_shortcut_row("release_cursor", "ALT+TAB :", is_ps, cx))
                                        .child(self.render_shortcut_row("lock_ui", "Lock UI Control :", is_ps, cx)),
                                )
                                // Column 3: Red warning notice, Remap Palette & Controller Silhouette Graphic
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_between()
                                        .p_3()
                                        .gap_3()
                                        // Warning Notice
                                        .child(
                                            div()
                                                .w_full()
                                                .p_2p5()
                                                .rounded_md()
                                                .bg(theme::danger_subtle())
                                                .border_1()
                                                .border_color(theme::danger())
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::danger())
                                                .text_center()
                                                .child("Warning: Steam Input may interfere with the controller shortcuts."),
                                        )
                                        // Interactive Remapping Palette (shown when an action is clicked for remapping)
                                        .when_some(self.remapping_controller_shortcut.clone(), |el, active_action| {
                                            let act_name = active_action.clone();
                                            el.child(
                                                div()
                                                    .w_full()
                                                    .p_2p5()
                                                    .rounded_md()
                                                    .bg(rgb(0x181820))
                                                    .border_1()
                                                    .border_color(theme::accent())
                                                    .flex()
                                                    .flex_col()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .justify_between()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(theme::accent())
                                                                    .child(format!("Select replacement button for '{}':", act_name)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .id("btn-cancel-shortcut-remap")
                                                                    .cursor_pointer()
                                                                    .px_2()
                                                                    .py_0p5()
                                                                    .rounded_md()
                                                                    .bg(theme::bg_active())
                                                                    .text_xs()
                                                                    .text_color(theme::text_muted())
                                                                    .hover(|s| s.text_color(theme::text_primary()))
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.remapping_controller_shortcut = None;
                                                                        cx.notify();
                                                                    }))
                                                                    .child("Cancel"),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_wrap()
                                                            .gap_1p5()
                                                            .children(
                                                                [
                                                                    ("A", if is_ps { "✕" } else { "A" }),
                                                                    ("B", if is_ps { "◯" } else { "B" }),
                                                                    ("X", if is_ps { "□" } else { "X" }),
                                                                    ("Y", if is_ps { "△" } else { "Y" }),
                                                                    ("LB", if is_ps { "L1" } else { "LB" }),
                                                                    ("RB", if is_ps { "R1" } else { "RB" }),
                                                                    ("LT", if is_ps { "L2" } else { "LT" }),
                                                                    ("RT", if is_ps { "R2" } else { "RT" }),
                                                                    ("LS", if is_ps { "L3" } else { "LS" }),
                                                                    ("RS", if is_ps { "R3" } else { "RS" }),
                                                                    ("Start", if is_ps { "Opt" } else { "≡" }),
                                                                    ("Back", if is_ps { "Shr" } else { "⧉" }),
                                                                    ("DPadUp", "▲"),
                                                                    ("DPadDown", "▼"),
                                                                    ("DPadLeft", "◀"),
                                                                    ("DPadRight", "▶"),
                                                                ]
                                                                .into_iter()
                                                                .map(|(code, glyph)| {
                                                                    let code_str = code.to_string();
                                                                    let target_act = act_name.clone();
                                                                    div()
                                                                        .id(ElementId::Name(format!("remap-pick-{}", code).into()))
                                                                        .cursor_pointer()
                                                                        .px_2()
                                                                        .py_1()
                                                                        .rounded_md()
                                                                        .bg(rgb(0x24242c))
                                                                        .border_1()
                                                                        .border_color(rgb(0x3a3a46))
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(theme::text_primary())
                                                                        .hover(|s| s.bg(theme::accent()).border_color(theme::accent()).text_color(theme::text_on_accent()))
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            this.remap_controller_action(&target_act, &code_str, cx);
                                                                        }))
                                                                        .child(glyph)
                                                                }),
                                                            ),
                                                    ),
                                            )
                                        })
                                        // Stylized Controller Silhouette Layout with clickable hot zones
                                        .child(
                                            div()
                                                .flex_1()
                                                .w_full()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .justify_center()
                                                .gap_3()
                                                // Controller Top Bumpers/Triggers Graphic
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .w(px(200.0))
                                                        .px_3()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(rgb(0x44444c))
                                                        .bg(rgb(0x1a1a1e))
                                                        .child({
                                                            let act_opt = self.remapping_controller_shortcut.clone();
                                                            div()
                                                                .id("sil-top-left")
                                                                .cursor_pointer()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::accent())
                                                                .hover(|s| s.text_color(rgb(0xffffff)))
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    if let Some(ref a) = act_opt {
                                                                        this.remap_controller_action(a, "LB", cx);
                                                                    }
                                                                }))
                                                                .child(if is_ps { "L1 / L2" } else { "LB / LT" })
                                                        })
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme::text_muted())
                                                                .child("TOP BUMPERS"),
                                                        )
                                                        .child({
                                                            let act_opt = self.remapping_controller_shortcut.clone();
                                                            div()
                                                                .id("sil-top-right")
                                                                .cursor_pointer()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::accent())
                                                                .hover(|s| s.text_color(rgb(0xffffff)))
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    if let Some(ref a) = act_opt {
                                                                        this.remap_controller_action(a, "RB", cx);
                                                                    }
                                                                }))
                                                                .child(if is_ps { "R1 / R2" } else { "RB / RT" })
                                                        }),
                                                )
                                                // Controller Main Face Graphic
                                                .child(
                                                    div()
                                                        .w(px(240.0))
                                                        .h(px(130.0))
                                                        .rounded_2xl()
                                                        .border_2()
                                                        .border_color(rgb(0x555562))
                                                        .bg(rgb(0x1e1e24))
                                                        .p_3()
                                                        .flex()
                                                        .flex_col()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                // Left Stick
                                                                .child({
                                                                    let act_opt = self.remapping_controller_shortcut.clone();
                                                                    div()
                                                                        .id("sil-ls")
                                                                        .cursor_pointer()
                                                                        .size(px(32.0))
                                                                        .rounded_full()
                                                                        .border_1()
                                                                        .border_color(rgb(0x666675))
                                                                        .bg(rgb(0x2a2a33))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .text_color(theme::text_muted())
                                                                        .hover(|s| s.bg(theme::accent()).text_color(theme::text_on_accent()))
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            if let Some(ref a) = act_opt {
                                                                                this.remap_controller_action(a, "LS", cx);
                                                                            }
                                                                        }))
                                                                        .child("LS")
                                                                })
                                                                // Center Guide Button
                                                                .child({
                                                                    let act_opt = self.remapping_controller_shortcut.clone();
                                                                    div()
                                                                        .id("sil-guide")
                                                                        .cursor_pointer()
                                                                        .size(px(28.0))
                                                                        .rounded_full()
                                                                        .bg(rgb(0xffffff))
                                                                        .text_color(rgb(0x000000))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            if let Some(ref a) = act_opt {
                                                                                this.remap_controller_action(a, "Guide", cx);
                                                                            }
                                                                        }))
                                                                        .child(if is_ps { "🅟" } else { "⨂" })
                                                                })
                                                                // Face Buttons Cluster
                                                                .child({
                                                                    let act_opt = self.remapping_controller_shortcut.clone();
                                                                    div()
                                                                        .id("sil-face")
                                                                        .cursor_pointer()
                                                                        .size(px(34.0))
                                                                        .rounded_full()
                                                                        .border_1()
                                                                        .border_color(rgb(0x666675))
                                                                        .bg(rgb(0x2a2a33))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_color(theme::accent())
                                                                        .hover(|s| s.bg(theme::accent()).text_color(theme::text_on_accent()))
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            if let Some(ref a) = act_opt {
                                                                                this.remap_controller_action(a, "A", cx);
                                                                            }
                                                                        }))
                                                                        .child(if is_ps { "△ ◯ □ ✕" } else { "Y B X A" })
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .px_4()
                                                                // DPad
                                                                .child({
                                                                    let act_opt = self.remapping_controller_shortcut.clone();
                                                                    div()
                                                                        .id("sil-dpad")
                                                                        .cursor_pointer()
                                                                        .size(px(28.0))
                                                                        .rounded_md()
                                                                        .bg(rgb(0x2a2a33))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .text_color(theme::text_muted())
                                                                        .hover(|s| s.bg(theme::accent()).text_color(theme::text_on_accent()))
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            if let Some(ref a) = act_opt {
                                                                                this.remap_controller_action(a, "DPadUp", cx);
                                                                            }
                                                                        }))
                                                                        .child("✚")
                                                                })
                                                                // Right Stick
                                                                .child({
                                                                    let act_opt = self.remapping_controller_shortcut.clone();
                                                                    div()
                                                                        .id("sil-rs")
                                                                        .cursor_pointer()
                                                                        .size(px(32.0))
                                                                        .rounded_full()
                                                                        .border_1()
                                                                        .border_color(rgb(0x666675))
                                                                        .bg(rgb(0x2a2a33))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .text_xs()
                                                                        .text_color(theme::text_muted())
                                                                        .hover(|s| s.bg(theme::accent()).text_color(theme::text_on_accent()))
                                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                                            if let Some(ref a) = act_opt {
                                                                                this.remap_controller_action(a, "RS", cx);
                                                                            }
                                                                        }))
                                                                        .child("RS")
                                                                }),
                                                        ),
                                                ),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Renders active interactive launch countdown prompt modal if present.
    pub(super) fn render_launch_prompt_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let req = self.active_launch_prompt.as_ref()?;
        let player_idx = req.player_index;
        let count = req.player_count;
        let msg = req.message.clone();
        let seconds = req.auto_proceed_seconds;

        let modal = crate::nefos::NefosLaunchPromptModal::new(player_idx, count, msg)
            .countdown(seconds)
            .on_proceed(cx.listener(|this, _, _, cx| {
                if let Some(req) = this.active_launch_prompt.take() {
                    let _ = req.response_tx.send(true);
                }
                cx.notify();
            }))
            .on_cancel(cx.listener(|this, _, _, cx| {
                if let Some(req) = this.active_launch_prompt.take() {
                    let _ = req.response_tx.send(false);
                }
                cx.notify();
            }));

        Some(modal.into_any_element())
    }

    /// Renders dismissible pre-flight diagnostics alert banner if errors or warnings exist.
    pub(super) fn render_preflight_banners(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(ref danger) = self.preflight_danger {
            let msg = danger.clone();
            Some(
                crate::nefos::NefosBanner::new("preflight-danger-banner", msg)
                    .tone(crate::nefos::NefosBannerTone::Danger)
                    .title("Critical Environment Health Alert")
                    .on_dismiss(cx.listener(|this, _, _, cx| {
                        this.preflight_danger = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        } else if let Some(ref warn) = self.preflight_warning {
            let msg = warn.clone();
            Some(
                crate::nefos::NefosBanner::new("preflight-warning-banner", msg)
                    .tone(crate::nefos::NefosBannerTone::Warning)
                    .title("System Compatibility Notice")
                    .on_dismiss(cx.listener(|this, _, _, cx| {
                        this.preflight_warning = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        } else {
            None
        }
    }

    /// Renders the Hub Account authentication modal if currently open.
    pub fn render_hub_account_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.hub_account_modal.is_open {
            return None;
        }
        let on_close = cx.listener(|this: &mut MainWindow, _, _, cx| {
            this.hub_account_modal.close();
            cx.notify();
        });
        Some(crate::hub_account_modal::render_hub_account_modal(
            &self.hub_account_modal,
            self.hub_account_summary.as_ref(),
            self.callbacks.on_hub_login.clone(),
            self.callbacks.on_hub_logout.clone(),
            Arc::new(on_close),
        ))
    }

    /// Renders the multi-instance save slot manager modal if currently open.
    pub fn render_save_slots_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.save_slots_modal.is_open {
            return None;
        }
        let active_slot = self
            .player_save_slots
            .get(self.save_slots_modal.target_player_index)
            .and_then(|s| s.as_deref());
        let on_close = cx.listener(|this: &mut MainWindow, _, _, cx| {
            this.save_slots_modal.close();
            cx.notify();
        });

        let selected_slot_cell = Arc::new(std::sync::Mutex::new(None::<Option<String>>));
        let cell_for_cb = selected_slot_cell.clone();
        let target_p = self.save_slots_modal.target_player_index;
        let set_slot_cb = self.callbacks.on_set_player_save_slot.clone();
        let on_select_slot = set_slot_cb.map(move |cb| {
            let c = cb.clone();
            let cell = cell_for_cb.clone();
            Arc::new(move |g: &str, p: usize, opt: Option<String>| {
                if let Ok(mut lock) = cell.lock() {
                    *lock = Some(opt.clone());
                }
                c(g, p, opt);
            }) as Arc<dyn Fn(&str, usize, Option<String>) + Send + Sync>
        });

        let cell_for_refresh = selected_slot_cell.clone();
        let on_refresh = cx.listener(move |this: &mut MainWindow, _, _, cx| {
            let guid = this.save_slots_modal.game_guid.clone();
            if let Some(ref lister) = this.callbacks.on_list_save_slots {
                this.current_save_slots = lister(&guid);
            }
            if let Ok(mut lock) = cell_for_refresh.lock() {
                if let Some(new_opt) = lock.take() {
                    if target_p < this.player_save_slots.len() {
                        this.player_save_slots[target_p] = new_opt;
                    } else {
                        this.player_save_slots.resize(target_p + 1, None);
                        this.player_save_slots[target_p] = new_opt;
                    }
                }
            }
            cx.notify();
        });
        Some(crate::save_slots_modal::render_save_slots_modal(
            &self.save_slots_modal,
            &self.current_save_slots,
            active_slot,
            on_select_slot,
            self.callbacks.on_create_save_slot.as_ref().map(|cb| {
                let c = cb.clone();
                Arc::new(move |g: &str, d: &str, t: &str, h: &str| {
                    let _ = c(g, d, t, h);
                }) as Arc<dyn Fn(&str, &str, &str, &str) + Send + Sync>
            }),
            self.callbacks.on_clone_save_slot.as_ref().map(|cb| {
                let c = cb.clone();
                Arc::new(move |g: &str, s: &str, n: &str| {
                    let _ = c(g, s, n);
                }) as Arc<dyn Fn(&str, &str, &str) + Send + Sync>
            }),
            self.callbacks.on_delete_save_slot.as_ref().map(|cb| {
                let c = cb.clone();
                Arc::new(move |g: &str, s: &str| {
                    let _ = c(g, s);
                }) as Arc<dyn Fn(&str, &str) + Send + Sync>
            }),
            Arc::new(on_close),
            Some(Arc::new(on_refresh)),
        ))
    }
}

/// Formats a controller button code into a display glyph based on Xbox vs PlayStation style.
pub fn format_gamepad_button_glyph(button_code: &str, is_ps: bool) -> &'static str {
    match button_code {
        "A" => {
            if is_ps {
                "✕"
            } else {
                "A"
            }
        }
        "B" => {
            if is_ps {
                "◯"
            } else {
                "B"
            }
        }
        "X" => {
            if is_ps {
                "□"
            } else {
                "X"
            }
        }
        "Y" => {
            if is_ps {
                "△"
            } else {
                "Y"
            }
        }
        "LB" => {
            if is_ps {
                "L1"
            } else {
                "LB"
            }
        }
        "RB" => {
            if is_ps {
                "R1"
            } else {
                "RB"
            }
        }
        "LT" => {
            if is_ps {
                "L2"
            } else {
                "LT"
            }
        }
        "RT" => {
            if is_ps {
                "R2"
            } else {
                "RT"
            }
        }
        "LS" | "L3" => {
            if is_ps {
                "L3"
            } else {
                "LS"
            }
        }
        "RS" | "R3" => {
            if is_ps {
                "R3"
            } else {
                "RS"
            }
        }
        "Start" | "Menu" => {
            if is_ps {
                "Opt"
            } else {
                "≡"
            }
        }
        "Back" | "View" => {
            if is_ps {
                "Shr"
            } else {
                "⧉"
            }
        }
        "DPadUp" | "Up" => "▲",
        "DPadDown" | "Down" => "▼",
        "DPadLeft" | "Left" => "◀",
        "DPadRight" | "Right" => "▶",
        "Guide" | "Home" => {
            if is_ps {
                "🅟"
            } else {
                "⨂"
            }
        }
        _ => "•",
    }
}
