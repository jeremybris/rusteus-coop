//! Screens 4 & 5: Big Picture Settings (General & Controllers with live identify) for 10-foot Couch mode.
//!
//! Provides:
//! - Left sidebar with General & Controllers tabs, app version, and documentation link.
//! - Screen 4 (General): Start in Big Picture toggle, Interface scale, Preferred display, Sounds, Language, Updates.
//! - Screen 5 (Controllers): Hardware summary, Refresh devices button, Assignment method,
//!   Connected devices list with live "Test / identify >" rumble feedback and Ignored devices count.

use gpui::prelude::*;
use gpui::*;
use gpui_component::Icon;
use gpui_kit_assets::IconName;

use crate::theme;
use crate::window::MainWindow;

/// Renders the complete Big Picture Settings screen (Screens 4 & 5).
pub fn render_big_picture_settings_view(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let is_controllers_tab = win.big_picture_settings_tab == 1;

    div()
        .id("big-picture-settings")
        .flex()
        .flex_col()
        .size_full()
        .px_10()
        .py_3()
        .bg(theme::bg_main())
        .text_color(theme::text_primary())
        // Header
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .pb_3()
                .child(
                    div()
                        .text_3xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("Settings"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text_muted())
                        .child(if is_controllers_tab {
                            "Everyone connected. Everyone in control."
                        } else {
                            "Make Nucleus feel at home."
                        }),
                ),
        )
        // Two-Column Grid: Left Sidebar & Right Settings Panel
        .child(
            div()
                .flex()
                .gap_8()
                .w_full()
                // Left Navigation Sidebar
                .child(
                    div()
                        .w(px(260.0))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .gap_6()
                        // Tabs List
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(render_sidebar_tab(
                                    "btn-settings-tab-general",
                                    "General",
                                    !is_controllers_tab,
                                    win.settings_focus_zone == 0
                                        && win.big_picture_settings_tab == 0,
                                    |this, cx| {
                                        this.big_picture_settings_tab = 0;
                                        cx.notify();
                                    },
                                    cx,
                                ))
                                .child(render_sidebar_tab(
                                    "btn-settings-tab-controllers",
                                    "Controllers",
                                    is_controllers_tab,
                                    win.settings_focus_zone == 0
                                        && win.big_picture_settings_tab == 1,
                                    |this, cx| {
                                        this.big_picture_settings_tab = 1;
                                        cx.notify();
                                    },
                                    cx,
                                )),
                        )
                        // Sidebar Footer: Version & Help / Summary
                        .child(if is_controllers_tab {
                            render_controllers_sidebar_summary(win, cx)
                        } else {
                            render_general_sidebar_footer(cx)
                        }),
                )
                // Right Settings Panel
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(if is_controllers_tab {
                            render_controllers_panel(win, cx)
                        } else {
                            render_general_panel(win, cx)
                        }),
                ),
        )
        .into_any_element()
}

fn render_sidebar_tab<F>(
    id: &'static str,
    label: &'static str,
    is_active: bool,
    has_focus_ring: bool,
    on_click_action: F,
    cx: &mut Context<MainWindow>,
) -> AnyElement
where
    F: Fn(&mut MainWindow, &mut Context<MainWindow>) + Send + Sync + 'static,
{
    div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .px_4()
        .py_2p5()
        .rounded_lg()
        .bg(if is_active {
            theme::accent().opacity(0.16)
        } else {
            gpui::transparent_black()
        })
        .border_2()
        .border_color(if has_focus_ring {
            rgb(0x22c55e).into()
        } else {
            gpui::transparent_black()
        })
        .hover(|s| {
            if !is_active {
                s.bg(theme::bg_panel())
            } else {
                s
            }
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            on_click_action(this, cx);
        }))
        .child(
            div()
                .text_sm()
                .font_weight(if is_active {
                    FontWeight::BOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if is_active {
                    theme::accent()
                } else {
                    theme::text_muted()
                })
                .child(label),
        )
        .into_any_element()
}

