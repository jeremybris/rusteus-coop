//! Big Picture library carousel screen for 10-foot television interface.
//!
//! Provides horizontal carousel browsing of installed game handlers, oversized
//! cover cards with focus highlight/glow, metadata badges, couch button prompts,
//! and seamless desktop/couch mode switching.

use crate::components::{CouchButton, CouchButtonSize};
use crate::models::HandlerItem;
use crate::theme;
use crate::window::MainWindow;
use crate::{Icon, IconName};
use gpui::prelude::*;
use gpui::*;

/// Active screen within Couch Big Picture mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BigPictureScreen {
    /// Screen 6: 10-foot game library carousel and hero banner.
    #[default]
    Carousel,
    /// Screen 7: Game details with artwork, setup shortcut, last session stats, and manage game button.
    GameDetails,
    /// Screen 1: Game management (tools, preferences, icon changer, open paths, remove).
    GameManagement,
    /// Screens 9 & 10: Couch controller slot auto-assignment and monitor partition setup.
    CouchSetup,
    /// Screen 8: Couch community handler browser and drive scanner ("Add games").
    CouchHub,
    /// Screens 2 & 3: Active session controls (running) or Saved profiles browser (idle).
    Sessions,
    /// Screens 4 & 5: Big Picture settings (General and Controllers with test/identify).
    Settings,
}

/// The 4 universal top tabs for Big Picture mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BigPictureTab {
    #[default]
    Library,
    AddGames,
    Sessions,
    Settings,
}

impl BigPictureScreen {
    /// Returns the top-level tab corresponding to this screen.
    pub fn top_tab(&self) -> BigPictureTab {
        match self {
            Self::Carousel | Self::GameDetails | Self::GameManagement | Self::CouchSetup => {
                BigPictureTab::Library
            }
            Self::CouchHub => BigPictureTab::AddGames,
            Self::Sessions => BigPictureTab::Sessions,
            Self::Settings => BigPictureTab::Settings,
        }
    }

    /// Cycles to the next top-level Big Picture tab (Library -> Add games -> Sessions -> Settings -> Library).
    pub fn next(&self) -> Self {
        match self.top_tab() {
            BigPictureTab::Library => Self::CouchHub,
            BigPictureTab::AddGames => Self::Sessions,
            BigPictureTab::Sessions => Self::Settings,
            BigPictureTab::Settings => Self::Carousel,
        }
    }

    /// Cycles to the previous top-level Big Picture tab (Library -> Settings -> Sessions -> Add games -> Library).
    pub fn prev(&self) -> Self {
        match self.top_tab() {
            BigPictureTab::Library => Self::Settings,
            BigPictureTab::Settings => Self::Sessions,
            BigPictureTab::Sessions => Self::CouchHub,
            BigPictureTab::AddGames => Self::Carousel,
        }
    }
}

/// Returns the current local wall-clock time formatted as HH:MM.
pub fn current_wall_clock_time() -> String {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(lp_system_time: *mut SystemTime);
        }
        let mut st = SystemTime::default();
        unsafe { GetLocalTime(&mut st) };
        format!("{:02}:{:02}", st.hour, st.minute)
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let hours = (secs / 3600) % 24;
        let mins = (secs % 3600) / 60;
        format!("{:02}:{:02}", hours, mins)
    }
}

/// Filter mode for the 10-foot Big Picture library carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarouselFilterMode {
    /// All handlers without filtering.
    #[default]
    All,
    /// Only games with an executable verified on disk (`is_installed == true`).
    InstalledOnly,
    /// Only marked favorite games.
    Favorites,
    /// Games supporting controller/gamepad input.
    ControllerSupport,
    /// Games supporting single keyboard and mouse input.
    KeyboardOnly,
    /// Games supporting multiple keyboards and mice (ProtoInput/RawInput).
    MultiKeyboardMouse,
    /// Games supporting 4 or more players (`max_players >= 4`).
    FourPlusPlayers,
    /// Games supporting 2 or more players (`max_players >= 2`).
    TwoPlusPlayers,
}

impl CarouselFilterMode {
    /// Slice of all selectable filter modes in preferred presentation order.
    pub const ALL: &'static [CarouselFilterMode] = &[
        Self::All,
        Self::InstalledOnly,
        Self::Favorites,
        Self::ControllerSupport,
        Self::KeyboardOnly,
        Self::MultiKeyboardMouse,
        Self::FourPlusPlayers,
        Self::TwoPlusPlayers,
    ];

    /// User-facing label for the filter mode.
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All Games",
            Self::InstalledOnly => "Ready to Launch",
            Self::Favorites => "Favorites",
            Self::ControllerSupport => "Controllers",
            Self::KeyboardOnly => "Keyboard/Mouse",
            Self::MultiKeyboardMouse => "Multi-K&M (ProtoInput)",
            Self::FourPlusPlayers => "4+ Players",
            Self::TwoPlusPlayers => "2+ Players",
        }
    }

    /// Icon corresponding to this filter mode.
    pub fn icon(&self) -> gpui_kit_assets::IconName {
        match self {
            Self::All => gpui_kit_assets::IconName::LayoutGrid,
            Self::InstalledOnly => gpui_kit_assets::IconName::Check,
            Self::Favorites => gpui_kit_assets::IconName::Star,
            Self::ControllerSupport => gpui_kit_assets::IconName::Gamepad2,
            Self::KeyboardOnly => gpui_kit_assets::IconName::Keyboard,
            Self::MultiKeyboardMouse => gpui_kit_assets::IconName::Mouse,
            Self::FourPlusPlayers => gpui_kit_assets::IconName::Users,
            Self::TwoPlusPlayers => gpui_kit_assets::IconName::Users,
        }
    }
}

