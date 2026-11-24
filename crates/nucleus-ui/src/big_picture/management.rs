//! Screen 1: Game Management (Tools and preferences) for 10-foot Couch Big Picture mode.
//!
//! Provides a 2x2 matrix matching the console dashboard design:
//! 1. Game files (Open game handler, Open original executable path, Open UserProfile save path)
//! 2. Game preferences (Assign gamepads segmented toggle, Steam language combobox, Game assets browse)
//! 3. Make it yours (Change game icon with async file dialog)
//! 4. Remove from library (Remove game from list with confirmation)

use gpui::prelude::*;
use gpui::*;
use gpui_component::Icon;
use gpui_kit_assets::IconName;

use crate::theme;
use crate::window::MainWindow;

/// Renders the complete Game Management screen (Screen 1).
pub fn render_game_management_view(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let target_game = match win.selected_game.as_ref() {
        Some(g) => g.clone(),
        None => "Unknown Game".to_string(),
    };

    let handler_opt = win.handlers.iter().find(|h| {
        h.game_name.eq_ignore_ascii_case(&target_game) || h.guid.eq_ignore_ascii_case(&target_game)
    });

    let game_title = handler_opt
        .map(|h| h.game_name.clone())
        .unwrap_or_else(|| target_game.clone());

    let exe_path_opt = handler_opt.and_then(|h| h.executable_path.clone());
    let handler_guid = handler_opt.map(|h| h.guid.clone()).unwrap_or_default();
    let handler_path_opt = handler_opt.and_then(|h| h.handler_path.clone());

    // Check if custom gamepad assignment mode is by index or on button press
    let is_on_press = win
        .app_settings
        .custom_preferences
        .get(&format!("{}:gamepad_mode", handler_guid))
        .map(|v| v == "on_button_press")
        .unwrap_or(true);

    let current_language = win
        .app_settings
        .custom_preferences
        .get(&format!("{}:steam_language", handler_guid))
        .cloned()
        .unwrap_or_else(|| "English".to_string());

    div()
        .id("big-picture-game-management")
        .flex()
        .flex_col()
        .size_full()
        .px_10()
        .py_6()
        .pb_16()
        .overflow_y_scroll()
        .bg(theme::bg_main())
        .text_color(theme::text_primary())
        // Header Row: Title & Subtitle + "Back to game ->"
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .w_full()
                .pb_6()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child("Game management"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text_muted())
                                .child(format!("{} · Tools and preferences", game_title)),
                        ),
                )
                .child({
                    let is_focused = win.management_focus_index == 8;
                    div()
                        .id("btn-mgmt-back-to-game")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_4()
                        .py_2()
                        .rounded_lg()
                        .bg(theme::bg_panel())
                        .border_2()
                        .border_color(if is_focused {
                            rgb(0x22c55e).into()
                        } else {
                            theme::border_color()
                        })
                        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.big_picture_screen =
                                crate::big_picture::BigPictureScreen::GameDetails;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text_primary())
                                .child("Back to game"),
                        )
                        .child(
                            Icon::new(IconName::ArrowRight)
                                .size_3p5()
                                .text_color(theme::text_muted()),
                        )
                }),
        )
        // 2x2 Panels Grid
        .child(
            div()
                .flex()
                .flex_col()
                .gap_6()
                .w_full()
                // Top Row: Game Files (Left) & Game Preferences (Right)
                .child(
                    div()
                        .flex()
                        .gap_6()
                        .w_full()
                        // 1. Game files panel
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .p_5()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Game files"),
                                )
                                // Item 1: Open game handler
                                .child(render_file_action_row(
                                    "btn-mgmt-open-handler",
                                    "Open game handler",
                                    "Review the handler used for this game",
                                    win.management_focus_index == 0,
                                    {
                                        let h_path = handler_path_opt.clone();
                                        let h_guid = handler_guid.clone();
                                        let g_name = game_title.clone();
                                        move |this, cx| {
                                            this.open_game_handler_file(
                                                h_path.as_deref(),
                                                &h_guid,
                                                &g_name,
                                                cx,
                                            );
                                        }
                                    },
                                    cx,
                                ))
                                // Item 2: Open original executable path
                                .child(render_file_action_row(
                                    "btn-mgmt-open-exe",
                                    "Open original executable path",
                                    "Find the installed game on this PC",
                                    win.management_focus_index == 2,
                                    {
                                        let exe_p = exe_path_opt.clone();
                                        let g_name = game_title.clone();
                                        move |this, cx| {
                                            this.open_game_exe_path(exe_p.as_deref(), &g_name, cx);
                                        }
                                    },
                                    cx,
                                ))
                                // Item 3: Open UserProfile save path
                                .child(render_file_action_row(
                                    "btn-mgmt-open-save-path",
                                    "Open UserProfile save path",
                                    "Access saves for your local players",
                                    win.management_focus_index == 4,
                                    {
                                        let g_name = game_title.clone();
                                        move |this, cx| {
                                            this.open_game_userprofile_path(&g_name, cx);
                                        }
                                    },
                                    cx,
                                )),
                        )
                        // 2. Game preferences panel
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .p_5()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Game preferences"),
                                )
                                // Row 1: Assign gamepads segmented control
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::text_muted())
                                                .child("Assign gamepads"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .p_0p5()
                                                .rounded_lg()
                                                .border_2()
                                                .border_color(if win.management_focus_index == 1 {
                                                    rgb(0x22c55e).into()
                                                } else {
                                                    gpui::transparent_black()
                                                })
                                                .child(render_segmented_btn(
                                                    "btn-mgmt-assign-index",
                                                    "By index",
                                                    !is_on_press,
                                                    {
                                                        let guid = handler_guid.clone();
                                                        move |this, cx| {
                                                            this.set_gamepad_mode_preference(
                                                                &guid, "by_index", cx,
                                                            );
                                                        }
                                                    },
                                                    cx,
                                                ))
                                                .child(render_segmented_btn(
                                                    "btn-mgmt-assign-button",
                                                    "On button press",
                                                    is_on_press,
                                                    {
                                                        let guid = handler_guid.clone();
                                                        move |this, cx| {
                                                            this.set_gamepad_mode_preference(
                                                                &guid,
                                                                "on_button_press",
                                                                cx,
                                                            );
                                                        }
                                                    },
                                                    cx,
                                                )),
                                        ),
                                )
                                // Row 2: Steam language
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .pt_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::text_primary())
                                                .child("Steam language"),
                                        )
                                        .child(
                                            div()
                                                .id("btn-mgmt-cycle-language")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .px_3()
                                                .py_1()
                                                .rounded_md()
                                                .bg(theme::bg_subtle())
                                                .border_2()
                                                .border_color(if win.management_focus_index == 3 {
                                                    rgb(0x22c55e).into()
                                                } else {
                                                    theme::border_subtle()
                                                })
                                                .hover(|s| s.bg(theme::bg_active()))
                                                .on_click({
                                                    let guid = handler_guid.clone();
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.cycle_steam_language_preference(
                                                            &guid, cx,
                                                        );
                                                    })
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(theme::accent())
                                                        .child(current_language.clone()),
                                                )
                                                .child(
                                                    Icon::new(IconName::ChevronRight)
                                                        .size_3()
                                                        .text_color(theme::accent()),
                                                ),
                                        ),
                                )
                                // Row 3: Game assets
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .pt_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::text_primary())
                                                .child("Game assets"),
                                        )
                                        .child(
                                            div()
                                                .id("btn-mgmt-browse-assets")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .px_3()
                                                .py_1()
                                                .rounded_md()
                                                .bg(theme::bg_subtle())
                                                .border_2()
                                                .border_color(if win.management_focus_index == 5 {
                                                    rgb(0x22c55e).into()
                                                } else {
                                                    theme::border_subtle()
                                                })
                                                .hover(|s| s.bg(theme::bg_active()))
                                                .on_click({
                                                    let g_name = game_title.clone();
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.open_game_assets_folder(&g_name, cx);
                                                    })
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(theme::text_muted())
                                                        .child("Browse"),
                                                )
                                                .child(
                                                    Icon::new(IconName::ChevronRight)
                                                        .size_3()
                                                        .text_color(theme::text_muted()),
                                                ),
                                        ),
                                ),
                        ),
                )
                // Bottom Row: Make it yours (Left) & Remove from library (Right)
                .child(
                    div()
                        .flex()
                        .gap_6()
                        .w_full()
                        // 3. Make it yours panel
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_5()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Make it yours"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::text_muted())
                                        .child("Change the icon shown in your library."),
                                )
                                .child(
                                    div().pt_2().child(
                                        div()
                                            .id("btn-mgmt-change-icon")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w_full()
                                            .py_2p5()
                                            .rounded_lg()
                                            .bg(theme::bg_subtle())
                                            .border_2()
                                            .border_color(if win.management_focus_index == 6 {
                                                rgb(0x22c55e).into()
                                            } else {
                                                theme::border_color()
                                            })
                                            .hover(|s| {
                                                s.bg(theme::bg_active())
                                                    .border_color(theme::accent())
                                            })
                                            .on_click({
                                                let g_guid = handler_guid.clone();
                                                let g_name = game_title.clone();
                                                cx.listener(move |this, _, _, cx| {
                                                    this.trigger_change_game_icon_async(
                                                        &g_guid, &g_name, cx,
                                                    );
                                                })
                                            })
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::text_primary())
                                                    .child("Change game icon"),
                                            ),
                                    ),
                                ),
                        )
                        // 4. Remove from library panel
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_5()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Remove from library"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::text_muted())
                                        .child(
                                            "Your installed game and save data stay on this PC.",
                                        ),
                                )
                                .child(
                                    div().pt_2().child(
                                        div()
                                            .id("btn-mgmt-remove-game")
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w_full()
                                            .py_2p5()
                                            .rounded_lg()
                                            .bg(theme::bg_subtle())
                                            .border_2()
                                            .border_color(if win.management_focus_index == 7 {
                                                rgb(0x22c55e).into()
                                            } else {
                                                theme::danger().opacity(0.40)
                                            })
                                            .hover(|s| {
                                                s.bg(theme::danger().opacity(0.12))
                                                    .border_color(theme::danger())
                                            })
                                            .on_click({
                                                let g_name = game_title.clone();
                                                let g_guid = handler_guid.clone();
                                                cx.listener(move |this, _, _, cx| {
                                                    this.prompt_remove_game_confirmation(
                                                        &g_guid, &g_name, cx,
                                                    );
                                                })
                                            })
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::danger())
                                                    .child("Remove game from list..."),
                                            ),
                                    ),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

