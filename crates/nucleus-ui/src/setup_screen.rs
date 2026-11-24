//! Player setup and screen split assignment view.
//!
//! Visual interface for allocating connected monitors, selecting player counts,
//! and assigning devices to screen partitions using the Nefos UI meta-framework.

use std::collections::HashMap;

use gpui::prelude::*;
use gpui::*;
use gpui_component::scroll::ScrollableElement;
use gpui_kit_assets::IconName;

use crate::models::{DeviceItem, HandlerItem};
use crate::nefos::*;
use crate::theme;
use crate::window::MainWindow;

/// Setup screen view coordinator for player-to-monitor assignments.
#[derive(Debug, Clone, Default)]
pub struct SetupScreenView {
    /// Number of active player slots configured.
    pub player_count: usize,
    /// Identifier of the game currently being configured.
    pub game_id: Option<String>,
    /// Index of slot currently grabbed for drag-to-merge / swap in desktop mode.
    pub grabbed_slot: Option<usize>,
    /// Whether the custom options modal is currently displayed.
    pub options_modal_open: bool,
    /// Active tab in custom options modal (0 = Game Options, 1 = ProtoInput & Hooks).
    pub options_modal_tab: usize,
    /// Currently expanded audio dropdown player index, if any.
    pub dropdown_audio_instance: Option<usize>,
    /// Currently open combobox option key, if any.
    pub combobox_open_key: Option<String>,
    /// Search / filter draft string for active combobox.
    pub combobox_search_draft: String,
    /// Validation error message for active combobox, if any.
    pub combobox_validation_error: Option<String>,
}

impl SetupScreenView {
    /// Creates a new `SetupScreenView`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the number of players participating in the split session.
    pub fn set_player_count(&mut self, count: usize) {
        self.player_count = count;
    }

    /// Toggles the combobox for the given option key open or closed.
    pub fn toggle_combobox(&mut self, key: &str) {
        if self.combobox_open_key.as_deref() == Some(key) {
            self.combobox_open_key = None;
            self.combobox_search_draft.clear();
            self.combobox_validation_error = None;
        } else {
            self.combobox_open_key = Some(key.to_string());
            self.combobox_search_draft.clear();
            self.combobox_validation_error = None;
        }
    }

    /// Closes any open combobox and resets draft and validation error.
    pub fn close_combobox(&mut self) {
        self.combobox_open_key = None;
        self.combobox_search_draft.clear();
        self.combobox_validation_error = None;
    }
}

/// Helper that renders preview boxes corresponding to the partitioned screen.
pub fn render_screen_partition_preview(players: usize, layout: &str) -> AnyElement {
    let lower = layout.to_lowercase();
    let split = if players == 1 || lower.contains("1x1") || lower.contains("single") {
        NefosSplitLayout::Single
    } else if lower.contains("stacked") || lower.contains("1x2") {
        NefosSplitLayout::Stacked
    } else if lower.contains("top 1 / bottom 2") || (players == 3 && !lower.contains("top 2")) {
        NefosSplitLayout::TopOneBottomTwo
    } else if lower.contains("top 2 / bottom 1") {
        NefosSplitLayout::TopTwoBottomOne
    } else if lower.contains("3x2")
        || lower.contains("2x3")
        || lower.contains("grid 6")
        || lower.contains("grid6")
        || players == 6
    {
        NefosSplitLayout::Grid6
    } else if lower.contains("4x2")
        || lower.contains("2x4")
        || lower.contains("grid 8")
        || lower.contains("grid8")
        || players == 8
    {
        NefosSplitLayout::Grid8
    } else if lower.contains("4x4")
        || lower.contains("grid 16")
        || lower.contains("grid16")
        || players > 8
    {
        NefosSplitLayout::Grid16
    } else if lower.contains("multi-monitor")
        || lower.contains("multimonitor")
        || lower.contains("multi monitor")
    {
        NefosSplitLayout::MultiMonitor { monitor_count: 2 }
    } else if players == 4 || lower.contains("grid") || lower.contains("2x2") {
        NefosSplitLayout::Grid4
    } else {
        NefosSplitLayout::SideBySide
    };

    NefosPartitionPreview::new(split)
        .size(px(460.0), px(160.0))
        .player_count(players)
        .into_any_element()
}

/// Helper to test if a device string represents a composite merged keyboard + mouse slot.
fn is_merged_kbm_str(s: &str) -> bool {
    (s.contains("Keyboard") && s.contains("Mouse"))
        || (s.contains("K#") && s.contains("M#"))
        || s.contains(" + ")
}

/// Helper to test if a device string represents an individual raw keyboard.
fn is_raw_keyboard_str(s: &str) -> bool {
    (s.contains("Keyboard") || s.contains("K#")) && !s.contains("Mouse") && !s.contains("M#")
}

/// Helper to test if a device string represents an individual raw mouse.
fn is_raw_mouse_str(s: &str) -> bool {
    (s.contains("Mouse") || s.contains("M#")) && !s.contains("Keyboard") && !s.contains("K#")
}

/// Helper to test if a device string represents any keyboard or mouse device.
fn is_kbm_str(s: &str) -> bool {
    s.contains("Keyboard")
        || s.contains("Mouse")
        || s.contains("K#")
        || s.contains("M#")
        || s.contains("KBM")
}

/// Helper to test if an option values list represents a boolean switch.
fn is_boolean_option(values: &[String]) -> bool {
    if values.len() != 2 {
        return false;
    }
    let v0 = values[0].to_lowercase();
    let v1 = values[1].to_lowercase();
    (v0 == "true" && v1 == "false")
        || (v0 == "false" && v1 == "true")
        || (v0 == "yes" && v1 == "no")
        || (v0 == "no" && v1 == "yes")
        || (v0 == "enabled" && v1 == "disabled")
        || (v0 == "disabled" && v1 == "enabled")
        || (v0 == "on" && v1 == "off")
        || (v0 == "off" && v1 == "on")
}

/// Helper to parse boolean truthiness from an option value string.
fn parse_boolean_value(val: &str) -> bool {
    matches!(
        val.to_lowercase().as_str(),
        "true" | "yes" | "enabled" | "on" | "1"
    )
}

/// Audio output devices available for per-instance routing.
const AUDIO_ENDPOINTS: [&str; 6] = [
    "Default",
    "Speakers (Realtek High Definition Audio)",
    "Headphones (Realtek High Definition Audio)",
    "Digital Audio (S/PDIF)",
    "CABLE Input (VB-Audio Virtual Cable)",
    "Steam Streaming Speakers",
];