/// Sort ordering for the 10-foot Big Picture library carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarouselSortMode {
    /// Sorted alphabetically A-Z by game name.
    #[default]
    Alphabetical,
    /// Sorted alphabetically Z-A by game name.
    AlphabeticalDesc,
    /// Sorted descending by most recently played date.
    RecentlyPlayed,
    /// Sorted descending by accumulated total play time.
    PlayTime,
    /// Sorted descending by maximum supported player count.
    MaxPlayers,
    /// Installed games first, followed by uninstalled games.
    InstalledFirst,
}

impl CarouselSortMode {
    /// Slice of all selectable sort modes in presentation order.
    pub const ALL: &'static [CarouselSortMode] = &[
        Self::Alphabetical,
        Self::AlphabeticalDesc,
        Self::RecentlyPlayed,
        Self::PlayTime,
        Self::MaxPlayers,
        Self::InstalledFirst,
    ];

    /// User-facing label for the sort mode.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Alphabetical => "Alphabetical (A-Z)",
            Self::AlphabeticalDesc => "Alphabetical (Z-A)",
            Self::RecentlyPlayed => "Recently Played",
            Self::PlayTime => "Total Play Time",
            Self::MaxPlayers => "Maximum Players",
            Self::InstalledFirst => "Installed First",
        }
    }

    /// Icon corresponding to this sort mode.
    pub fn icon(&self) -> gpui_kit_assets::IconName {
        match self {
            Self::Alphabetical => gpui_kit_assets::IconName::ArrowDownAZ,
            Self::AlphabeticalDesc => gpui_kit_assets::IconName::ArrowDownAZ,
            Self::RecentlyPlayed => gpui_kit_assets::IconName::Clock,
            Self::PlayTime => gpui_kit_assets::IconName::Timer,
            Self::MaxPlayers => gpui_kit_assets::IconName::Users,
            Self::InstalledFirst => gpui_kit_assets::IconName::Check,
        }
    }
}

/// Vertical focus zone in the Carousel screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarouselFocusZone {
    HeaderTabs,
    HeroBanner(usize), // 0: "Set up session", 1: "Game details", 2: "Search library"
    #[default]
    Cards,
}

/// State tracking selected item index and total items in the horizontal carousel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CarouselState {
    /// Total count of game handler items in the carousel.
    pub items_count: usize,
    /// Currently focused/selected game index (0-based).
    pub selected_index: usize,
    /// Active filter mode for the carousel library.
    pub filter_mode: CarouselFilterMode,
    /// Active sort mode for the carousel library.
    pub sort_mode: CarouselSortMode,
    /// Active search query string.
    pub search_query: String,
    /// Whether the interactive search input bar is currently open.
    pub search_input_open: bool,
    /// Whether the filter options list picker popover is currently open.
    pub filter_picker_open: bool,
    /// Whether the sort options list picker popover is currently open.
    pub sort_picker_open: bool,
    /// Current vertical navigation focus zone.
    pub focus_zone: CarouselFocusZone,
}

impl CarouselState {
    /// Creates a new `CarouselState` with the specified item count.
    pub fn new(count: usize) -> Self {
        Self {
            items_count: count,
            selected_index: 0,
            filter_mode: CarouselFilterMode::All,
            sort_mode: CarouselSortMode::Alphabetical,
            search_query: String::new(),
            search_input_open: false,
            filter_picker_open: false,
            sort_picker_open: false,
            focus_zone: CarouselFocusZone::Cards,
        }
    }

    /// Moves focus vertically upward.
    pub fn move_up(&mut self) {
        match self.focus_zone {
            CarouselFocusZone::Cards => {
                self.focus_zone = CarouselFocusZone::HeroBanner(0);
            }
            CarouselFocusZone::HeroBanner(_) => {
                self.focus_zone = CarouselFocusZone::HeaderTabs;
            }
            CarouselFocusZone::HeaderTabs => {}
        }
    }

    /// Moves focus vertically downward.
    pub fn move_down(&mut self) {
        match self.focus_zone {
            CarouselFocusZone::HeaderTabs => {
                self.focus_zone = CarouselFocusZone::HeroBanner(0);
            }
            CarouselFocusZone::HeroBanner(_) => {
                self.focus_zone = CarouselFocusZone::Cards;
            }
            CarouselFocusZone::Cards => {}
        }
    }

    /// Moves selection to the left, wrapping from 0 to the last item.
    pub fn move_left(&mut self) {
        if self.items_count == 0 {
            self.selected_index = 0;
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.items_count.saturating_sub(1);
        } else {
            self.selected_index -= 1;
        }
    }

    /// Moves selection to the right, wrapping from the last item to 0.
    pub fn move_right(&mut self) {
        if self.items_count == 0 {
            self.selected_index = 0;
            return;
        }
        if self.selected_index + 1 >= self.items_count {
            self.selected_index = 0;
        } else {
            self.selected_index += 1;
        }
    }

    /// Sets the selected index directly, clamped to `items_count - 1`.
    pub fn select(&mut self, idx: usize) {
        if self.items_count == 0 {
            self.selected_index = 0;
            return;
        }
        let max_idx = self.items_count.saturating_sub(1);
        self.selected_index = idx.min(max_idx);
    }

    /// Returns the currently selected item index.
    pub fn selected(&self) -> usize {
        self.selected_index
    }

    /// Updates the total item count and ensures the selected index remains in bounds.
    pub fn set_count(&mut self, count: usize) {
        self.items_count = count;
        if self.items_count == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= self.items_count {
            self.selected_index = self.items_count.saturating_sub(1);
        }
    }