fn render_file_action_row<F>(
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    has_focus_ring: bool,
    on_click_action: F,
    cx: &mut Context<MainWindow>,
) -> AnyElement
where
    F: Fn(&mut MainWindow, &mut Context<MainWindow>) + Send + Sync + 'static,
{
    let border_color = if has_focus_ring {
        theme::accent()
    } else {
        theme::border_subtle()
    };

    div()
        .id(id)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_between()
        .p_3()
        .rounded_lg()
        .bg(if has_focus_ring {
            theme::accent().opacity(0.08)
        } else {
            theme::bg_subtle()
        })
        .border_1()
        .border_color(border_color)
        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
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
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
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
            Icon::new(IconName::ArrowUpRight)
                .size_3p5()
                .text_color(if has_focus_ring {
                    theme::accent()
                } else {
                    theme::text_muted()
                }),
        )
        .into_any_element()
}

fn render_segmented_btn<F>(
    id: &'static str,
    label: &'static str,
    is_active: bool,
    on_click_action: F,
    cx: &mut Context<MainWindow>,
) -> AnyElement
where
    F: Fn(&mut MainWindow, &mut Context<MainWindow>) + Send + Sync + 'static,
{
    div()
        .id(id)
        .cursor_pointer()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .py_1p5()
        .rounded_md()
        .bg(if is_active {
            theme::accent()
        } else {
            theme::bg_subtle()
        })
        .hover(|s| {
            if !is_active {
                s.bg(theme::bg_active())
            } else {
                s
            }
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            on_click_action(this, cx);
        }))
        .child(
            div()
                .text_xs()
                .font_weight(if is_active {
                    FontWeight::BOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if is_active {
                    theme::text_on_accent()
                } else {
                    theme::text_primary()
                })
                .child(label),
        )
        .into_any_element()
}