fn render_general_sidebar_footer(cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .pt_4()
        .border_t_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_muted())
                        .child("NUCLEUS CO-OP"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("Version 2.4.2"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme::text_muted())
                        .child("Built for playing together."),
                ),
        )
        .child(
            div()
                .id("btn-settings-help-doc")
                .cursor_pointer()
                .flex()
                .items_center()
                .gap_1p5()
                .pt_1()
                .hover(|s| s.opacity(0.80))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open_url_external("https://nucleus-coop.github.io/", cx);
                }))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text_primary())
                        .child("Help & documentation"),
                )
                .child(
                    Icon::new(IconName::ArrowUpRight)
                        .size_3()
                        .text_color(theme::text_muted()),
                ),
        )
        .into_any_element()
}

fn render_controllers_sidebar_summary(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let connected = win.detected_devices_list.len().max(win.detected_devices);
    let assigned = win.couch_setup_state.slots.len();
    let available = connected.saturating_sub(assigned);
    let ignored = win.app_settings.ignored_device_guids.len();

    div()
        .flex()
        .flex_col()
        .gap_3()
        .pt_4()
        .border_t_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::accent())
                .child(format!("{} connected", connected)),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text_muted())
                .child(format!(
                    "{} assigned · {} available\n{} ignored",
                    assigned, available, ignored
                )),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::NORMAL)
                .text_color(theme::text_muted())
                .child("Press any button on a controller to identify it."),
        )
        .child(
            div()
                .id("btn-settings-refresh-devices")
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .w_full()
                .py_2()
                .rounded_lg()
                .bg(theme::bg_panel())
                .border_1()
                .border_color(theme::border_color())
                .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.refresh_detected_gamepads(cx);
                }))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text_primary())
                        .child("Refresh devices"),
                ),
        )
        .into_any_element()
}

/// Screen 4: General Settings Panel.
fn render_general_panel(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let start_in_bp = win.app_settings.start_in_big_picture;

    div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        // Row 1: Start in Big Picture
        .child(render_setting_toggle_row(
            "btn-toggle-start-in-bp",
            "Start in Big Picture",
            "Open directly in the controller-friendly interface",
            start_in_bp,
            win.settings_focus_zone == 1 && win.settings_item_index == 0,
            |this, cx| {
                this.app_settings.start_in_big_picture = !this.app_settings.start_in_big_picture;
                let _ = this.app_settings.save();
                this.show_osd(
                    if this.app_settings.start_in_big_picture {
                        "Start in Big Picture enabled"
                    } else {
                        "Start in Big Picture disabled"
                    },
                    1200,
                    cx,
                );
                cx.notify();
            },
            cx,
        ))
        // Row 2: Interface scale
        .child(render_setting_nav_row(
            "btn-setting-interface-scale",
            "Interface scale",
            "Larger controls and text for viewing from the couch",
            win.app_settings.interface_scale.clone(),
            win.settings_focus_zone == 1 && win.settings_item_index == 1,
            |this, cx| {
                this.cycle_interface_scale_preference(cx);
            },
            cx,
        ))
        // Row 3: Preferred display
        .child(render_setting_nav_row(
            "btn-setting-preferred-display",
            "Preferred display",
            "Where new sessions open by default",
            format!("Display {}", win.app_settings.preferred_display + 1),
            win.settings_focus_zone == 1 && win.settings_item_index == 2,
            |this, cx| {
                this.cycle_preferred_display_preference(cx);
            },
            cx,
        ))
        // Row 4: Interface sounds
        .child(render_setting_toggle_row(
            "btn-toggle-interface-sounds",
            "Interface sounds",
            "Play a sound when moving focus or confirming",
            win.app_settings.interface_sounds,
            win.settings_focus_zone == 1 && win.settings_item_index == 3,
            |this, cx| {
                this.app_settings.interface_sounds = !this.app_settings.interface_sounds;
                let _ = this.app_settings.save();
                this.show_osd(
                    if this.app_settings.interface_sounds {
                        "Interface sounds enabled"
                    } else {
                        "Interface sounds muted"
                    },
                    1200,
                    cx,
                );
                cx.notify();
            },
            cx,
        ))
        // Row 5: Language
        .child(render_setting_nav_row(
            "btn-setting-language",
            "Language",
            "Language used for the Nucleus interface",
            win.app_settings.language.clone(),
            win.settings_focus_zone == 1 && win.settings_item_index == 4,
            |this, cx| {
                this.cycle_language_preference(cx);
            },
            cx,
        ))
        // Row 6: Check for updates
        .child(render_setting_toggle_row(
            "btn-toggle-check-updates",
            "Check for updates",
            "Keep Nucleus and game handlers up to date",
            win.app_settings.check_for_updates,
            win.settings_focus_zone == 1 && win.settings_item_index == 5,
            |this, cx| {
                this.app_settings.check_for_updates = !this.app_settings.check_for_updates;
                let _ = this.app_settings.save();
                this.show_osd(
                    if this.app_settings.check_for_updates {
                        "Auto-updates enabled"
                    } else {
                        "Auto-updates disabled"
                    },
                    1200,
                    cx,
                );
                cx.notify();
            },
            cx,
        ))
        // Row 7: Developer mode
        .child(render_setting_toggle_row(
            "btn-toggle-developer-mode",
            "Developer Mode",
            "Enable developer tools, virtual gamepad injection, and advanced diagnostics",
            win.app_settings.developer_mode,
            win.settings_focus_zone == 1 && win.settings_item_index == 6,
            |this, cx| {
                this.app_settings.developer_mode = !this.app_settings.developer_mode;
                let _ = this.app_settings.save();
                this.show_osd(
                    if this.app_settings.developer_mode {
                        "Developer mode enabled"
                    } else {
                        "Developer mode disabled"
                    },
                    1200,
                    cx,
                );
                cx.notify();
            },
            cx,
        ))
        // Footer Note
        .child(
            div()
                .pt_2()
                .text_xs()
                .font_weight(FontWeight::NORMAL)
                .text_color(theme::text_muted())
                .child("Changes are saved automatically."),
        )
        .into_any_element()
}