    /// Filters and sorts the given slice of `HandlerItem`s according to `filter_mode`, `sort_mode`, and `search_query`.
    pub fn filter_and_sort_handlers<'a>(
        &self,
        handlers: &'a [HandlerItem],
        search_query: &str,
    ) -> Vec<&'a HandlerItem> {
        let q = search_query.trim().to_lowercase();
        let mut list: Vec<&'a HandlerItem> = handlers
            .iter()
            .filter(|h| {
                // Search filter (if query is non-empty)
                if !q.is_empty() {
                    let name_match = h.game_name.to_lowercase().contains(&q);
                    let exe_match = h.executable_name.to_lowercase().contains(&q);
                    let title_match = h.handler_title.to_lowercase().contains(&q);
                    if !name_match && !exe_match && !title_match {
                        return false;
                    }
                }

                // Mode filter
                match self.filter_mode {
                    CarouselFilterMode::All => true,
                    CarouselFilterMode::InstalledOnly => h.is_installed,
                    CarouselFilterMode::Favorites => h.is_favorite,
                    CarouselFilterMode::ControllerSupport => {
                        h.supports_gamepad || h.supports_xinput || h.supports_dinput
                    }
                    CarouselFilterMode::KeyboardOnly => h.supports_keyboard,
                    CarouselFilterMode::MultiKeyboardMouse => {
                        h.supports_multiple_keyboards_and_mice || h.supports_raw_input
                    }
                    CarouselFilterMode::FourPlusPlayers => h.max_players >= 4,
                    CarouselFilterMode::TwoPlusPlayers => h.max_players >= 2,
                }
            })
            .collect();

        // Sort
        match self.sort_mode {
            CarouselSortMode::Alphabetical => {
                list.sort_by(|a, b| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()));
            }
            CarouselSortMode::AlphabeticalDesc => {
                list.sort_by(|a, b| b.game_name.to_lowercase().cmp(&a.game_name.to_lowercase()));
            }
            CarouselSortMode::RecentlyPlayed => {
                list.sort_by(|a, b| b.last_played_timestamp.cmp(&a.last_played_timestamp));
            }
            CarouselSortMode::PlayTime => {
                list.sort_by(|a, b| b.play_time_seconds.cmp(&a.play_time_seconds));
            }
            CarouselSortMode::MaxPlayers => {
                // Descending by max_players; secondary tie-breaker alphabetical
                list.sort_by(|a, b| {
                    b.max_players
                        .cmp(&a.max_players)
                        .then_with(|| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()))
                });
            }
            CarouselSortMode::InstalledFirst => {
                // Installed first, then uninstalled; secondary tie-breaker alphabetical
                list.sort_by(|a, b| {
                    b.is_installed
                        .cmp(&a.is_installed)
                        .then_with(|| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()))
                });
            }
        }

        list
    }
}

/// Renders the complete Couch Big Picture view shell.
pub fn render_big_picture_view(
    win: &MainWindow,
    _window: &mut Window,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let toast = win
        .couch_setup_state
        .active_disconnect_banner()
        .map(|(idx, slot, rem, tot)| {
            crate::big_picture::setup::render_disconnect_toast_banner(idx, slot, rem, tot)
        });

    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .bg(theme::bg_main())
        .text_color(theme::text_primary())
        .child(render_big_picture_header(win, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_hidden()
                .child(match win.big_picture_screen {
                    BigPictureScreen::Carousel => render_carousel_view(win, cx),
                    BigPictureScreen::GameDetails => {
                        crate::big_picture::render_game_details_view(win, cx)
                    }
                    BigPictureScreen::GameManagement => {
                        crate::big_picture::render_game_management_view(win, cx)
                    }
                    BigPictureScreen::CouchSetup => {
                        crate::big_picture::render_couch_setup_view(win, cx)
                    }
                    BigPictureScreen::CouchHub => {
                        crate::big_picture::render_couch_hub_view(win, cx)
                    }
                    BigPictureScreen::Sessions => crate::big_picture::render_sessions_view(win, cx),
                    BigPictureScreen::Settings => {
                        crate::big_picture::render_big_picture_settings_view(win, cx)
                    }
                }),
        )
        .child(render_couch_prompt_bar(win.big_picture_screen))
        .children(toast)
        .children(if win.big_picture_screen != BigPictureScreen::CouchHub {
            win.couch_hub_state
                .custom_options_modal
                .as_ref()
                .map(|m| crate::big_picture::render_custom_options_modal(m, cx))
        } else {
            None
        })
        .children(if win.big_picture_screen != BigPictureScreen::CouchHub {
            win.couch_hub_state
                .missing_exe_modal_handler
                .as_deref()
                .map(|h| crate::big_picture::render_missing_exe_modal(h, cx))
        } else {
            None
        })
        .into_any_element()
}

fn render_big_picture_header(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let dev_count = win.detected_devices_list.len().max(win.detected_devices);
    let controller_label = if dev_count == 1 {
        "1 controller".to_string()
    } else {
        format!("{} controllers", dev_count)
    };

    let clock_str = current_wall_clock_time();
    let current_top_tab = win.big_picture_screen.top_tab();

    div()
        .id("big-picture-header")
        .flex()
        .items_center()
        .justify_between()
        .h(px(64.0))
        .w_full()
        .px_10()
        .bg(theme::bg_panel())
        .border_b_1()
        .border_color(theme::border_subtle())
        // Left: "n NUCLEUS CO-OP" Branding
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(28.0))
                        .rounded_md()
                        .bg(theme::accent())
                        .text_color(theme::text_on_accent())
                        .font_weight(FontWeight::BOLD)
                        .text_sm()
                        .child("n"),
                )
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("NUCLEUS CO-OP"),
                ),
        )
        // Center: 4 Universal Console Tabs with Bumper Hints (LB / RB)
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(render_bumper_hint("LB"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_6()
                        .child(render_universal_header_tab(
                            "Library",
                            current_top_tab == BigPictureTab::Library,
                            BigPictureScreen::Carousel,
                            cx,
                        ))
                        .child(render_universal_header_tab(
                            "Add games",
                            current_top_tab == BigPictureTab::AddGames,
                            BigPictureScreen::CouchHub,
                            cx,
                        ))
                        .child(render_universal_header_tab(
                            "Sessions",
                            current_top_tab == BigPictureTab::Sessions,
                            BigPictureScreen::Sessions,
                            cx,
                        ))
                        .child(render_universal_header_tab(
                            "Settings",
                            current_top_tab == BigPictureTab::Settings,
                            BigPictureScreen::Settings,
                            cx,
                        )),
                )
                .child(render_bumper_hint("RB")),
        )
        // Right: Controller Status Dot & Digital Clock + Exit Icon
        .child(
            div()
                .flex()
                .items_center()
                .gap_5()
                // Controller status dot
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size(px(7.0)).rounded_full().bg(if dev_count > 0 {
                            theme::accent()
                        } else {
                            theme::text_muted()
                        }))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text_primary())
                                .child(controller_label),
                        ),
                )
                // Clock
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text_primary())
                        .child(clock_str),
                )
                // Exit to desktop icon button
                .child(
                    div()
                        .id("btn-couch-exit-icon")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(28.0))
                        .rounded_md()
                        .bg(theme::bg_subtle())
                        .border_1()
                        .border_color(theme::border_subtle())
                        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_big_picture(cx);
                        }))
                        .child(
                            Icon::new(IconName::LogOut)
                                .size_3p5()
                                .text_color(theme::text_muted()),
                        ),
                ),
        )
        .into_any_element()
}