/// Helper to render the complete Game Profile Management section using `NefosCard` & `NefosButton`.
fn render_profile_management_section(
    selected_handler: Option<&HandlerItem>,
    selected_game: Option<&str>,
    active_profile_id: Option<usize>,
    saved_profiles: &[(usize, String)],
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let guid = selected_handler
        .map(|h| h.guid.clone())
        .or_else(|| selected_game.map(|s| s.to_string()))
        .unwrap_or_default();
    let active_id = active_profile_id;
    let profiles = saved_profiles;

    let active_badge = if let Some(id) = active_id {
        NefosTag::new(format!("Active: Profile {}", id))
            .tone(NefosTagTone::Accent)
            .into_any_element()
    } else {
        NefosTag::new("Active: [Default Setup]")
            .tone(NefosTagTone::Neutral)
            .into_any_element()
    };

    let header_actions = div()
        .flex()
        .items_center()
        .gap_2()
        .when(active_id.is_some(), |bar| {
            bar.child(
                NefosButton::danger("btn-unload-profile", "Unload Profile")
                    .size(NefosButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active_profile_id = None;
                        if let Some(ref unloader) = this.callbacks.on_unload_profile {
                            unloader();
                        }
                        this.status_message =
                            "Handler profile unloaded. Using default setup.".to_string();
                        cx.notify();
                    })),
            )
        })
        .child({
            let g_save = guid.clone();
            NefosButton::primary("btn-save-new-profile", "Save New Profile")
                .icon(IconName::Plus)
                .size(NefosButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.app_settings.disable_game_profiles
                        || this
                            .handlers
                            .iter()
                            .find(|h| h.guid == g_save || h.game_name == g_save)
                            .map_or(false, |h| h.disable_profile)
                    {
                        this.status_message = "Profiles are disabled for this game.".to_string();
                        cx.notify();
                        return;
                    }
                    let mut prof = nucleus_profile::GameProfile::default();
                    prof.player_count = this.player_count;
                    prof.layout = this.selected_layout.clone();
                    prof.player_device_assignments = this.player_device_assignments.clone();
                    prof.options = this.selected_handler_options.clone();
                    if let Some(ref saver) = this.callbacks.on_save_numbered_profile {
                        if let Some(new_id) = saver(&g_save, None, &prof) {
                            this.active_profile_id = Some(new_id);
                            this.refresh_saved_profiles();
                            this.status_message =
                                format!("Saved new profile (Profile {})!", new_id);
                        }
                    }
                    cx.notify();
                }))
        })
        .when(active_id.is_some(), |bar| {
            let g_save = guid.clone();
            let cur_id = active_id.unwrap();
            bar.child(
                NefosButton::secondary(
                    "btn-update-active-profile",
                    format!("Update Profile {}", cur_id),
                )
                .icon(IconName::Save)
                .size(NefosButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.app_settings.disable_game_profiles
                        || this
                            .handlers
                            .iter()
                            .find(|h| h.guid == g_save || h.game_name == g_save)
                            .map_or(false, |h| h.disable_profile)
                    {
                        this.status_message = "Profiles are disabled for this game.".to_string();
                        cx.notify();
                        return;
                    }
                    let mut prof = nucleus_profile::GameProfile::default();
                    prof.player_count = this.player_count;
                    prof.layout = this.selected_layout.clone();
                    prof.player_device_assignments = this.player_device_assignments.clone();
                    prof.options = this.selected_handler_options.clone();
                    if let Some(ref saver) = this.callbacks.on_save_numbered_profile {
                        saver(&g_save, Some(cur_id), &prof);
                        this.refresh_saved_profiles();
                        this.status_message = format!("Updated Profile {}!", cur_id);
                    }
                    cx.notify();
                })),
            )
        });

    let profiles_body = div()
        .flex()
        .flex_wrap()
        .gap_2()
        .when(profiles.is_empty(), |row| {
            row.child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child("No saved profiles yet. Customize settings below and click 'Save New Profile'."),
            )
        })
        .children(profiles.iter().map(|(prof_id, title)| {
            let prof_id = *prof_id;
            let g_click = guid.clone();
            let is_active = active_id == Some(prof_id);
            div()
                .id(("profile-chip-container", prof_id))
                .flex()
                .items_center()
                .gap_1p5()
                .child(
                    div()
                        .id(("profile-chip-select", prof_id))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(ref loader) = this.callbacks.on_load_numbered_profile {
                                if let Some(prof) = loader(&g_click, prof_id) {
                                    this.player_count = prof.player_count;
                                    this.setup_screen.set_player_count(prof.player_count);
                                    this.selected_layout = prof.layout.clone();
                                    this.couch_setup_state.selected_layout = prof.layout.clone();
                                    if !prof.player_device_assignments.is_empty() {
                                        this.player_device_assignments =
                                            prof.player_device_assignments.clone();
                                    } else {
                                        this.sync_player_assignments();
                                    }
                                    this.selected_handler_options = prof.options.clone();
                                    this.active_profile_id = Some(prof_id);
                                    this.status_message = format!("Loaded Profile {}!", prof_id);
                                    cx.notify();
                                }
                            }
                        }))
                        .child(
                            NefosTag::new(title.clone())
                                .tone(if is_active {
                                    NefosTagTone::Accent
                                } else {
                                    NefosTagTone::Neutral
                                }),
                        ),
                )
                .child({
                    let g_del = guid.clone();
                    NefosButton::danger(("del-profile-btn", prof_id), "")
                        .icon(IconName::X)
                        .size(NefosButtonSize::Sm)
                        .tooltip("Delete this saved profile configuration")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.confirm_dialog = Some(crate::ConfirmDialogState {
                                title: format!("Delete Profile #{}?", prof_id),
                                message: "Are you sure you want to permanently delete this saved profile configuration?".to_string(),
                                confirm_label: "Delete Profile".to_string(),
                                is_destructive: true,
                                action: crate::ConfirmAction::DeleteNumberedProfile(g_del.clone(), prof_id),
                            });
                            cx.notify();
                        }))
                })
        }));

    NefosCard::surface("setup-profile-card")
        .title(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child("Game Profile Management")
                .child(active_badge),
        )
        .description_text(
            "Save and load custom per-game player setups, layout presets, and option configs",
        )
        .header_action(header_actions)
        .child(profiles_body)
        .into_any_element()
}

/// Helper to render the Unassigned Controllers Tray.
fn render_unassigned_tray(
    detected_devices: &[DeviceItem],
    player_device_assignments: &[String],
    cx: &mut Context<MainWindow>,
) -> Option<AnyElement> {
    let unassigned_devices: Vec<&DeviceItem> = detected_devices
        .iter()
        .filter(|d| {
            let id_str = format!("Gamepad #{}", d.id);
            let virt_str = format!("Virtual Gamepad #{}", d.id);
            let virt_ctrl = format!("Virtual Controller #{}", d.id);
            !player_device_assignments.iter().any(|a| {
                a.starts_with(&id_str) || a.starts_with(&virt_str) || a.starts_with(&virt_ctrl)
            })
        })
        .collect();

    if unassigned_devices.is_empty() {
        return None;
    }

    let tray_body =
        div()
            .flex()
            .flex_wrap()
            .gap_3()
            .children(unassigned_devices.into_iter().enumerate().map(|(_i, dev)| {
                let dev_id = dev.id;
                let dev_name = dev.name.clone();
                let is_virt = dev.is_virtual;

                div()
                    .id(("tray-item", dev_id))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(theme::bg_subtle())
                    .border_1()
                    .border_color(theme::border_subtle())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                gpui_component::Icon::new(IconName::Gamepad2)
                                    .size(px(18.0))
                                    .text_color(theme::text_muted()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::text_primary())
                                            .child(dev_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child(format!("ID: #{} | {}", dev_id, dev.guid)),
                                    ),
                            ),
                    )
                    .child(
                        NefosButton::primary(("btn-reclaim-tray", dev_id), "+ Assign")
                            .size(NefosButtonSize::Sm)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let new_assign_str = if is_virt {
                                    format!("Virtual Gamepad #{} ({})", dev_id, dev_name)
                                } else {
                                    format!("Gamepad #{} ({})", dev_id, dev_name)
                                };

                                if let Some(pos) = this
                                    .player_device_assignments
                                    .iter()
                                    .position(|a| a.starts_with("Unassigned"))
                                {
                                    this.player_device_assignments[pos] = new_assign_str;
                                } else if this.player_count < 8 {
                                    this.player_count += 1;
                                    this.setup_screen.set_player_count(this.player_count);
                                    this.player_device_assignments.push(new_assign_str);
                                }
                                this.status_message =
                                    format!("Assigned device #{} to player slot.", dev_id);
                                cx.notify();
                            })),
                    )
            }));

    Some(
        NefosCard::surface("setup-unassigned-tray-card")
            .title_text("Unassigned Controllers Tray")
            .description_text("Connected gamepads available for instant player allocation")
            .child(tray_body)
            .into_any_element(),
    )
}

/// Helper to render the Per-Instance Audio Endpoint Routing card.
fn render_audio_routing_section(
    player_count: usize,
    dropdown_audio_instance: Option<usize>,
    app_settings: &nucleus_profile::AppSettings,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let rows = div()
        .flex()
        .flex_col()
        .gap_2p5()
        .children((0..player_count).map(|p_idx| {
            let is_dropdown_open = dropdown_audio_instance == Some(p_idx);
            let cur_dev = app_settings.get_audio_instance(p_idx).to_string();

            div()
                .id(("setup-audio-inst", p_idx))
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .bg(theme::bg_subtle())
                .rounded_lg()
                .border_1()
                .border_color(if is_dropdown_open {
                    theme::accent()
                } else {
                    theme::border_subtle()
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2p5()
                                .child(
                                    NefosTag::player(p_idx, format!("Player {}", p_idx + 1))
                                        .into_any_element(),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::text_primary())
                                        .child(format!("Audio Output (Instance #{})", p_idx + 1)),
                                ),
                        )
                        .child(
                            div()
                                .id(("btn-setup-audio-select", p_idx))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(if is_dropdown_open {
                                    theme::accent()
                                } else {
                                    theme::border_color()
                                })
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text_primary())
                                .hover(|s| s.bg(theme::bg_hover()))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.setup_screen.dropdown_audio_instance =
                                        if this.setup_screen.dropdown_audio_instance == Some(p_idx)
                                        {
                                            None
                                        } else {
                                            Some(p_idx)
                                        };
                                    cx.notify();
                                }))
                                .child(cur_dev.clone())
                                .child(
                                    div()
                                        .text_color(theme::accent())
                                        .child(if is_dropdown_open { "▲" } else { "▼" }),
                                ),
                        ),
                )
                .when(is_dropdown_open, |slot| {
                    let cur_active = cur_dev.clone();
                    slot.child(
                        div()
                            .flex()
                            .flex_col()
                            .p_2()
                            .bg(theme::bg_panel())
                            .rounded_md()
                            .border_1()
                            .border_color(theme::accent().opacity(0.4))
                            .gap_1()
                            .children(AUDIO_ENDPOINTS.into_iter().enumerate().map(
                                |(d_idx, d_name)| {
                                    let is_cur = cur_active == d_name;
                                    let dev_str = d_name.to_string();
                                    div()
                                        .id(("setup-audio-opt", p_idx * 100 + d_idx))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .px_2p5()
                                        .py_1p5()
                                        .rounded_md()
                                        .text_xs()
                                        .when(is_cur, |s| {
                                            s.bg(theme::accent().opacity(0.15))
                                                .text_color(theme::accent())
                                                .font_weight(FontWeight::BOLD)
                                        })
                                        .when(!is_cur, |s| {
                                            s.text_color(theme::text_primary())
                                                .hover(|h| h.bg(theme::bg_hover()))
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.app_settings.set_audio_instance(p_idx, &dev_str);
                                            this.setup_screen.dropdown_audio_instance = None;
                                            this.status_message = format!(
                                                "Instance {} audio routed to {}",
                                                p_idx + 1,
                                                dev_str
                                            );
                                            cx.notify();
                                        }))
                                        .child(d_name)
                                        .children(if is_cur {
                                            Some(
                                                gpui_component::Icon::new(IconName::Check)
                                                    .size(px(14.0))
                                                    .text_color(theme::accent()),
                                            )
                                        } else {
                                            None
                                        })
                                },
                            )),
                    )
                })
        }));

    NefosCard::surface("setup-audio-card")
        .title_text("Per-Instance Audio Endpoint Routing")
        .description_text("Direct each player instance audio to separate physical headphones, speakers, or virtual cables")
        .child(rows)
        .into_any_element()
}

