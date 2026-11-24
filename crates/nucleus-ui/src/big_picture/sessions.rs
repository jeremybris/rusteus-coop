//! Screens 2 & 3: Session Controls (Active) & Saved Profiles (Idle) for 10-foot Couch Big Picture mode.
//!
//! Provides:
//! - When a game is running: Active Session Controls with live elapsed timer, connected players,
//!   Resume game, Player devices, Guide, Save profile, and End session.
//! - When idle: Saved Profiles catalog with "Load profile ->", "+ Save current setup", and setup guidance.

use gpui::prelude::*;
use gpui::*;
use gpui_component::Icon;
use gpui_kit_assets::IconName;

use crate::big_picture::overlay::format_duration_hhmmss;
use crate::theme;
use crate::window::MainWindow;

/// Renders the complete Sessions screen (Screen 2 if session is active, Screen 3 if idle).
pub fn render_sessions_view(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    if win.session_active {
        render_active_session_controls(win, cx)
    } else {
        render_saved_profiles_browser(win, cx)
    }
}

/// Screen 2: Active Session Controls view.
fn render_active_session_controls(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let target_game = win
        .selected_game
        .clone()
        .unwrap_or_else(|| "Active Game".to_string());

    let handler_opt = win.handlers.iter().find(|h| {
        h.game_name.eq_ignore_ascii_case(&target_game) || h.guid.eq_ignore_ascii_case(&target_game)
    });

    let cover_url_opt = handler_opt.and_then(|h| h.cover_image_url.clone());
    let player_count = win.couch_setup_state.slots.len().max(2);
    let layout_name = &win.selected_layout;

    let elapsed_secs = win.in_game_osd.elapsed_seconds;
    let formatted_time = format_duration_hhmmss(elapsed_secs);

    div()
        .id("big-picture-sessions-active")
        .flex()
        .flex_col()
        .size_full()
        .px_10()
        .py_6()
        .pb_16()
        .overflow_y_scroll()
        .bg(theme::bg_main())
        .text_color(theme::text_primary())
        // Header
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .pb_6()
                .child(
                    div()
                        .text_3xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("Session controls"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text_muted())
                        .child("Your game keeps running while this menu is open."),
                ),
        )
        // Two-Column Grid: Left Hero Card (Session Running) & Right Action Card
        .child(
            div()
                .flex()
                .gap_6()
                .w_full()
                // Left Hero Session Card
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .rounded_xl()
                        .overflow_hidden()
                        .bg(theme::bg_panel())
                        .border_1()
                        .border_color(theme::border_subtle())
                        // Artwork Top
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(px(260.0))
                                .children(cover_url_opt.as_deref().map(|url| {
                                    img(crate::make_image_source(url))
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Cover)
                                }))
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(theme::bg_main().opacity(0.65)),
                                )
                                .child(
                                    div()
                                        .relative()
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .size_full()
                                        .p_6()
                                        // Badge
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .px_3()
                                                .py_1()
                                                .rounded_full()
                                                .bg(theme::bg_main().opacity(0.85))
                                                .border_1()
                                                .border_color(theme::success())
                                                .child(
                                                    div()
                                                        .size(px(8.0))
                                                        .rounded_full()
                                                        .bg(theme::success()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(theme::success())
                                                        .child("SESSION RUNNING"),
                                                ),
                                        )
                                        // Title & Specs
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
                                                        .child(target_game),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme::text_muted())
                                                        .child(format!(
                                                            "{} players · {} · Display 1",
                                                            player_count, layout_name
                                                        )),
                                                ),
                                        ),
                                ),
                        )
                        // Metrics Bottom Bar
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .p_5()
                                .bg(theme::bg_panel())
                                .border_t_1()
                                .border_color(theme::border_subtle())
                                // Timer Metric
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child(formatted_time),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::text_muted())
                                                .child("Session time"),
                                        ),
                                )
                                // Player Status Dots
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .size(px(6.0))
                                                        .rounded_full()
                                                        .bg(theme::accent()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme::text_primary())
                                                        .child("Player 1 connected"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .size(px(6.0))
                                                        .rounded_full()
                                                        .bg(rgb(0x3b82f6)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme::text_primary())
                                                        .child("Player 2 connected"),
                                                ),
                                        ),
                                )
                                // Saved Profile Metric
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
                                                .child("Couch co-op"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::text_muted())
                                                .child("Saved profile"),
                                        ),
                                ),
                        ),
                )
                // Right Action Card
                .child(
                    div()
                        .w(px(380.0))
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_6()
                        .rounded_xl()
                        .bg(theme::bg_panel())
                        .border_1()
                        .border_color(theme::border_subtle())
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
                                        .child("Take a moment."),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(theme::text_muted())
                                        .child("Then get back to the co-op."),
                                ),
                        )
                        // Action 1: "Resume game" (large primary green button)
                        .child({
                            let is_focused = win.sessions_focus_index == 0;
                            div()
                                .id("btn-sessions-resume-game")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .w_full()
                                .py_3()
                                .rounded_lg()
                                .bg(theme::accent())
                                .border_2()
                                .border_color(if is_focused {
                                    rgb(0xffffff).into()
                                } else {
                                    theme::accent()
                                })
                                .hover(|s| s.opacity(0.92))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.resume_running_session_windows(cx);
                                }))
                                .child(
                                    Icon::new(IconName::Play)
                                        .size_4()
                                        .text_color(theme::text_on_accent()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_on_accent())
                                        .child("Resume game"),
                                )
                        })
                        // Action 2: "Player devices"
                        .child(render_session_action_btn(
                            "btn-sessions-player-devices",
                            "Player devices",
                            win.sessions_focus_index == 1,
                            |this, cx| {
                                this.in_game_osd.open_reassign(0);
                                this.set_osd_visible(true);
                                cx.notify();
                            },
                            cx,
                        ))
                        // Action 3: "Read first / Game guide"
                        .child(render_session_action_btn(
                            "btn-sessions-game-guide",
                            "Read first / Game guide",
                            win.sessions_focus_index == 2,
                            |this, cx| {
                                this.big_picture_screen =
                                    crate::big_picture::BigPictureScreen::GameDetails;
                                cx.notify();
                            },
                            cx,
                        ))
                        // Action 4: "Save session profile"
                        .child(render_session_action_btn(
                            "btn-sessions-save-profile",
                            "Save session profile",
                            win.sessions_focus_index == 3,
                            |this, cx| {
                                this.save_current_session_profile(cx);
                            },
                            cx,
                        ))
                        // Action 5: "End session..." (danger red button)
                        .child({
                            let is_focused = win.sessions_focus_index == 4;
                            div()
                                .id("btn-sessions-end-session")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w_full()
                                .py_2p5()
                                .rounded_lg()
                                .bg(theme::bg_subtle())
                                .border_2()
                                .border_color(if is_focused {
                                    rgb(0x22c55e).into()
                                } else {
                                    theme::danger().opacity(0.40)
                                })
                                .hover(|s| {
                                    s.bg(theme::danger().opacity(0.12))
                                        .border_color(theme::danger())
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stop_active_session_cleanly(cx);
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme::danger())
                                        .child("End session..."),
                                )
                        }),
                ),
        )
        .into_any_element()
}