fn render_bumper_hint(badge: &'static str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px_2()
        .py_0p5()
        .rounded_md()
        .bg(theme::bg_subtle())
        .border_1()
        .border_color(theme::border_subtle())
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(theme::text_muted())
        .child(badge)
        .into_any_element()
}

fn render_universal_header_tab(
    label: &'static str,
    is_active: bool,
    target_screen: BigPictureScreen,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let tab_id = match target_screen {
        BigPictureScreen::Carousel => "couch-tab-library",
        BigPictureScreen::CouchHub => "couch-tab-add-games",
        BigPictureScreen::Sessions => "couch-tab-sessions",
        BigPictureScreen::Settings => "couch-tab-settings",
        _ => "couch-tab-item",
    };

    div()
        .id(tab_id)
        .cursor_pointer()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .hover(|s| s.opacity(0.85))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.big_picture_screen = target_screen;
            if target_screen == BigPictureScreen::CouchHub && this.hub_packages.is_empty() {
                this.trigger_hub_search_async(cx);
            }
            cx.notify();
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
                    theme::text_primary()
                } else {
                    theme::text_muted()
                })
                .child(label),
        )
        // Emerald underline pill when active
        .child(div().h(px(2.5)).w_full().rounded_full().bg(if is_active {
            theme::accent()
        } else {
            gpui::transparent_black()
        }))
        .into_any_element()
}

/// Computes the sub-list of visible handler indices centered around `sel_idx`.
fn get_carousel_window_indices(sel_idx: usize, total: usize) -> Vec<usize> {
    if total <= 1 {
        return vec![0];
    }
    if total == 2 {
        return vec![0, 1];
    }
    if total == 3 {
        return vec![
            (sel_idx + total - 1) % total,
            sel_idx,
            (sel_idx + 1) % total,
        ];
    }
    if total == 4 {
        return vec![
            (sel_idx + total - 1) % total,
            sel_idx,
            (sel_idx + 1) % total,
            (sel_idx + 2) % total,
        ];
    }
    // 5 or more: 2 left, 1 center, 2 right
    vec![
        (sel_idx + total - 2) % total,
        (sel_idx + total - 1) % total,
        sel_idx,
        (sel_idx + 1) % total,
        (sel_idx + 2) % total,
    ]
}