/// Helper that renders the Handler Custom Options dialog modal using `NefosModal`.
fn render_handler_options_modal(
    game_title: &str,
    handler: &HandlerItem,
    selected_options: &HashMap<String, String>,
    setup_state: &SetupScreenView,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let options = &handler.options;
    let supports_proto = handler.supports_raw_input || handler.supports_multiple_keyboards_and_mice;
    let active_tab = setup_state.options_modal_tab;

    let modal_tabs = if supports_proto {
        Some(
            NefosSegmentedControl::new("modal-options-tabs")
                .item(
                    "Game Options",
                    Some(IconName::SlidersHorizontal),
                    active_tab == 0,
                )
                .item("ProtoInput & Hooks", Some(IconName::Zap), active_tab == 1)
                .on_select(move |this, idx, _window, cx| {
                    this.setup_screen.options_modal_tab = idx;
                    this.setup_screen.close_combobox();
                    cx.notify();
                })
                .into_any_element(),
        )
    } else {
        None
    };

    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .children(options.iter().enumerate().map(|(opt_idx, opt)| {
            let opt_key = opt.key.clone();
            let opt_name = opt.name.clone();
            let opt_desc = opt.description.clone();
            let opt_values = opt.values.clone();

            let current_val = selected_options.get(&opt_key).cloned().unwrap_or_else(|| {
                opt_values
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Default".to_string())
            });

            if is_boolean_option(&opt_values) {
                let is_enabled = parse_boolean_value(&current_val);
                let opt_key_click = opt_key.clone();
                let vals_toggle = opt_values.clone();

                NefosToggle::new(("modal-opt-toggle", opt_idx), opt_name, is_enabled)
                    .description(opt_desc)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let next_val = if is_enabled {
                            vals_toggle
                                .iter()
                                .find(|v| !parse_boolean_value(v))
                                .cloned()
                                .unwrap_or_else(|| "false".to_string())
                        } else {
                            vals_toggle
                                .iter()
                                .find(|v| parse_boolean_value(v))
                                .cloned()
                                .unwrap_or_else(|| "true".to_string())
                        };
                        this.selected_handler_options
                            .insert(opt_key_click.clone(), next_val);
                        cx.notify();
                    }))
                    .into_any_element()
            } else if should_use_combobox(&opt_values) {
                let is_open = setup_state.combobox_open_key.as_deref() == Some(opt_key.as_str());
                let draft = if is_open {
                    setup_state.combobox_search_draft.clone()
                } else {
                    String::new()
                };
                let validation_error = if is_open {
                    setup_state.combobox_validation_error.clone()
                } else {
                    None
                };

                let opt_key_toggle = opt_key.clone();
                let opt_key_select = opt_key.clone();
                let opt_key_key = opt_key.clone();
                let vals_select = opt_values.clone();
                let vals_key = opt_values.clone();
                let entity = cx.entity().clone();

                NefosOptionCombobox::new(
                    ("modal-opt-combobox", opt_idx),
                    opt_name,
                    opt_desc,
                    current_val,
                    opt_values.clone(),
                )
                .is_open(is_open)
                .search_draft(draft)
                .validation_error(validation_error)
                .on_toggle(cx.listener(move |this, _, _, cx| {
                    this.setup_screen.toggle_combobox(&opt_key_toggle);
                    cx.notify();
                }))
                .on_select(move |selected_val, _window, app| {
                    let _ = entity.update(app, |this, cx| {
                        match validate_combobox_input(&selected_val, &vals_select) {
                            Ok(valid_val) => {
                                this.selected_handler_options
                                    .insert(opt_key_select.clone(), valid_val);
                                this.setup_screen.close_combobox();
                            }
                            Err(err) => {
                                this.setup_screen.combobox_validation_error = Some(err);
                            }
                        }
                        cx.notify();
                    });
                })
                .on_clear_draft(cx.listener(|this, _, _, cx| {
                    this.setup_screen.combobox_search_draft.clear();
                    this.setup_screen.combobox_validation_error = None;
                    cx.notify();
                }))
                .on_key_down(
                    cx.listener(move |this, event: &gpui::KeyDownEvent, _window, cx| {
                        if event.keystroke.modifiers.control
                            || event.keystroke.modifiers.alt
                            || event.keystroke.modifiers.platform
                        {
                            return;
                        }
                        let key = event.keystroke.key.as_str();
                        if key.eq_ignore_ascii_case("escape") {
                            this.setup_screen.close_combobox();
                            cx.notify();
                        } else if key.eq_ignore_ascii_case("backspace") {
                            this.setup_screen.combobox_search_draft.pop();
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                        } else if key.eq_ignore_ascii_case("enter") {
                            let draft = this.setup_screen.combobox_search_draft.trim();
                            if !draft.is_empty() {
                                match validate_combobox_input(draft, &vals_key) {
                                    Ok(valid_val) => {
                                        this.selected_handler_options
                                            .insert(opt_key_key.clone(), valid_val);
                                        this.setup_screen.close_combobox();
                                    }
                                    Err(err) => {
                                        this.setup_screen.combobox_validation_error = Some(err);
                                    }
                                }
                                cx.notify();
                            }
                        } else if key.eq_ignore_ascii_case("space") {
                            this.setup_screen.combobox_search_draft.push(' ');
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                        } else if let Some(ref s) = event.keystroke.key_char {
                            this.setup_screen.combobox_search_draft.push_str(s);
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                        } else if key.chars().count() == 1 {
                            this.setup_screen.combobox_search_draft.push_str(key);
                            this.setup_screen.combobox_validation_error = None;
                            cx.notify();
                        }
                    }),
                )
                .into_any_element()
            } else {
                let opt_key_prev = opt_key.clone();
                let opt_key_next = opt_key.clone();
                let vals_prev = opt_values.clone();
                let vals_next = opt_values.clone();
                let cur_for_prev = current_val.clone();
                let cur_for_next = current_val.clone();

                NefosOptionRow::new(("modal-opt-row", opt_idx), opt_name, opt_desc, current_val)
                    .on_prev(cx.listener(move |this, _, _, cx| {
                        if let Some(pos) = vals_prev.iter().position(|v| v == &cur_for_prev) {
                            let prev_idx = if pos == 0 {
                                vals_prev.len().saturating_sub(1)
                            } else {
                                pos - 1
                            };
                            if let Some(new_val) = vals_prev.get(prev_idx) {
                                this.selected_handler_options
                                    .insert(opt_key_prev.clone(), new_val.clone());
                            }
                        } else if let Some(first) = vals_prev.first() {
                            this.selected_handler_options
                                .insert(opt_key_prev.clone(), first.clone());
                        }
                        cx.notify();
                    }))
                    .on_next(cx.listener(move |this, _, _, cx| {
                        if let Some(pos) = vals_next.iter().position(|v| v == &cur_for_next) {
                            let next_idx = (pos + 1) % vals_next.len();
                            if let Some(new_val) = vals_next.get(next_idx) {
                                this.selected_handler_options
                                    .insert(opt_key_next.clone(), new_val.clone());
                            }
                        } else if let Some(first) = vals_next.first() {
                            this.selected_handler_options
                                .insert(opt_key_next.clone(), first.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            }
        }));

    let content = if active_tab == 1 && supports_proto {
        let proto_options = [
            (
                "proto_lock_input",
                "Lock External Input",
                "Intercept mouse and keyboard when windows lose focus",
                "true",
            ),
            (
                "proto_hook_cursor",
                "Hook Message Filter",
                "Filter Windows cursor and activation messages",
                "true",
            ),
            (
                "proto_virtual_cursor",
                "Virtual Mouse Cursor",
                "Emulate independent virtual mouse cursors for each instance",
                "true",
            ),
            (
                "proto_hook_controllers",
                "Direct Controller Hook",
                "Hook XInput and DirectInput controllers directly to game instances",
                "true",
            ),
            (
                "proto_fake_focus",
                "Fake Focus Heartbeat",
                "Send continuous WM_ACTIVATE and WM_SETFOCUS messages",
                "true",
            ),
        ];

        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(proto_options.into_iter().enumerate().map(
                |(idx, (key, title, desc, def_val))| {
                    let current_val = selected_options
                        .get(key)
                        .map(|s| s.as_str())
                        .unwrap_or(def_val);
                    let is_enabled = parse_boolean_value(current_val);
                    let key_str = key.to_string();

                    NefosToggle::new(("proto-opt-toggle", idx), title, is_enabled)
                        .description(desc)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let next = if is_enabled { "false" } else { "true" };
                            this.selected_handler_options
                                .insert(key_str.clone(), next.to_string());
                            cx.notify();
                        }))
                        .into_any_element()
                },
            ))
    } else {
        body
    };

    let mut modal_content = div().flex().flex_col().gap_3();
    if let Some(tabs) = modal_tabs {
        modal_content = modal_content.child(tabs);
    }
    modal_content = modal_content.child(content);

    let modal = NefosModal::new(
        "modal-handler-options-dialog",
        format!("Splitscreen Options - {}", game_title),
    )
    .subtitle("Select your preferences for gameplay, graphics, and input hooks:")
    .width(NefosModalWidth::Lg)
    .icon(IconName::SlidersHorizontal)
    .tone(NefosTagTone::Accent)
    .body(modal_content);

    modal
        .action(
            NefosButton::secondary("btn-cancel-modal-options", "Cancel / Close")
                .icon(IconName::X)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.setup_screen.options_modal_open = false;
                    this.setup_screen.close_combobox();
                    cx.notify();
                })),
        )
        .action(
            NefosButton::primary("btn-save-modal-options", "Apply & Save Preferences")
                .icon(IconName::Check)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.setup_screen.options_modal_open = false;
                    this.setup_screen.close_combobox();
                    this.status_message = "Handler options applied successfully.".to_string();
                    cx.notify();
                })),
        )
        .into_any_element()
}

