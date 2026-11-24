//! Screen 7: Game Details for 10-foot Couch Big Picture mode.
//!
//! Provides a dedicated game briefing view matching the console dashboard design:
//! - Breadcrumb: `LIBRARY / GAME DETAILS`
//! - Left Hero: Game artwork banner, title, player count and tags
//! - Left Guide: `Read first` setup instructions
//! - Right Top: `READY TO PLAY` card with primary "Set up session" and "Load saved profile"
//! - Right Bottom: `LAST SESSION` card with play time and "Manage game ->" shortcut

use gpui::prelude::*;
use gpui::*;
use gpui_component::Icon;
use gpui_kit_assets::IconName;

use crate::big_picture::overlay::format_duration_hhmmss;
use crate::theme;
use crate::window::MainWindow;

/// Renders the complete Game Details screen (Screen 7).
pub fn render_game_details_view(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
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

    let cover_url_opt = handler_opt.and_then(|h| h.cover_image_url.clone());
    let max_players = handler_opt.map(|h| h.max_players).unwrap_or(4);
    let min_players = 2;

    let handler_notes = handler_opt
        .map(|h| h.description.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            "Host a multiplayer game in the first instance. Other players can join through the in-game LAN or lobby browser. Use a mouse if gamepad menus do not respond.".to_string()
        });

    let play_time_secs = handler_opt.map(|h| h.play_time_seconds).unwrap_or(0);
    let last_played_str = handler_opt
        .and_then(|h| h.last_played_str.clone())
        .unwrap_or_else(|| "Never played".to_string());

    let formatted_duration = format_duration_hhmmss(play_time_secs);

    div()
        .id("big-picture-game-details")
        .flex()
        .flex_col()
        .size_full()
        .px_10()
        .py_4()
        .bg(theme::bg_main())
        .text_color(theme::text_primary())
        // Breadcrumb Title
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .pb_4()
                .child(
                    div()
                        .id("breadcrumb-library")
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if win.details_focus_index == 3 { theme::accent() } else { theme::text_muted() })
                        .when(win.details_focus_index == 3, |s| s.underline())
                        .hover(|s| s.text_color(theme::accent()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.big_picture_screen = crate::big_picture::BigPictureScreen::Carousel;
                            cx.notify();
                        }))
                        .child("LIBRARY"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_muted())
                        .child("/"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent())
                        .child("GAME DETAILS"),
                ),
        )
        // Two-Column Grid: Left Column (Banner + Read First) & Right Column (Actions + Tools)
        .child(
            div()
                .flex()
                .gap_6()
                .w_full()
                // Left Column
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_4()
                        // Hero Banner Card
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(px(180.0))
                                .rounded_xl()
                                .overflow_hidden()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                // Cover art background with dark gradient overlay
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
                                        .bg(theme::bg_main().opacity(0.70)),
                                )
                                // Text Information on Banner
                                .child(
                                    div()
                                        .relative()
                                        .flex()
                                        .flex_col()
                                        .justify_end()
                                        .size_full()
                                        .p_4()
                                        .child(
                                            div()
                                                .text_2xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child(game_title.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::text_muted())
                                                .child(format!(
                                                    "{}-{} players · Splitscreen co-op",
                                                    min_players, max_players
                                                )),
                                        ),
                                 ),
                        )
                        // "Read first" Card
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_4()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Icon::new(IconName::BookOpen)
                                                .size_4()
                                                .text_color(theme::accent()),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child("Read first"),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("details-handler-notes-scroll")
                                        .max_h(px(160.0))
                                        .overflow_y_scroll()
                                        .pr_1()
                                        .text_xs()
                                        .font_weight(FontWeight::NORMAL)
                                        .line_height(relative(1.4))
                                        .text_color(theme::text_muted())
                                        .child(handler_notes),
                                ),
                        ),
                )
                // Right Column
                .child(
                    div()
                        .w(px(380.0))
                        .flex()
                        .flex_col()
                        .gap_4()
                        // Right Top: "READY TO PLAY" Card
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2p5()
                                .p_4()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .bg(theme::accent().opacity(0.16))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::accent())
                                                .child("READY TO PLAY"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Make room for player two."),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(theme::text_muted())
                                        .child("Choose your players and split the screen. Your last setup is ready to use."),
                                )
                                // Action 1: "Set up session" (large primary green button)
                                .child({
                                    let game_for_setup = game_title.clone();
                                    let is_focused = win.details_focus_index == 0;
                                    div()
                                        .id("btn-details-setup-session")
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
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.enter_couch_setup(&game_for_setup, cx);
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
                                                .child("Set up session"),
                                        )
                                })
                                // Action 2: "Load saved profile"
                                .child({
                                    let is_focused = win.details_focus_index == 1;
                                    div()
                                        .id("btn-details-load-saved-profile")
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
                                            theme::border_color()
                                        })
                                        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.load_last_saved_profile_for_selected(cx);
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme::text_primary())
                                                .child("Load saved profile"),
                                        )
                                }),
                        )
                        // Right Bottom: "LAST SESSION" Card
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2p5()
                                .p_4()
                                .rounded_xl()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_muted())
                                                .child("LAST SESSION"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme::text_primary())
                                                .child(format!("{} · {}", last_played_str, formatted_duration)),
                                        ),
                                )
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
                                                .child("Handler and game tools"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::NORMAL)
                                                .text_color(theme::text_muted())
                                                .child("Files, assets, language and preferences"),
                                        ),
                                )
                                // Action: "Manage game ->"
                                .child({
                                    let is_focused = win.details_focus_index == 2;
                                    div()
                                        .id("btn-details-manage-game")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .w_full()
                                        .py_2p5()
                                        .rounded_lg()
                                        .bg(theme::bg_subtle())
                                        .border_2()
                                        .border_color(if is_focused {
                                            rgb(0x22c55e).into()
                                        } else {
                                            theme::border_color()
                                        })
                                        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.big_picture_screen = crate::big_picture::BigPictureScreen::GameManagement;
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme::text_primary())
                                                .child("Manage game"),
                                        )
                                        .child(
                                            Icon::new(IconName::ArrowRight)
                                                .size_3p5()
                                                .text_color(theme::text_muted()),
                                        )
                                }),
                        ),
                ),
        )
        .into_any_element()
}