/// Renders the 10-foot game library carousel browsing view.
pub fn render_carousel_view(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    if win.handlers.is_empty() {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_8()
                    .rounded_xl()
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(theme::border_color())
                    .max_w(px(520.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(64.0))
                            .rounded_full()
                            .bg(theme::bg_subtle())
                            .child(Icon::new(IconName::Gamepad2).size_8().text_color(theme::text_muted())),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text_primary())
                            .child("No Games Installed"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_center()
                            .text_color(theme::text_muted())
                            .child("No game handlers installed. Switch to Hub or Desktop Mode to download handlers."),
                    ),
            )
            .into_any_element();
    }

    let filtered_handlers = win
        .carousel_state
        .filter_and_sort_handlers(&win.handlers, &win.carousel_state.search_query);

    let total_matching = filtered_handlers.len();
    let action_bar = render_carousel_action_bar(win, total_matching, cx);

    if total_matching == 0 {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_between()
            .size_full()
            .py_6()
            .px_8()
            .child(action_bar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .p_8()
                    .rounded_xl()
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(theme::border_color())
                    .max_w(px(520.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(64.0))
                            .rounded_full()
                            .bg(theme::bg_subtle())
                            .child(Icon::new(IconName::ListFilter).size_8().text_color(theme::text_muted())),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text_primary())
                            .child("No Matching Games"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_center()
                            .text_color(theme::text_muted())
                            .child("No games match the current filter or search criteria. Try cycling the filter or resetting."),
                    )
                    .child(
                        CouchButton::secondary("btn-reset-carousel-filters", "Reset Filters")
                            .size(CouchButtonSize::Md)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.carousel_state.filter_mode = CarouselFilterMode::All;
                                this.carousel_state.sort_mode = CarouselSortMode::Alphabetical;
                                this.carousel_state.search_query.clear();
                                let total = this.carousel_state.filter_and_sort_handlers(&this.handlers, &this.carousel_state.search_query).len();
                                this.carousel_state.set_count(total);
                                cx.notify();
                            })),
                    ),
            )
            .child(div().h(px(52.0)))
            .into_any_element();
    }

    let sel_idx = win
        .carousel_state
        .selected_index
        .min(total_matching.saturating_sub(1));
    let selected_handler = filtered_handlers[sel_idx];

    let visible_indices = get_carousel_window_indices(sel_idx, total_matching);

    div()
        .flex()
        .flex_col()
        .size_full()
        .py_4()
        .px_10()
        .gap_4()
        .overflow_y_hidden()
        // 1. Top Header Row: Library / Subtitle + Search Library button (Image 6)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child("Library"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Good company. Great games."),
                        ),
                )
                .child(action_bar),
        )
        // 2. Hero Banner Card (Image 6)
        .child(render_carousel_hero_banner(win, selected_handler, cx))
        // 3. Subheader: "Your games  {N} games" (Image 6)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .w_full()
                .pt_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child("Your games"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(format!("{} games", total_matching)),
                        ),
                ),
        )
        // 4. Horizontal cards row with left/right buttons (Image 6)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_4()
                .w_full()
                // Left Arrow Button
                .child(
                    CouchButton::ghost("btn-carousel-left", "◀")
                        .size(CouchButtonSize::Lg)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let total = this
                                .carousel_state
                                .filter_and_sort_handlers(
                                    &this.handlers,
                                    &this.carousel_state.search_query,
                                )
                                .len();
                            this.carousel_state.set_count(total);
                            this.carousel_state.move_left();
                            cx.notify();
                        })),
                )
                // Cards Container
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_4()
                        .overflow_hidden()
                        .children(visible_indices.iter().map(|&idx| {
                            let handler = filtered_handlers[idx];
                            let is_selected = idx == sel_idx;
                            let is_focus_card =
                                win.carousel_state.focus_zone == CarouselFocusZone::Cards;
                            render_carousel_card(handler, idx, is_selected, is_focus_card, cx)
                        })),
                )
                // Right Arrow Button
                .child(
                    CouchButton::ghost("btn-carousel-right", "▶")
                        .size(CouchButtonSize::Lg)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let total = this
                                .carousel_state
                                .filter_and_sort_handlers(
                                    &this.handlers,
                                    &this.carousel_state.search_query,
                                )
                                .len();
                            this.carousel_state.set_count(total);
                            this.carousel_state.move_right();
                            cx.notify();
                        })),
                ),
        )
        .into_any_element()
}

fn render_carousel_hero_banner(
    win: &MainWindow,
    handler: &HandlerItem,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let focus_btn = match win.carousel_state.focus_zone {
        CarouselFocusZone::HeroBanner(idx) => Some(idx),
        _ => None,
    };
    let game_name_launch = handler.game_name.clone();
    let game_name_details = handler.game_name.clone();
    let cover_url = handler.cover_image_url.clone();

    let player_text = if handler.max_players > 1 {
        format!("1-{} players · Splitscreen co-op", handler.max_players)
    } else {
        "1-2 players · Splitscreen co-op".to_string()
    };

    div()
        .id("carousel-hero-banner")
        .relative()
        .w_full()
        .h(px(210.0))
        .rounded_2xl()
        .overflow_hidden()
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_subtle())
        // Background cover art with dark gradient overlay
        .children(cover_url.as_deref().map(|url| {
            img(crate::make_image_source(url))
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(gpui::ObjectFit::Cover)
                .opacity(0.25)
        }))
        // Gradient overlay
        .child(
            div()
                .absolute()
                .inset_0()
                .bg(theme::bg_dark().opacity(0.65)),
        )
        // Content container
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .justify_between()
                .size_full()
                .p_6()
                // Top: JUMP BACK IN pill + Title + Subtitle
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            div()
                                .px_2p5()
                                .py_0p5()
                                .rounded_full()
                                .bg(rgb(0x22c55e).opacity(0.18))
                                .border_1()
                                .border_color(rgb(0x22c55e).opacity(0.35))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x22c55e))
                                .w_auto()
                                .child("JUMP BACK IN"),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child(handler.game_name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text_muted())
                                .child(player_text),
                        ),
                )
                // Bottom: Action Buttons
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        // Button 0: Set up session (emerald green)
                        .child(
                            div()
                                .id("btn-hero-setup-session")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_5()
                                .py_2p5()
                                .rounded_lg()
                                .bg(rgb(0x22c55e))
                                .text_color(rgb(0x0a0f1d))
                                .font_weight(FontWeight::BOLD)
                                .text_sm()
                                .border_2()
                                .border_color(if focus_btn == Some(0) {
                                    rgb(0xffffff).into()
                                } else {
                                    gpui::transparent_black()
                                })
                                .shadow_md()
                                .hover(|s| s.opacity(0.9))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.enter_couch_setup(&game_name_launch, cx);
                                }))
                                .child(Icon::new(IconName::Play).size_3p5())
                                .child("Set up session"),
                        )
                        // Button 1: Game details (dark subtle)
                        .child(
                            div()
                                .id("btn-hero-game-details")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_5()
                                .py_2p5()
                                .rounded_lg()
                                .bg(theme::bg_subtle())
                                .border_2()
                                .border_color(if focus_btn == Some(1) {
                                    rgb(0x22c55e).into()
                                } else {
                                    theme::border_subtle()
                                })
                                .text_color(theme::text_primary())
                                .font_weight(FontWeight::MEDIUM)
                                .text_sm()
                                .hover(|s| {
                                    s.bg(theme::bg_hover()).border_color(theme::border_color())
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_game = Some(game_name_details.clone());
                                    this.big_picture_screen = BigPictureScreen::GameDetails;
                                    cx.notify();
                                }))
                                .child("Game details"),
                        ),
                ),
        )
        .into_any_element()
}