/// Screen 5: Controllers Settings Panel.
fn render_controllers_panel(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let target_game = win
        .selected_game
        .clone()
        .unwrap_or_else(|| "Left 4 Dead 2".to_string());

    let devices = &win.detected_devices_list;
    let ignored_count = win.app_settings.ignored_device_guids.len();

    div()
        .flex()
        .flex_col()
        .gap_4()
        .w_full()
        // Row 1: Assignment method
        .child(render_setting_nav_row(
            "btn-setting-assignment-method",
            "Assignment method",
            "Choose how devices are matched to players.",
            "On button press",
            win.settings_focus_zone == 1 && win.settings_item_index == 0,
            |this, cx| {
                this.show_osd("Assignment method: On button press", 1500, cx);
                cx.notify();
            },
            cx,
        ))
        // Connected devices header
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("Connected devices"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text_muted())
                        .child(format!("Last setup: {}", target_game)),
                ),
        )
        // Connected devices list
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2p5()
                .children(devices.iter().enumerate().map(|(idx, dev)| {
                    let d_guid = dev.guid.clone();
                    let d_name = dev.name.clone();
                    let player_badge_text = match idx {
                        0 => "Player 1",
                        1 => "Player 2",
                        _ => "Available",
                    };
                    let player_badge_color = match idx {
                        0 => theme::accent(),
                        1 => rgb(0x3b82f6).into(),
                        _ => theme::text_muted(),
                    };
                    let is_active_identifying =
                        win.identifying_controller_guid.as_deref() == Some(&d_guid);
                    let is_card_focused =
                        win.settings_focus_zone == 1 && win.settings_item_index == idx + 1;

                    div()
                        .id(("settings-device-card", idx))
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_4()
                        .rounded_xl()
                        .bg(theme::bg_panel())
                        .border_2()
                        .border_color(if is_card_focused || is_active_identifying {
                            rgb(0x22c55e).into()
                        } else {
                            theme::border_subtle()
                        })
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child(d_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::text_muted())
                                        .child(d_guid.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_4()
                                // Player assignment tag
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .size(px(6.0))
                                                .rounded_full()
                                                .bg(player_badge_color),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(player_badge_color)
                                                .child(player_badge_text),
                                        ),
                                )
                                // Action: Test / identify >
                                .child(
                                    div()
                                        .id(("btn-identify-dev", idx))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(theme::bg_subtle())
                                        .hover(|s| {
                                            s.bg(theme::bg_active()).border_color(theme::accent())
                                        })
                                        .on_click({
                                            let guid_c = d_guid.clone();
                                            let name_c = d_name.clone();
                                            cx.listener(move |this, _, _, cx| {
                                                this.test_identify_controller(&guid_c, &name_c, cx);
                                            })
                                        })
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(if is_active_identifying {
                                                    theme::accent()
                                                } else {
                                                    theme::text_muted()
                                                })
                                                .child(if is_active_identifying {
                                                    "Identified!"
                                                } else {
                                                    "Test / identify"
                                                }),
                                        )
                                        .child(
                                            Icon::new(IconName::ChevronRight).size_3().text_color(
                                                if is_active_identifying {
                                                    theme::accent()
                                                } else {
                                                    theme::text_muted()
                                                },
                                            ),
                                        ),
                                ),
                        )
                })),
        )
        // Ignored devices footer row
        .child({
            let is_ignored_focused =
                win.settings_focus_zone == 1 && win.settings_item_index == devices.len() + 1;
            div()
                .flex()
                .items_center()
                .justify_between()
                .p_4()
                .rounded_xl()
                .bg(theme::bg_panel())
                .border_2()
                .border_color(if is_ignored_focused {
                    rgb(0x22c55e).into()
                } else {
                    theme::border_subtle()
                })
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text_muted())
                        .child("Ignored devices"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text_muted())
                                .child(format!("{} devices", ignored_count)),
                        )
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .size_3()
                                .text_color(theme::text_muted()),
                        ),
                )
        })
        .into_any_element()
}