/// Screen 3: Saved Profiles Browser view (Idle state).
fn render_saved_profiles_browser(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let saved_profiles = win.get_all_saved_profiles_items();

    div()
        .id("big-picture-saved-profiles")
        .flex()
        .flex_col()
        .size_full()
        .px_10()
        .py_6()
        .pb_16()
        .overflow_y_scroll()
        .bg(theme::bg_main())
        .text_color(theme::text_primary())
        // Header Row: Title & Subtitle + "+ Save current setup"
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
                                .child("Saved profiles"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text_muted())
                                .child("Your couch, your rules. Keep a setup for every game night."),
                        ),
                )
                .child({
                    let is_focused = win.sessions_focus_index == 0;
                    div()
                        .id("btn-sessions-save-current-setup")
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
                            this.save_current_session_profile(cx);
                        }))
                        .child(
                            Icon::new(IconName::Plus)
                                .size_3p5()
                                .text_color(theme::text_primary()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text_primary())
                                .child("Save current setup"),
                        )
                }),
        )
        // Saved Profiles Grid
        .child(
            div()
                .flex()
                .gap_6()
                .w_full()
                .flex_wrap()
                .children(saved_profiles.into_iter().enumerate().map(|(idx, prof)| {
                    let p_num = prof.profile_number;
                    let g_guid = prof.game_guid.clone();
                    let g_title = prof.game_title.clone();
                    let p_title = prof.profile_title.clone();
                    let p_layout = prof.layout.clone();
                    let p_players = prof.player_count;
                    let p_cover = prof.cover_url.clone();
                    let is_card_focused = win.sessions_focus_index == idx + 1;
                    let is_first = idx == 0;

                    div()
                        .id(("saved-profile-card", idx))
                        .flex_1()
                        .min_w(px(280.0))
                        .max_w(px(360.0))
                        .flex()
                        .flex_col()
                        .rounded_xl()
                        .overflow_hidden()
                        .bg(theme::bg_panel())
                        .border_2()
                        .border_color(if is_card_focused {
                            rgb(0x22c55e).into()
                        } else {
                            theme::border_subtle()
                        })
                        // Artwork Top
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(px(140.0))
                                .bg(theme::bg_subtle())
                                .children(p_cover.as_ref().map(|url| {
                                    img(crate::make_image_source(url))
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Cover)
                                }))
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(theme::bg_main().opacity(0.35)),
                                ),
                        )
                        // Content Bottom
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_4()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child(p_title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::text_muted())
                                        .child(g_title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(theme::text_muted())
                                        .child(format!("{} players · {}", p_players, p_layout)),
                                )
                                .child(
                                    div()
                                        .pt_2()
                                        .child(
                                            div()
                                                .id(("btn-load-profile", idx))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .gap_2()
                                                .w_full()
                                                .py_2()
                                                .rounded_lg()
                                                .bg(if is_first {
                                                    theme::accent()
                                                } else {
                                                    theme::bg_subtle()
                                                })
                                                .border_1()
                                                .border_color(if is_first {
                                                    theme::accent()
                                                } else {
                                                    theme::border_color()
                                                })
                                                .hover(|s| {
                                                    if is_first {
                                                        s.opacity(0.92)
                                                    } else {
                                                        s.bg(theme::bg_active()).border_color(theme::accent())
                                                    }
                                                })
                                                .on_click({
                                                    let guid = g_guid.clone();
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.load_saved_profile_by_number(&guid, p_num, cx);
                                                    })
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(if is_first {
                                                            theme::text_on_accent()
                                                        } else {
                                                            theme::text_primary()
                                                        })
                                                        .child("Load profile"),
                                                )
                                                .child(
                                                    Icon::new(IconName::ArrowRight)
                                                        .size_3()
                                                        .text_color(if is_first {
                                                            theme::text_on_accent()
                                                        } else {
                                                            theme::text_muted()
                                                        }),
                                                ),
                                        ),
                                ),
                        )
                })),
        )
        // Bottom Informational Card
        .child(
            div()
                .mt_8()
                .p_5()
                .rounded_xl()
                .bg(theme::bg_panel())
                .border_1()
                .border_color(theme::border_subtle())
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("Pick up where everyone left off."),
                )
                .child(
                    div()
                        .pt_1()
                        .text_xs()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme::text_muted())
                        .child("Profiles remember your players, controller assignments, screen layout and game options."),
                ),
        )
        .into_any_element()
}

fn render_session_action_btn<F>(
    id: &'static str,
    label: &'static str,
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
        .justify_center()
        .w_full()
        .py_2p5()
        .rounded_lg()
        .bg(theme::bg_subtle())
        .border_2()
        .border_color(if has_focus_ring {
            rgb(0x22c55e).into()
        } else {
            theme::border_color()
        })
        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
        .on_click(cx.listener(move |this, _, _, cx| {
            on_click_action(this, cx);
        }))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text_primary())
                .child(label),
        )
        .into_any_element()
}