/// Renders the action strip with Search, Filter, Sort, and Index indicator at the top of the carousel library.
fn render_carousel_action_bar(
    win: &MainWindow,
    total_matching: usize,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let sel_idx = win
        .carousel_state
        .selected_index
        .min(total_matching.saturating_sub(1));
    let has_search = !win.carousel_state.search_query.is_empty();
    let is_search_focused = matches!(
        win.carousel_state.focus_zone,
        CarouselFocusZone::HeroBanner(2)
    );
    let is_filtered = win.carousel_state.filter_mode != CarouselFilterMode::All;
    let is_sorted = win.carousel_state.sort_mode != CarouselSortMode::Alphabetical;

    let search_label = if win.carousel_state.search_input_open {
        if has_search {
            format!("{} █", win.carousel_state.search_query)
        } else {
            "Type to search... █".to_string()
        }
    } else if has_search {
        format!("Search: \"{}\"", win.carousel_state.search_query)
    } else {
        "Search".to_string()
    };

    div()
        .id("carousel-action-bar")
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .gap_3()
        .min_w_0()
        // 1. Search Pill + Popover
        .child(
            div()
                .relative()
                .child(
                    div()
                        .id("btn-carousel-search")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3p5()
                        .py_1p5()
                        .rounded_full()
                        .min_w_0()
                        .bg(if has_search || win.carousel_state.search_input_open {
                            theme::accent_subtle()
                        } else {
                            theme::bg_panel()
                        })
                        .border_2()
                        .border_color(if is_search_focused {
                            rgb(0x22c55e).into()
                        } else if has_search || win.carousel_state.search_input_open {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .text_color(
                            if is_search_focused
                                || has_search
                                || win.carousel_state.search_input_open
                            {
                                theme::accent()
                            } else {
                                theme::text_primary()
                            },
                        )
                        .hover(|s| s.bg(theme::bg_hover()).border_color(theme::border_color()))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.carousel_state.search_input_open =
                                !this.carousel_state.search_input_open;
                            this.carousel_state.filter_picker_open = false;
                            this.carousel_state.sort_picker_open = false;
                            cx.notify();
                        }))
                        .child(Icon::new(IconName::Search).size_3p5().text_color(
                            if has_search || win.carousel_state.search_input_open {
                                theme::accent()
                            } else {
                                theme::text_muted()
                            },
                        ))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .min_w_0()
                                .child(search_label),
                        )
                        .children(if has_search {
                            Some(
                                div()
                                    .id("btn-clear-search-pill")
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .hover(|s| s.text_color(theme::danger()))
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.carousel_state.search_query.clear();
                                        let total = this
                                            .carousel_state
                                            .filter_and_sort_handlers(
                                                &this.handlers,
                                                &this.carousel_state.search_query,
                                            )
                                            .len();
                                        this.carousel_state.set_count(total);
                                        cx.notify();
                                    }))
                                    .child(Icon::new(IconName::X).size_3())
                                    .into_any_element(),
                            )
                        } else {
                            None
                        }),
                )
                .when(win.carousel_state.search_input_open, |popover_anchor| {
                    popover_anchor.child(
                        gpui::deferred(
                            div()
                                .id("carousel-search-popover")
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.carousel_state.search_input_open = false;
                                    cx.notify();
                                }))
                                .absolute()
                                .top(px(40.0))
                                .left_0()
                                .flex()
                                .flex_col()
                                .min_w(px(280.0))
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::accent())
                                .rounded_lg()
                                .shadow_lg()
                                .p_3()
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
                                                .text_color(theme::text_primary())
                                                .child("Search Library"),
                                        )
                                        .child(
                                            div()
                                                .id("btn-close-search-popover")
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .text_xs()
                                                .text_color(theme::text_muted())
                                                .hover(|s| s.text_color(theme::danger()))
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                    cx.stop_propagation();
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.carousel_state.search_input_open = false;
                                                    cx.notify();
                                                }))
                                                .child(Icon::new(IconName::X).size_3())
                                                .child("Close"),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2p5()
                                        .py_1p5()
                                        .bg(theme::bg_main())
                                        .border_1()
                                        .border_color(theme::border_color())
                                        .rounded_md()
                                        .child(
                                            Icon::new(IconName::Search)
                                                .size_3p5()
                                                .text_color(theme::accent()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if has_search {
                                                    theme::text_primary()
                                                } else {
                                                    theme::text_muted()
                                                })
                                                .child(if has_search {
                                                    format!("{} █", win.carousel_state.search_query)
                                                } else {
                                                    "Type query or click preset below..."
                                                        .to_string()
                                                }),
                                        ),
                                )
                                .child(
                                    div().flex().flex_wrap().gap_1p5().pt_1().children(
                                        [
                                            "Left 4 Dead",
                                            "Portal",
                                            "Borderlands",
                                            "Halo",
                                            "Resident Evil",
                                        ]
                                        .iter()
                                        .enumerate()
                                        .map(
                                            |(idx, &preset)| {
                                                div()
                                                    .id(("search-preset", idx))
                                                    .cursor_pointer()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_full()
                                                    .bg(theme::bg_subtle())
                                                    .border_1()
                                                    .border_color(theme::border_color())
                                                    .text_xs()
                                                    .text_color(theme::text_muted())
                                                    .hover(|s| {
                                                        s.bg(theme::accent().opacity(0.15))
                                                            .text_color(theme::accent())
                                                            .border_color(theme::accent())
                                                    })
                                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                        cx.stop_propagation();
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        cx.stop_propagation();
                                                        this.carousel_state.search_query =
                                                            preset.to_string();
                                                        this.carousel_state.search_input_open =
                                                            false;
                                                        let total = this
                                                            .carousel_state
                                                            .filter_and_sort_handlers(
                                                                &this.handlers,
                                                                &this.carousel_state.search_query,
                                                            )
                                                            .len();
                                                        this.carousel_state.set_count(total);
                                                        cx.notify();
                                                    }))
                                                    .child(preset)
                                            },
                                        ),
                                    ),
                                ),
                        )
                        .with_priority(10),
                    )
                }),
        )
        // 2. Filter Pill + Popover Options
        .child(
            div()
                .relative()
                .child(
                    div()
                        .id("btn-carousel-filter")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3p5()
                        .py_1p5()
                        .rounded_full()
                        .min_w_0()
                        .bg(if is_filtered || win.carousel_state.filter_picker_open {
                            theme::accent_subtle()
                        } else {
                            theme::bg_panel()
                        })
                        .border_1()
                        .border_color(if is_filtered || win.carousel_state.filter_picker_open {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .text_color(if is_filtered || win.carousel_state.filter_picker_open {
                            theme::accent()
                        } else {
                            theme::text_primary()
                        })
                        .hover(|s| s.bg(theme::bg_hover()).border_color(theme::border_color()))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.carousel_state.filter_picker_open =
                                !this.carousel_state.filter_picker_open;
                            this.carousel_state.sort_picker_open = false;
                            this.carousel_state.search_input_open = false;
                            cx.notify();
                        }))
                        .child(Icon::new(IconName::ListFilter).size_3p5().text_color(
                            if is_filtered || win.carousel_state.filter_picker_open {
                                theme::accent()
                            } else {
                                theme::text_muted()
                            },
                        ))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .min_w_0()
                                .child(format!(
                                    "Filter: {}",
                                    win.carousel_state.filter_mode.label()
                                )),
                        )
                        .child(div().text_xs().text_color(theme::accent()).child(
                            if win.carousel_state.filter_picker_open {
                                "▲"
                            } else {
                                "▼"
                            },
                        )),
                )
                .when(win.carousel_state.filter_picker_open, |popover_anchor| {
                    popover_anchor.child(
                        gpui::deferred(
                            div()
                                .id("carousel-filter-popover")
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.carousel_state.filter_picker_open = false;
                                    cx.notify();
                                }))
                                .absolute()
                                .top(px(40.0))
                                .left_0()
                                .flex()
                                .flex_col()
                                .min_w(px(240.0))
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_color())
                                .rounded_lg()
                                .shadow_lg()
                                .p_2()
                                .gap_1()
                                .children(CarouselFilterMode::ALL.iter().enumerate().map(
                                    |(idx, &mode)| {
                                        let is_active = win.carousel_state.filter_mode == mode;
                                        div()
                                            .id(("filter-opt", idx))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_xs()
                                            .when(is_active, |s| {
                                                s.bg(theme::accent().opacity(0.15))
                                                    .text_color(theme::accent())
                                                    .font_weight(FontWeight::BOLD)
                                            })
                                            .when(!is_active, |s| {
                                                s.text_color(theme::text_primary())
                                                    .hover(|h| h.bg(theme::bg_hover()))
                                            })
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation();
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.carousel_state.filter_mode = mode;
                                                this.carousel_state.filter_picker_open = false;
                                                let total = this
                                                    .carousel_state
                                                    .filter_and_sort_handlers(
                                                        &this.handlers,
                                                        &this.carousel_state.search_query,
                                                    )
                                                    .len();
                                                this.carousel_state.set_count(total);
                                                cx.notify();
                                            }))
                                            .child(mode.label())
                                            .when(is_active, |el| {
                                                el.child(
                                                    Icon::new(IconName::Check)
                                                        .size_3p5()
                                                        .text_color(theme::accent()),
                                                )
                                            })
                                    },
                                )),
                        )
                        .with_priority(10),
                    )
                }),
        )
        // 3. Sort Pill + Popover Options
        .child(
            div()
                .relative()
                .child(
                    div()
                        .id("btn-carousel-sort")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3p5()
                        .py_1p5()
                        .rounded_full()
                        .min_w_0()
                        .bg(if is_sorted || win.carousel_state.sort_picker_open {
                            theme::accent_subtle()
                        } else {
                            theme::bg_panel()
                        })
                        .border_1()
                        .border_color(if is_sorted || win.carousel_state.sort_picker_open {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .text_color(if is_sorted || win.carousel_state.sort_picker_open {
                            theme::accent()
                        } else {
                            theme::text_primary()
                        })
                        .hover(|s| s.bg(theme::bg_hover()).border_color(theme::border_color()))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.carousel_state.sort_picker_open =
                                !this.carousel_state.sort_picker_open;
                            this.carousel_state.filter_picker_open = false;
                            this.carousel_state.search_input_open = false;
                            cx.notify();
                        }))
                        .child(Icon::new(IconName::ArrowUpDown).size_3p5().text_color(
                            if is_sorted || win.carousel_state.sort_picker_open {
                                theme::accent()
                            } else {
                                theme::text_muted()
                            },
                        ))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .min_w_0()
                                .child(format!("Sort: {}", win.carousel_state.sort_mode.label())),
                        )
                        .child(div().text_xs().text_color(theme::accent()).child(
                            if win.carousel_state.sort_picker_open {
                                "▲"
                            } else {
                                "▼"
                            },
                        )),
                )
                .when(win.carousel_state.sort_picker_open, |popover_anchor| {
                    popover_anchor.child(
                        gpui::deferred(
                            div()
                                .id("carousel-sort-popover")
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.carousel_state.sort_picker_open = false;
                                    cx.notify();
                                }))
                                .absolute()
                                .top(px(40.0))
                                .left_0()
                                .flex()
                                .flex_col()
                                .min_w(px(220.0))
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_color())
                                .rounded_lg()
                                .shadow_lg()
                                .p_2()
                                .gap_1()
                                .children(CarouselSortMode::ALL.iter().enumerate().map(
                                    |(idx, &mode)| {
                                        let is_active = win.carousel_state.sort_mode == mode;
                                        div()
                                            .id(("sort-opt", idx))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_xs()
                                            .when(is_active, |s| {
                                                s.bg(theme::accent().opacity(0.15))
                                                    .text_color(theme::accent())
                                                    .font_weight(FontWeight::BOLD)
                                            })
                                            .when(!is_active, |s| {
                                                s.text_color(theme::text_primary())
                                                    .hover(|h| h.bg(theme::bg_hover()))
                                            })
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation();
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.carousel_state.sort_mode = mode;
                                                this.carousel_state.sort_picker_open = false;
                                                let total = this
                                                    .carousel_state
                                                    .filter_and_sort_handlers(
                                                        &this.handlers,
                                                        &this.carousel_state.search_query,
                                                    )
                                                    .len();
                                                this.carousel_state.set_count(total);
                                                cx.notify();
                                            }))
                                            .child(mode.label())
                                            .when(is_active, |el| {
                                                el.child(
                                                    Icon::new(IconName::Check)
                                                        .size_3p5()
                                                        .text_color(theme::accent()),
                                                )
                                            })
                                    },
                                )),
                        )
                        .with_priority(10),
                    )
                }),
        )
        // 4. Index Counter Indicator
        .child(
            div()
                .id("carousel-index-counter")
                .flex()
                .items_center()
                .gap_1p5()
                .px_3p5()
                .py_1p5()
                .rounded_full()
                .min_w_0()
                .bg(theme::bg_panel())
                .border_1()
                .border_color(theme::border_subtle())
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent())
                        .child(if total_matching > 0 {
                            format!("{}", sel_idx + 1)
                        } else {
                            "0".to_string()
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(format!("of {}", total_matching)),
                ),
        )
        .into_any_element()
}