fn render_setting_toggle_row<F>(
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    is_on: bool,
    has_focus_ring: bool,
    on_click_action: F,
    cx: &mut Context<MainWindow>,
) -> AnyElement
where
    F: Fn(&mut MainWindow, &mut Context<MainWindow>) + Send + Sync + 'static,
{
    div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .py_2p5()
        .px_4()
        .rounded_xl()
        .bg(theme::bg_panel())
        .border_2()
        .border_color(if has_focus_ring {
            rgb(0x22c55e).into()
        } else {
            theme::border_subtle()
        })
        .hover(|s| s.bg(theme::bg_active()).border_color(theme::border_color()))
        .on_click(cx.listener(move |this, _, _, cx| {
            on_click_action(this, cx);
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme::text_muted())
                        .child(subtitle),
                ),
        )
        // Toggle pill switch
        .child(
            div()
                .relative()
                .w(px(44.0))
                .h(px(24.0))
                .rounded_full()
                .bg(if is_on {
                    theme::accent()
                } else {
                    theme::bg_subtle()
                })
                .child(
                    div()
                        .absolute()
                        .top(px(2.0))
                        .left(if is_on { px(22.0) } else { px(2.0) })
                        .size(px(20.0))
                        .rounded_full()
                        .bg(theme::text_on_accent()),
                ),
        )
        .into_any_element()
}

fn render_setting_nav_row<F>(
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    value_label: impl Into<SharedString>,
    has_focus_ring: bool,
    on_click_action: F,
    cx: &mut Context<MainWindow>,
) -> AnyElement
where
    F: Fn(&mut MainWindow, &mut Context<MainWindow>) + Send + Sync + 'static,
{
    let val_str: SharedString = value_label.into();
    div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .py_2p5()
        .px_4()
        .rounded_xl()
        .bg(theme::bg_panel())
        .border_2()
        .border_color(if has_focus_ring {
            rgb(0x22c55e).into()
        } else {
            theme::border_subtle()
        })
        .hover(|s| s.bg(theme::bg_active()).border_color(theme::border_color()))
        .on_click(cx.listener(move |this, _, _, cx| {
            on_click_action(this, cx);
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme::text_muted())
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::accent())
                        .child(val_str),
                )
                .child(
                    Icon::new(IconName::ChevronRight)
                        .size_3p5()
                        .text_color(theme::accent()),
                ),
        )
        .into_any_element()
}