/// Renders the complete modernized setup screen view with player count selector, monitor layout, and device assignment.
pub fn render_setup_view(
    setup_state: &SetupScreenView,
    selected_handler: Option<&HandlerItem>,
    selected_game: Option<&str>,
    player_count: usize,
    selected_layout: &str,
    detected_devices: &[DeviceItem],
    player_device_assignments: &[String],
    active_profile_id: Option<usize>,
    saved_profiles: &[(usize, String)],
    selected_handler_options: &std::collections::HashMap<String, String>,
    dropdown_player_device: Option<usize>,
    app_settings: &nucleus_profile::AppSettings,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    // Individual keyboards and mice are offered whenever the game's handler supports them.
    let handler_multi_kbm = selected_handler.map_or(false, |h| {
        h.supports_raw_input || h.supports_multiple_keyboards_and_mice
    });
    let game_title = selected_handler
        .map(|h| h.game_name.as_str())
        .unwrap_or("No Game Selected (choose from Library tab)");
    let current_layout = selected_layout.to_string();

    let handler_description = selected_handler
        .map(|h| h.description.as_str())
        .unwrap_or("");

    let layout_segments = [
        ("1x1 (Single)", Some(IconName::Monitor)),
        ("2x1 (Side-by-Side)", Some(IconName::Columns2)),
        ("1x2 (Stacked)", Some(IconName::Rows2)),
        ("2x2 (Grid)", Some(IconName::Grid2x2)),
        ("Top 1 / Bottom 2", Some(IconName::LayoutGrid)),
        ("Top 2 / Bottom 1", Some(IconName::LayoutGrid)),
        ("3x2 (Grid 6)", Some(IconName::Grid3x3)),
        ("4x2 (Grid 8)", Some(IconName::LayoutGrid)),
        ("4x4 (Grid 16)", Some(IconName::LayoutGrid)),
        ("Multi-Monitor", Some(IconName::Tv)),
    ];

    let mut seg_control = NefosSegmentedControl::new("setup-layout-segmented-control");
    for (layout_name, icon) in layout_segments {
        let is_active = current_layout.eq_ignore_ascii_case(layout_name)
            || (layout_name.contains("1x1") && current_layout.contains("1x1"))
            || (layout_name.contains("2x1") && current_layout.contains("2x1"))
            || (layout_name.contains("1x2") && current_layout.contains("1x2"))
            || (layout_name.contains("2x2") && current_layout.contains("2x2"))
            || (layout_name.contains("Top 1") && current_layout.contains("Top 1"))
            || (layout_name.contains("Top 2") && current_layout.contains("Top 2"))
            || (layout_name.contains("Grid 6") && current_layout.contains("Grid 6"))
            || (layout_name.contains("Grid 8") && current_layout.contains("Grid 8"))
            || (layout_name.contains("Grid 16") && current_layout.contains("Grid 16"))
            || (layout_name.contains("Multi-Monitor") && current_layout.contains("Multi-Monitor"));
        seg_control = seg_control.item(layout_name, icon, is_active);
    }

    seg_control = seg_control.on_select(move |this, idx, _window, cx| {
        let chosen = match idx {
            0 => "1x1 (Single)",
            1 => "2x1 (Side-by-Side)",
            2 => "1x2 (Stacked)",
            3 => "2x2 (Grid)",
            4 => "Top 1 / Bottom 2",
            5 => "Top 2 / Bottom 1",
            6 => "3x2 (Grid 6)",
            7 => "4x2 (Grid 8)",
            8 => "4x4 (Grid 16)",
            9 => "Multi-Monitor",
            _ => "2x1 (Side-by-Side)",
        };
        this.selected_layout = chosen.to_string();
        this.couch_setup_state.selected_layout = chosen.to_string();
        this.status_message = format!("Partition layout set to: {}", chosen);
        cx.notify();
    });

    let on_click_seg = cx.listener(move |this, &idx: &usize, _window, cx| {
        let chosen = match idx {
            0 => "1x1 (Single)",
            1 => "2x1 (Side-by-Side)",
            2 => "1x2 (Stacked)",
            3 => "2x2 (Grid)",
            4 => "Top 1 / Bottom 2",
            5 => "Top 2 / Bottom 1",
            6 => "3x2 (Grid 6)",
            7 => "4x2 (Grid 8)",
            8 => "4x4 (Grid 16)",
            9 => "Multi-Monitor",
            _ => "2x1 (Side-by-Side)",
        };
        this.selected_layout = chosen.to_string();
        this.couch_setup_state.selected_layout = chosen.to_string();
        this.status_message = format!("Partition layout set to: {}", chosen);
        cx.notify();
    });
    seg_control = seg_control.on_click(move |idx, win, app| {
        on_click_seg(&idx, win, app);
    });

    let grabbed_desktop_slot = setup_state.grabbed_slot;

    let unassigned_tray = render_unassigned_tray(detected_devices, player_device_assignments, cx);

    div()
        .relative()
        .size_full()
        .child(
            div()
                .id("setup-scroll")
                .flex()
                .flex_col()
                .size_full()
                .min_w_0()
                .p_6()
                .gap_4()
                .overflow_y_scroll()
                // Top Header with Title and Feature Badges
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Splitscreen Session Configuration"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme::text_muted())
                                        .child(format!("Target Game: {}", game_title)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .when_some(selected_handler, |el, h| {
                                    el.child(
                                        NefosTag::new(format!("Play Time: {}", h.formatted_play_time()))
                                            .id("badge-setup-playtime")
                                            .icon(IconName::Timer)
                                            .tone(NefosTagTone::Neutral)
                                            .tooltip("Total time spent playing this game through Nucleus Co-op"),
                                    )
                                    .child(
                                        NefosTag::new(format!("Last Played: {}", h.formatted_last_played()))
                                            .id("badge-setup-lastplayed")
                                            .icon(IconName::Calendar)
                                            .tone(NefosTagTone::Neutral)
                                            .tooltip("Last date and time this game was launched"),
                                    )
                                })
                                .when(
                                    selected_handler
                                        .map_or(false, |h| h.supports_xinput),
                                    |el| {
                                        el.child(
                                            NefosTag::new("XInput Supported")
                                                .id("badge-xinput")
                                                .icon(IconName::Gamepad2)
                                                .tooltip("Supports modern Xbox and XInput controllers"),
                                        )
                                    },
                                )
                                .when(
                                    selected_handler
                                        .map_or(false, |h| h.supports_dinput),
                                    |el| {
                                        el.child(
                                            NefosTag::new("DirectInput Supported")
                                                .id("badge-dinput")
                                                .icon(IconName::Gamepad)
                                                .tooltip("Supports legacy DirectInput controllers via dev_reorder"),
                                        )
                                    },
                                )
                                .when(
                                    selected_handler.map_or(false, |h| {
                                        h.supports_raw_input
                                            || h.supports_multiple_keyboards_and_mice
                                    }),
                                    |el| {
                                        el.child(
                                            NefosTag::new("RawInput (ProtoInput)")
                                                .id("badge-rawinput")
                                                .tone(NefosTagTone::Accent)
                                                .icon(IconName::Zap)
                                                .tooltip("Supports multiple keyboards & mice via ProtoInput hooks"),
                                        )
                                    },
                                )
                                .when(
                                    selected_handler
                                        .map_or(false, |h| h.supports_keyboard),
                                    |el| {
                                        el.child(
                                            NefosTag::new("Keyboard & Mouse")
                                                .id("badge-keyboard")
                                                .icon(IconName::Keyboard)
                                                .tooltip("Supports keyboard & mouse input"),
                                        )
                                    },
                                ),
                        ),
                )
                // Section 1: Profile Management Section
                .when(!app_settings.disable_game_profiles && !selected_handler.map_or(false, |h| h.disable_profile), |this_sec| {
                    this_sec.child(render_profile_management_section(
                        selected_handler,
                        selected_game,
                        active_profile_id,
                        saved_profiles,
                        cx,
                    ))
                })
                // Section 2: Handler Instructions & README Notes Card
                .child(
                    NefosCard::surface("setup-notes-card")
                        .title_text("Handler Instructions & README Notes")
                        .description_text("Essential launch parameters, focus notes, and in-game setup guides")
                        .header_action({
                            let copy_notes = handler_description.to_string();
                            NefosButton::secondary("btn-copy-setup-readme", "Copy Setup Notes")
                                .icon(IconName::Copy)
                                .size(NefosButtonSize::Sm)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_notes.clone()));
                                    this.show_osd("Handler notes copied to clipboard!", 2000, cx);
                                }))
                        })
                        .child(
                            div()
                                .id("setup-notes-scroll")
                                .w_full()
                                .min_w_0()
                                .max_h(px(220.0))
                                .overflow_y_scrollbar()
                                .p_3()
                                .rounded_md()
                                .bg(theme::bg_main())
                                .border_1()
                                .border_color(theme::border_color())
                                .child(
                                    if handler_description.is_empty() {
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child("No custom README notes provided for this handler. Connect gamepads, select your screen partitioning layout, and click Start below.")
                                            .into_any_element()
                                    } else {
                                        crate::markdown::render_markdown_with_id("setup-handler-notes", handler_description)
                                    }
                                ),
                        ),
                )
                // Section 3: Player Count Selector Card
                .child(
                    NefosCard::surface("setup-player-count-card")
                        .title_text("1. Select Player Count")
                        .description_text("Choose the number of active players for this splitscreen session")
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2p5()
                                .children([1, 2, 3, 4, 6, 8, 16].into_iter().map(|count| {
                                    let is_active = player_count == count;
                                    div()
                                        .id(("player-count-btn", count))
                                        .cursor_pointer()
                                        .px_4()
                                        .py_2()
                                        .rounded_md()
                                        .border_1()
                                        .when(is_active, |this| {
                                            this.bg(theme::accent())
                                                .border_color(theme::accent())
                                                .text_color(theme::text_on_accent())
                                                .font_weight(FontWeight::BOLD)
                                        })
                                        .when(!is_active, |this| {
                                            this.bg(theme::bg_main())
                                                .border_color(theme::border_color())
                                                .text_color(theme::text_muted())
                                                .hover(|s| s.bg(theme::bg_hover()).text_color(theme::text_primary()))
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.player_count = count;
                                            this.setup_screen.set_player_count(count);
                                            if count == 1 {
                                                this.selected_layout = "1x1 (Single)".to_string();
                                            } else if count == 4 {
                                                this.selected_layout = "2x2 (Grid)".to_string();
                                            } else if count == 3 {
                                                this.selected_layout = "Top 1 / Bottom 2".to_string();
                                            } else if count == 6 {
                                                this.selected_layout = "3x2 (Grid 6)".to_string();
                                            } else if count == 8 {
                                                this.selected_layout = "4x2 (Grid 8)".to_string();
                                            } else if count == 16 {
                                                this.selected_layout = "4x4 (Grid 16)".to_string();
                                            } else {
                                                this.selected_layout = "2x1 (Side-by-Side)".to_string();
                                            }
                                            this.couch_setup_state.selected_layout = this.selected_layout.clone();
                                            this.sync_player_assignments();
                                            this.status_message = format!("Configured session for {} player(s).", count);
                                            cx.notify();
                                        }))
                                        .child(if count == 1 { "1 Player (Solo)".to_string() } else { format!("{} Players", count) })
                                })),
                        ),
                )
                // Section 4: Screen Partitioning Layout Card with NefosSegmentedControl
                .child(
                    NefosCard::surface("setup-layout-card")
                        .title_text("2. Screen Partitioning Layout")
                        .description_text("Apple-style segmented control dynamically positions instances across viewport boundaries")
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(seg_control)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .h(px(180.0))
                                        .p_2()
                                        .bg(theme::bg_main())
                                        .rounded_lg()
                                        .border_2()
                                        .border_color(theme::border_color())
                                        .child(render_screen_partition_preview(player_count, selected_layout)),
                                ),
                        ),
                )
                // Section 5: Standardized Player Slot Cards Grid with NefosSlotCard
                .child(
                    NefosCard::surface("setup-device-allocation-card")
                        .title_text("3. Controller & Input Device Allocation")
                        .description_text("Assign gamepads and raw K&M. Click slot to grab, then click another to swap or drag-to-merge!")
                        .header_action(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(
                                    NefosTag::new(if app_settings.developer_mode && detected_devices.iter().any(|d| d.is_virtual) {
                                        format!(
                                            "{} Ready ({} Virtual)",
                                            detected_devices.len(),
                                            detected_devices.iter().filter(|d| d.is_virtual).count()
                                        )
                                    } else {
                                        format!("{} Ready", detected_devices.len())
                                    }),
                                )
                                .when(app_settings.developer_mode, |el| {
                                    el.child(
                                        NefosButton::secondary("btn-add-mock-pad", "+ Add Virtual Gamepad")
                                            .size(NefosButtonSize::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.add_virtual_gamepad();
                                                cx.notify();
                                            })),
                                    )
                                    .when(detected_devices.iter().any(|d| d.is_virtual), |el| {
                                        el.child(
                                            NefosButton::danger("btn-clear-mock-pads", "Clear Mock Pads")
                                                .size(NefosButtonSize::Sm)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.clear_virtual_gamepads();
                                                    cx.notify();
                                                })),
                                        )
                                    })
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_4()
                                        .w_full()
                                        .children((0..player_count).map(|p_idx| {
                                            let assigned = player_device_assignments
                                                .get(p_idx)
                                                .cloned()
                                                .unwrap_or_else(|| {
                                                    if p_idx < detected_devices.len() {
                                                        format!(
                                                            "Gamepad #{} ({})",
                                                            detected_devices[p_idx].id,
                                                            detected_devices[p_idx].name
                                                        )
                                                    } else {
                                                        "Keyboard & Mouse Slot".to_string()
                                                    }
                                                });

                                            let is_kbm = is_kbm_str(&assigned);
                                            let is_merged = is_merged_kbm_str(&assigned);
                                            let is_raw = assigned.contains("ProtoInput");
                                            let is_grabbed = grabbed_desktop_slot == Some(p_idx);
                                            let is_drop_target = grabbed_desktop_slot.is_some() && !is_grabbed;

                                            let status_badge = if assigned.starts_with("Unassigned") {
                                                NefosTag::new("Unassigned").tone(NefosTagTone::Warning)
                                            } else {
                                                NefosTag::new("Ready").tone(NefosTagTone::Success).with_dot()
                                            };

                                            let dev_guid_copy = detected_devices
                                                .iter()
                                                .find(|d| assigned.contains(&format!("#{}", d.id)))
                                                .map(|d| d.guid.clone());

                                            let mut sc = NefosSlotCard::new(
                                                format!("desktop-slot-card-{}", p_idx),
                                                p_idx,
                                            )
                                            .device_label(Some(assigned.clone()))
                                            .device_guid(dev_guid_copy.clone())
                                            .kbm(is_kbm)
                                            .merged_kbm(is_merged)
                                            .raw_device(is_raw)
                                            .drop_target(is_drop_target)
                                            .grabbed(is_grabbed)
                                            .status_badge(status_badge)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(grabbed) = this.setup_screen.grabbed_slot {
                                                    if grabbed == p_idx {
                                                        this.setup_screen.grabbed_slot = None;
                                                        this.status_message = "Cancelled slot grab.".to_string();
                                                    } else {
                                                        let dev_a = this.player_device_assignments.get(grabbed).cloned().unwrap_or_default();
                                                        let dev_b = this.player_device_assignments.get(p_idx).cloned().unwrap_or_default();

                                                        let is_mouse_onto_kb = is_raw_mouse_str(&dev_a) && is_raw_keyboard_str(&dev_b);
                                                        let is_kb_onto_mouse = is_raw_keyboard_str(&dev_a) && is_raw_mouse_str(&dev_b);

                                                        if is_mouse_onto_kb {
                                                            let kb_clean = dev_b.replace(" (ProtoInput)", "").trim().to_string();
                                                            let m_clean = dev_a.replace(" (ProtoInput)", "").trim().to_string();
                                                            this.player_device_assignments[p_idx] = format!("{} + {} (ProtoInput)", kb_clean, m_clean);
                                                            if grabbed < this.player_device_assignments.len() {
                                                                this.player_device_assignments.remove(grabbed);
                                                            }
                                                            this.player_count = this.player_device_assignments.len();
                                                            this.setup_screen.player_count = this.player_count;
                                                            this.setup_screen.grabbed_slot = None;
                                                            this.status_message = format!("Merged Keyboard and Mouse into Player Slot {}!", p_idx + 1);
                                                        } else if is_kb_onto_mouse {
                                                            let kb_clean = dev_a.replace(" (ProtoInput)", "").trim().to_string();
                                                            let m_clean = dev_b.replace(" (ProtoInput)", "").trim().to_string();
                                                            this.player_device_assignments[p_idx] = format!("{} + {} (ProtoInput)", kb_clean, m_clean);
                                                            if grabbed < this.player_device_assignments.len() {
                                                                this.player_device_assignments.remove(grabbed);
                                                            }
                                                            this.player_count = this.player_device_assignments.len();
                                                            this.setup_screen.player_count = this.player_count;
                                                            this.setup_screen.grabbed_slot = None;
                                                            this.status_message = format!("Merged Keyboard and Mouse into Player Slot {}!", p_idx + 1);
                                                        } else {
                                                            if grabbed < this.player_device_assignments.len() && p_idx < this.player_device_assignments.len() {
                                                                this.player_device_assignments.swap(grabbed, p_idx);
                                                            }
                                                            this.setup_screen.grabbed_slot = None;
                                                            this.status_message = format!("Swapped Player Slot {} and Player Slot {}", grabbed + 1, p_idx + 1);
                                                        }
                                                    }
                                                } else {
                                                    this.setup_screen.grabbed_slot = Some(p_idx);
                                                    this.status_message = format!("Grabbed Player Slot {}. Click another slot to swap or merge!", p_idx + 1);
                                                }
                                                cx.notify();
                                            }))
                                            .on_remove(cx.listener(move |this, _, _, cx| {
                                                if p_idx < this.player_device_assignments.len() {
                                                    this.player_device_assignments[p_idx] = "Unassigned (No Device Available)".to_string();
                                                    this.status_message = format!("Unassigned device from Player Slot {}.", p_idx + 1);
                                                }
                                                cx.notify();
                                            }));

                                            if is_merged {
                                                sc = sc.on_split(cx.listener(move |this, _, _, cx| {
                                                    if p_idx < this.player_device_assignments.len() && this.player_device_assignments.len() < 32 {
                                                        let dev = this.player_device_assignments[p_idx].clone();
                                                        if is_merged_kbm_str(&dev) {
                                                            let (kb_name, mouse_name) = if dev.contains('&') {
                                                                ("Keyboard #1".to_string(), "Mouse #1".to_string())
                                                            } else if dev.contains('+') {
                                                                let without_suffix = dev
                                                                    .replace(" (ProtoInput)", "")
                                                                    .replace(" (ProtoInput Pair)", "");
                                                                let parts: Vec<String> = without_suffix.split('+').map(|s| s.trim().to_string()).collect();
                                                                let kb = parts.first().cloned().unwrap_or_else(|| "Keyboard #1".to_string());
                                                                let ms = parts.get(1).cloned().unwrap_or_else(|| "Mouse #1".to_string());
                                                                (kb, ms)
                                                            } else {
                                                                ("Keyboard #1".to_string(), "Mouse #1".to_string())
                                                            };

                                                            let final_kb = if kb_name.contains("Mouse") {
                                                                "Keyboard #1 (ProtoInput)".to_string()
                                                            } else if kb_name.contains("(ProtoInput)") {
                                                                kb_name
                                                            } else {
                                                                format!("{} (ProtoInput)", kb_name)
                                                            };

                                                            let final_mouse = if mouse_name.contains("Keyboard") {
                                                                "Mouse #1 (ProtoInput)".to_string()
                                                            } else if mouse_name.contains("(ProtoInput)") {
                                                                mouse_name
                                                            } else {
                                                                format!("{} (ProtoInput)", mouse_name)
                                                            };

                                                            this.player_device_assignments[p_idx] = final_kb;
                                                            this.player_device_assignments.insert(p_idx + 1, final_mouse);
                                                            this.player_count = this.player_device_assignments.len().min(32);
                                                            this.setup_screen.player_count = this.player_count;
                                                            this.status_message = format!("Split composite K&M on Player Slot {} into separate slots.", p_idx + 1);
                                                        }
                                                    }
                                                    cx.notify();
                                                }));
                                            }

                                            if let Some(guid_str) = dev_guid_copy {
                                                sc = sc.on_ignore(cx.listener(move |this, _, _, cx| {
                                                    if !this.app_settings.ignored_device_guids.contains(&guid_str) {
                                                        this.app_settings.ignored_device_guids.push(guid_str.clone());
                                                        let _ = this.app_settings.save();
                                                        this.status_message = format!("Added device {} to ignore list.", guid_str);
                                                    }
                                                    cx.notify();
                                                }));
                                            }

                                            let is_device_dropdown_open = dropdown_player_device == Some(p_idx);
                                            let cur_assigned = assigned.clone();

                                            div()
                                                .flex_1()
                                                .min_w(px(240.0))
                                                .max_w(px(340.0))
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(sc)
                                                // Device Selector dropdown trigger button
                                                .child(
                                                    div()
                                                        .id(("btn-select-device-card", p_idx))
                                                        .cursor_pointer()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .px_3()
                                                        .py_1p5()
                                                        .rounded_md()
                                                        .bg(theme::bg_panel())
                                                        .border_1()
                                                        .border_color(if is_device_dropdown_open { theme::accent() } else { theme::border_color() })
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme::text_primary())
                                                        .hover(|s| s.bg(theme::bg_hover()))
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.dropdown_player_device = if this.dropdown_player_device == Some(p_idx) {
                                                                None
                                                            } else {
                                                                Some(p_idx)
                                                            };
                                                            cx.notify();
                                                        }))
                                                        .child("Change Device Assignment")
                                                        .child(
                                                            div()
                                                                .text_color(theme::accent())
                                                                .child(if is_device_dropdown_open { "▲" } else { "▼" }),
                                                        ),
                                                )
                                                // Expandable device options
                                                .when(is_device_dropdown_open, |slot| {
                                                    let mut keyboard_taken = false;
                                                    let mut claimed_pads = Vec::new();
                                                    for (o_idx, a) in player_device_assignments.iter().enumerate() {
                                                        if o_idx == p_idx { continue; }
                                                        if a == "Keyboard & Mouse Slot" || a == "Keyboard & Mouse (Paired)" { keyboard_taken = true; }
                                                        for d in detected_devices {
                                                            if a.starts_with(&format!("Gamepad #{}", d.id))
                                                                || a.starts_with(&format!("Virtual Gamepad #{}", d.id))
                                                                || a.starts_with(&format!("Virtual Controller #{}", d.id))
                                                            {
                                                                claimed_pads.push(d.id);
                                                            }
                                                        }
                                                    }
                                                    let mut dev_opts: Vec<String> = Vec::new();
                                                    for d in detected_devices {
                                                        if !claimed_pads.contains(&d.id) {
                                                            if d.is_virtual {
                                                                dev_opts.push(format!("Virtual Gamepad #{} ({})", d.id, d.name));
                                                            } else {
                                                                dev_opts.push(format!("Gamepad #{} ({})", d.id, d.name));
                                                            }
                                                        }
                                                    }
                                                    if !keyboard_taken {
                                                        dev_opts.push("Keyboard & Mouse (Paired)".to_string());
                                                    }
                                                    if app_settings.developer_mode || handler_multi_kbm {
                                                        // One entry per attached keyboard/mouse, plus pairs.
                                                        dev_opts.extend(nucleus_input::proto_assignment_labels(
                                                            &nucleus_input::cached_raw_devices(),
                                                            |n| format!("Keyboard #{n} + Mouse #{n} (ProtoInput)"),
                                                        ));
                                                    }
                                                    if detected_devices.iter().any(|d| !d.is_virtual) {
                                                        dev_opts.push("DirectInput Emulation (x360ce)".to_string());
                                                    }
                                                    dev_opts.push("Unassigned (No Device Available)".to_string());

                                                    let active_dev = cur_assigned.clone();
                                                    slot.child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .p_2()
                                                            .bg(theme::bg_panel())
                                                            .rounded_md()
                                                            .border_1()
                                                            .border_color(theme::accent().opacity(0.4))
                                                            .gap_1()
                                                            .children(dev_opts.into_iter().enumerate().map(|(o_i, opt)| {
                                                                let is_current = opt == active_dev;
                                                                let opt_str = opt.clone();
                                                                div()
                                                                    .id(("dev-opt", p_idx * 100 + o_i))
                                                                    .cursor_pointer()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_between()
                                                                    .px_2p5()
                                                                    .py_1p5()
                                                                    .rounded_md()
                                                                    .text_xs()
                                                                    .when(is_current, |s| {
                                                                        s.bg(theme::accent().opacity(0.15))
                                                                            .text_color(theme::accent())
                                                                            .font_weight(FontWeight::BOLD)
                                                                    })
                                                                    .when(!is_current, |s| {
                                                                        s.text_color(theme::text_primary())
                                                                            .hover(|h| h.bg(theme::bg_hover()))
                                                                    })
                                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                                        if p_idx < this.player_device_assignments.len() {
                                                                            this.player_device_assignments[p_idx] = opt_str.clone();
                                                                            this.status_message = format!("Player {} assigned to {}", p_idx + 1, opt_str);
                                                                        }
                                                                        this.dropdown_player_device = None;
                                                                        cx.notify();
                                                                    }))
                                                                    .child(opt)
                                                                    .children(if is_current {
                                                                        Some(gpui_component::Icon::new(IconName::Check).size(px(14.0)).text_color(theme::accent()))
                                                                    } else {
                                                                        None
                                                                    })
                                                            }))
                                                    )
                                                })
                                                .into_any_element()
                                        })),
                                ),
                        ),
                )
                // Unassigned Controllers Tray (if any detected gamepads are not assigned)
                .children(unassigned_tray)
                // Input device warning banner if players lack devices
                .when(
                    player_device_assignments
                        .iter()
                        .any(|a| a.starts_with("Unassigned")),
                    |this| {
                        let unassigned_count = player_device_assignments
                            .iter()
                            .filter(|a| a.starts_with("Unassigned"))
                            .count();
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .p_3()
                                .rounded_lg()
                                .bg(theme::danger_subtle())
                                .border_1()
                                .border_color(theme::danger())
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::danger())
                                        .child(format!(
                                            "Warning: {} player slot(s) unassigned. Connect physical gamepads or adjust player count.",
                                            unassigned_count
                                        )),
                                ),
                        )
                    },
                )
                // Section 6: Audio Routing Controls Wrapped in NefosCard
                .child(render_audio_routing_section(
                    player_count,
                    setup_state.dropdown_audio_instance,
                    app_settings,
                    cx,
                ))
                // Section 7: Custom Handler Options Card with NefosOptionRow & NefosToggle
                .when(
                    selected_handler.map_or(false, |h| !h.options.is_empty()),
                    |this_sec| {
                        let options = selected_handler.map(|h| &h.options).unwrap();
                        this_sec.child(
                            NefosCard::surface("setup-options-summary-card")
                                .title_text("5. Custom Handler Options (Game.AddOption)")
                                .description_text("Configure per-game script settings, rendering parameters, and player flags")
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .children(options.iter().enumerate().map(|(opt_idx, opt)| {
                                            let opt_key = opt.key.clone();
                                            let opt_name = opt.name.clone();
                                            let opt_desc = opt.description.clone();
                                            let opt_values = opt.values.clone();

                                            let current_val = selected_handler_options
                                                .get(&opt_key)
                                                .cloned()
                                                .unwrap_or_else(|| {
                                                    opt_values.first().cloned().unwrap_or_else(|| "Default".to_string())
                                                });

                                            if is_boolean_option(&opt_values) {
                                                let is_enabled = parse_boolean_value(&current_val);
                                                let opt_key_click = opt_key.clone();
                                                let vals_toggle = opt_values.clone();

                                                NefosToggle::new(
                                                    ("setup-opt-toggle", opt_idx),
                                                    opt_name,
                                                    is_enabled,
                                                )
                                                .description(opt_desc)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    let next_val = if is_enabled {
                                                        vals_toggle
                                                            .iter()
                                                            .find(|v| !parse_boolean_value(v))
                                                            .cloned()
                                                            .unwrap_or_else(|| "false".to_string())
                                                    } else {
                                                        vals_toggle
                                                            .iter()
                                                            .find(|v| parse_boolean_value(v))
                                                            .cloned()
                                                            .unwrap_or_else(|| "true".to_string())
                                                    };
                                                    this.selected_handler_options
                                                        .insert(opt_key_click.clone(), next_val);
                                                    cx.notify();
                                                }))
                                                .into_any_element()
                                            } else if should_use_combobox(&opt_values) {
                                                let is_open = setup_state.combobox_open_key.as_deref() == Some(opt_key.as_str());
                                                let draft = if is_open {
                                                    setup_state.combobox_search_draft.clone()
                                                } else {
                                                    String::new()
                                                };
                                                let validation_error = if is_open {
                                                    setup_state.combobox_validation_error.clone()
                                                } else {
                                                    None
                                                };

                                                let opt_key_toggle = opt_key.clone();
                                                let opt_key_select = opt_key.clone();
                                                let opt_key_key = opt_key.clone();
                                                let vals_select = opt_values.clone();
                                                let vals_key = opt_values.clone();
                                                let entity = cx.entity().clone();

                                                NefosOptionCombobox::new(
                                                    ("setup-opt-combobox", opt_idx),
                                                    opt_name,
                                                    opt_desc,
                                                    current_val,
                                                    opt_values.clone(),
                                                )
                                                .is_open(is_open)
                                                .search_draft(draft)
                                                .validation_error(validation_error)
                                                .on_toggle(cx.listener(move |this, _, _, cx| {
                                                    this.setup_screen.toggle_combobox(&opt_key_toggle);
                                                    cx.notify();
                                                }))
                                                .on_select(move |selected_val, _window, app| {
                                                    let _ = entity.update(app, |this, cx| {
                                                        match validate_combobox_input(&selected_val, &vals_select) {
                                                            Ok(valid_val) => {
                                                                this.selected_handler_options
                                                                    .insert(opt_key_select.clone(), valid_val);
                                                                this.setup_screen.close_combobox();
                                                            }
                                                            Err(err) => {
                                                                this.setup_screen.combobox_validation_error = Some(err);
                                                            }
                                                        }
                                                        cx.notify();
                                                    });
                                                })
                                                .on_clear_draft(cx.listener(|this, _, _, cx| {
                                                    this.setup_screen.combobox_search_draft.clear();
                                                    this.setup_screen.combobox_validation_error = None;
                                                    cx.notify();
                                                }))
                                                .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _window, cx| {
                                                    if event.keystroke.modifiers.control || event.keystroke.modifiers.alt || event.keystroke.modifiers.platform {
                                                        return;
                                                    }
                                                    let key = event.keystroke.key.as_str();
                                                    if key.eq_ignore_ascii_case("escape") {
                                                        this.setup_screen.close_combobox();
                                                        cx.notify();
                                                    } else if key.eq_ignore_ascii_case("backspace") {
                                                        this.setup_screen.combobox_search_draft.pop();
                                                        this.setup_screen.combobox_validation_error = None;
                                                        cx.notify();
                                                    } else if key.eq_ignore_ascii_case("enter") {
                                                        let draft = this.setup_screen.combobox_search_draft.trim();
                                                        if !draft.is_empty() {
                                                            match validate_combobox_input(draft, &vals_key) {
                                                                Ok(valid_val) => {
                                                                    this.selected_handler_options
                                                                        .insert(opt_key_key.clone(), valid_val);
                                                                    this.setup_screen.close_combobox();
                                                                }
                                                                Err(err) => {
                                                                    this.setup_screen.combobox_validation_error = Some(err);
                                                                }
                                                            }
                                                            cx.notify();
                                                        }
                                                    } else if key.eq_ignore_ascii_case("space") {
                                                        this.setup_screen.combobox_search_draft.push(' ');
                                                        this.setup_screen.combobox_validation_error = None;
                                                        cx.notify();
                                                    } else if let Some(ref s) = event.keystroke.key_char {
                                                        this.setup_screen.combobox_search_draft.push_str(s);
                                                        this.setup_screen.combobox_validation_error = None;
                                                        cx.notify();
                                                    } else if key.chars().count() == 1 {
                                                        this.setup_screen.combobox_search_draft.push_str(key);
                                                        this.setup_screen.combobox_validation_error = None;
                                                        cx.notify();
                                                    }
                                                }))
                                                .into_any_element()
                                            } else {
                                                let opt_key_prev = opt_key.clone();
                                                let opt_key_next = opt_key.clone();
                                                let vals_prev = opt_values.clone();
                                                let vals_next = opt_values.clone();
                                                let cur_for_prev = current_val.clone();
                                                let cur_for_next = current_val.clone();

                                                NefosOptionRow::new(
                                                    ("setup-opt-row", opt_idx),
                                                    opt_name,
                                                    opt_desc,
                                                    current_val,
                                                )
                                                .on_prev(cx.listener(move |this, _, _, cx| {
                                                    if let Some(pos) = vals_prev.iter().position(|v| v == &cur_for_prev) {
                                                        let prev_idx = if pos == 0 {
                                                            vals_prev.len().saturating_sub(1)
                                                        } else {
                                                            pos - 1
                                                        };
                                                        if let Some(new_val) = vals_prev.get(prev_idx) {
                                                            this.selected_handler_options.insert(opt_key_prev.clone(), new_val.clone());
                                                        }
                                                    } else if let Some(first) = vals_prev.first() {
                                                        this.selected_handler_options.insert(opt_key_prev.clone(), first.clone());
                                                    }
                                                    cx.notify();
                                                }))
                                                .on_next(cx.listener(move |this, _, _, cx| {
                                                    if let Some(pos) = vals_next.iter().position(|v| v == &cur_for_next) {
                                                        let next_idx = (pos + 1) % vals_next.len();
                                                        if let Some(new_val) = vals_next.get(next_idx) {
                                                            this.selected_handler_options.insert(opt_key_next.clone(), new_val.clone());
                                                        }
                                                    } else if let Some(first) = vals_next.first() {
                                                        this.selected_handler_options.insert(opt_key_next.clone(), first.clone());
                                                    }
                                                    cx.notify();
                                                }))
                                                .into_any_element()
                                            }
                                        })),
                                ),
                        )
                    },
                )
                // Section 7.5: Multiplayer Backend & PeerParty P2P Co-op
                .child({
                    let emu_val = selected_handler_options
                        .get("preferred_emulator")
                        .cloned()
                        .unwrap_or_else(|| "Auto".to_string());
                    let is_peerparty = emu_val.contains("PeerParty");
                    let room_val = selected_handler_options
                        .get("peerparty_room")
                        .cloned()
                        .unwrap_or_default();
                    let display_room = if room_val.is_empty() {
                        "None (LAN Only)".to_string()
                    } else {
                        room_val.clone()
                    };

                    NefosCard::surface("setup-multiplayer-backend-card")
                        .title_text("Multiplayer Backend & PeerParty P2P Co-op")
                        .description_text("Select emulator backend (Auto, PeerParty, Goldberg, Nemirtingas)")
                        .child({
                            let emu_options = vec![
                                "Auto".to_string(),
                                "PeerParty (P2P DHT)".to_string(),
                                "Goldberg (Offline LAN)".to_string(),
                                "Nemirtingas (Epic/Galaxy)".to_string(),
                            ];
                            let emu_options_prev = emu_options.clone();
                            let emu_options_next = emu_options.clone();
                            let cur_val_prev = emu_val.clone();
                            let cur_val_next = emu_val.clone();

                            NefosOptionRow::new(
                                "setup-emu-backend-row",
                                "Preferred Emulator Backend",
                                "Switch between decentralized P2P co-op and offline LAN emulators",
                                emu_val.clone(),
                            )
                            .on_prev(cx.listener(move |this, _, _, cx| {
                                let pos = emu_options_prev.iter().position(|v| v == &cur_val_prev).unwrap_or(0);
                                let prev_idx = if pos == 0 { emu_options_prev.len() - 1 } else { pos - 1 };
                                this.selected_handler_options.insert("preferred_emulator".to_string(), emu_options_prev[prev_idx].clone());
                                cx.notify();
                            }))
                            .on_next(cx.listener(move |this, _, _, cx| {
                                let pos = emu_options_next.iter().position(|v| v == &cur_val_next).unwrap_or(0);
                                let next_idx = (pos + 1) % emu_options_next.len();
                                this.selected_handler_options.insert("preferred_emulator".to_string(), emu_options_next[next_idx].clone());
                                cx.notify();
                            }))
                            .into_any_element()
                        })
                        .when(is_peerparty, |card| {
                            let room_val_clear = room_val.clone();
                            card.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .p_3()
                                    .rounded_lg()
                                    .bg(theme::bg_subtle())
                                    .border_1()
                                    .border_color(theme::border_subtle())
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::text_primary())
                                                    .child("PeerParty Room Code"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme::text_muted())
                                                    .child("Connect with friends over the internet using BitTorrent DHT"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                NefosTag::new(display_room)
                                                    .tone(if room_val.is_empty() { NefosTagTone::Neutral } else { NefosTagTone::Accent })
                                            )
                                            .child(
                                                NefosButton::secondary("btn-gen-room-code", "🎲 New Code")
                                                    .size(NefosButtonSize::Sm)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        let random_num: u16 = (std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_millis() & 0xFFFF) as u16;
                                                        let new_code = format!("ROOM-{:04X}", random_num);
                                                        this.selected_handler_options.insert("peerparty_room".to_string(), new_code);
                                                        cx.notify();
                                                    }))
                                            )
                                            .when(!room_val_clear.is_empty(), |b| {
                                                b.child(
                                                    NefosButton::secondary("btn-clear-room-code", "Clear")
                                                        .size(NefosButtonSize::Sm)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.selected_handler_options.remove("peerparty_room");
                                                            cx.notify();
                                                        }))
                                                )
                                            })
                                    )
                            )
                        })
                })
                // Section 8: Save Slots & "Start Session" Buttons
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .w_full()
                        .child(
                            NefosButton::secondary("btn-manage-saves", "Manage Save Slots")
                                .size(NefosButtonSize::Lg)
                                .icon(IconName::Save)
                                .on_click({
                                    let g_guid = selected_handler.map(|h| h.guid.clone()).unwrap_or_default();
                                    let g_name = game_title.to_string();
                                    cx.listener(move |this, _, _, cx| {
                                        if !g_guid.is_empty() {
                                            if let Some(ref lister) = this.callbacks.on_list_save_slots {
                                                this.current_save_slots = lister(&g_guid);
                                            }
                                            this.save_slots_modal.open(&g_guid, &g_name, 0);
                                            cx.notify();
                                        }
                                    })
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    NefosButton::primary("btn-start-session", "START SPLITSCREEN SESSION")
                                        .size(NefosButtonSize::Lg)
                                        .icon(IconName::Play)
                                        .variant(
                                            if player_device_assignments
                                                .iter()
                                                .any(|a| a.starts_with("Unassigned"))
                                            {
                                                NefosButtonVariant::Secondary
                                            } else {
                                                NefosButtonVariant::Primary
                                            },
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this
                                                .player_device_assignments
                                                .iter()
                                                .any(|a| a.starts_with("Unassigned"))
                                            {
                                                this.status_message = "Cannot launch: Not all players have an assigned input device. Please connect gamepads or reduce player count.".to_string();
                                                cx.notify();
                                                return;
                                            }

                                            let target = this
                                                .selected_game
                                                .clone()
                                                .unwrap_or_else(|| "My Game".to_string());
                                            println!(
                                                "Starting splitscreen session: game '{}', {} players, layout {}",
                                                target, this.player_count, this.selected_layout
                                            );
                                            if let Some(ref launcher) = this.callbacks.on_launch {
                                                launcher(
                                                    &target,
                                                    this.player_count,
                                                    &this.selected_layout,
                                                    &this.selected_handler_options,
                                                    &this.player_device_assignments,
                                                    &this.player_monitor_assignments,
                                                    &this.player_audio_device_assignments,
                                                );
                                                this.status_message = format!(
                                                    "Splitscreen session launched for '{}' ({} players)! Press Ctrl+Q or VK_END to toggle focus lock.",
                                                    target, this.player_count
                                                );
                                            } else {
                                                this.status_message = format!(
                                                    "Splitscreen session started! {} players assigned. Press Ctrl+Q to toggle focus lock.",
                                                    this.player_count
                                                );
                                            }
                                            cx.notify();
                                        })),
                                ),
                        ),
                ),
        )
        // Handler Custom Options Modal Overlay when requested
        .when(
            setup_state.options_modal_open
                && selected_handler.map_or(false, |h| !h.options.is_empty()),
            |overlay| {
                if let Some(handler) = selected_handler {
                    overlay.child(render_handler_options_modal(
                        game_title,
                        handler,
                        selected_handler_options,
                        setup_state,
                        cx,
                    ))
                } else {
                    overlay
                }
            },
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn test_setup_screen_player_count() {
        let mut screen = SetupScreenView::new();
        assert_eq!(screen.player_count, 0);

        screen.set_player_count(4);
        assert_eq!(screen.player_count, 4);
    }

    #[test]
    fn test_setup_screen_grabbed_slot_and_modal() {
        let mut screen = SetupScreenView::new();
        assert_eq!(screen.grabbed_slot, None);
        assert!(!screen.options_modal_open);

        screen.grabbed_slot = Some(1);
        screen.options_modal_open = true;

        assert_eq!(screen.grabbed_slot, Some(1));
        assert!(screen.options_modal_open);
    }

    #[test]
    fn test_is_merged_and_raw_kbm_detection() {
        assert!(is_merged_kbm_str("Keyboard #1 + Mouse #1 (ProtoInput)"));
        assert!(is_merged_kbm_str("K#1 + M#1 (ProtoInput Pair)"));
        assert!(is_merged_kbm_str("Keyboard & Mouse (Paired)"));
        assert!(!is_merged_kbm_str("Gamepad #0 (Xbox Controller)"));

        assert!(is_raw_keyboard_str("Keyboard #1 (ProtoInput)"));
        assert!(!is_raw_keyboard_str("Keyboard #1 + Mouse #1 (ProtoInput)"));

        assert!(is_raw_mouse_str("Mouse #1 (ProtoInput)"));
        assert!(!is_raw_mouse_str("Keyboard #1 + Mouse #1 (ProtoInput)"));
    }

    #[test]
    fn test_is_boolean_option_detection() {
        let bool_opts = vec!["true".to_string(), "false".to_string()];
        assert!(is_boolean_option(&bool_opts));

        let yn_opts = vec!["Yes".to_string(), "No".to_string()];
        assert!(is_boolean_option(&yn_opts));

        let res_opts = vec!["1080p".to_string(), "1440p".to_string(), "4K".to_string()];
        assert!(!is_boolean_option(&res_opts));
    }

    #[test]
    fn test_setup_screen_combobox_state_toggle_and_reset() {
        let mut screen = SetupScreenView::new();
        assert_eq!(screen.combobox_open_key, None);
        assert_eq!(screen.combobox_search_draft, "");
        assert_eq!(screen.combobox_validation_error, None);

        // Toggle open
        screen.toggle_combobox("resolution");
        assert_eq!(screen.combobox_open_key.as_deref(), Some("resolution"));
        assert_eq!(screen.combobox_search_draft, "");
        assert_eq!(screen.combobox_validation_error, None);

        // Type draft and set error
        screen.combobox_search_draft = "1080".to_string();
        screen.combobox_validation_error = Some("Invalid choice".to_string());

        // Toggle same key closes and resets draft + error
        screen.toggle_combobox("resolution");
        assert_eq!(screen.combobox_open_key, None);
        assert_eq!(screen.combobox_search_draft, "");
        assert_eq!(screen.combobox_validation_error, None);

        // Toggle open different key
        screen.toggle_combobox("quality");
        assert_eq!(screen.combobox_open_key.as_deref(), Some("quality"));
        screen.combobox_search_draft = "Ultra".to_string();

        // Close combobox
        screen.close_combobox();
        assert_eq!(screen.combobox_open_key, None);
        assert_eq!(screen.combobox_search_draft, "");
        assert_eq!(screen.combobox_validation_error, None);
    }
}