/// Returns the visual opacity for a carousel game card based on installation status and selection state.
pub fn carousel_card_opacity(is_installed: bool, is_selected: bool) -> f32 {
    if !is_installed {
        if is_selected { 0.60 } else { 0.40 }
    } else if !is_selected {
        0.70
    } else {
        1.0
    }
}

/// Prominent badge text displayed on uninstalled carousel cards.
pub const UNINSTALLED_BADGE_TEXT: &str = "Uninstalled (.exe missing)";

/// Callout text displayed in the metadata strip when a selected game's executable is missing.
pub const MISSING_EXE_METADATA_CALLOUT: &str = "Executable Missing - Press (A) to Locate .exe";

fn render_carousel_card(
    handler: &HandlerItem,
    idx: usize,
    is_selected: bool,
    is_focus_card: bool,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let (w, h) = (px(190.0), px(110.0));
    let cover_url_opt = handler.cover_image_url.clone();
    let game_name = handler.game_name.clone();

    let player_text = if !handler.is_installed {
        "Handler available".to_string()
    } else if handler.max_players > 1 {
        format!("{} players", handler.max_players)
    } else {
        "Co-op".to_string()
    };

    let border_color: Hsla = if is_selected && is_focus_card {
        rgb(0x22c55e).into()
    } else if is_selected {
        theme::accent()
    } else {
        theme::border_subtle()
    };

    let card_box = div()
        .w(w)
        .h(h)
        .rounded_xl()
        .overflow_hidden()
        .bg(theme::bg_panel())
        .border_2()
        .border_color(border_color)
        .shadow_md()
        .child(if let Some(url) = cover_url_opt {
            img(crate::make_image_source(&url))
                .size_full()
                .object_fit(gpui::ObjectFit::Cover)
                .into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .bg(theme::bg_subtle())
                .child(
                    Icon::new(IconName::Gamepad2)
                        .size_8()
                        .text_color(theme::text_muted()),
                )
                .into_any_element()
        });

    div()
        .id(("carousel-card-wrap", idx))
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap_1p5()
        .w(w)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.select_carousel_game(idx, cx);
        }))
        .child(card_box)
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
                        .line_clamp(1)
                        .child(game_name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(player_text),
                ),
        )
        .into_any_element()
}

fn render_couch_prompt_bar(screen: BigPictureScreen) -> AnyElement {
    let mut left_prompts = div().flex().items_center().gap_6();

    match screen {
        BigPictureScreen::CouchSetup => {
            left_prompts = left_prompts
                .child(render_couch_prompt("A", "Assign / swap"))
                .child(render_couch_prompt("X", "Remove"))
                .child(render_couch_prompt("Y", "Ignore device"))
                .child(render_couch_prompt("B", "Back"))
                .child(render_couch_prompt("LB RB", "Switch tabs"));
        }
        BigPictureScreen::GameManagement => {
            left_prompts = left_prompts
                .child(render_couch_prompt("A", "Select"))
                .child(render_couch_prompt("B", "Back"))
                .child(render_couch_prompt("LB RB", "Switch tabs"));
        }
        BigPictureScreen::Sessions => {
            left_prompts = left_prompts
                .child(render_couch_prompt("A", "Select"))
                .child(render_couch_prompt("B", "Resume game"))
                .child(render_couch_prompt("LB RB", "Switch tabs"));
        }
        _ => {
            left_prompts = left_prompts
                .child(render_couch_prompt("A", "Select"))
                .child(render_couch_prompt("B", "Back"))
                .child(render_couch_prompt("LB RB", "Switch tabs"));
        }
    }

    div()
        .id("couch-prompt-bar")
        .flex()
        .items_center()
        .justify_between()
        .h(px(48.0))
        .w_full()
        .px_10()
        .bg(theme::bg_panel())
        .border_t_1()
        .border_color(theme::border_subtle())
        .child(left_prompts)
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_muted())
                .child("BIG PICTURE"),
        )
        .into_any_element()
}

fn render_couch_prompt(btn: &str, action: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(theme::bg_panel())
                .border_1()
                .border_color(theme::border_color())
                .shadow_sm()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::accent())
                .child(btn.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text_muted())
                .child(action.to_string()),
        )
        .into_any_element()
}
