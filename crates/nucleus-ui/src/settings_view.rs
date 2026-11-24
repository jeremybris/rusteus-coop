//! Application settings panel view.
//!
//! Provides configuration controls for application appearance, keyboard shortcuts,
//! default paths, logging level, network preferences, player slots, audio routing,
//! and windows merger layout options matching C# Nucleus Co-op parity,
//! rethemed and modernized to Adobe Spectrum 2 design principles.

use crate::components::button::{CouchButton, CouchButtonSize};
use crate::components::option_row::CouchOptionRow;
use crate::components::search_bar::CouchSearchBar;
use crate::nefos::*;
use crate::theme;
use crate::window::{ActiveTab, MainWindow};
use gpui::prelude::*;
use gpui::*;
use gpui_component::Icon;
use gpui_kit_assets::IconName;

thread_local! {
    static SETTINGS_SEARCH_FOCUS: std::cell::RefCell<Option<FocusHandle>> = std::cell::RefCell::new(None);
}

/// Player slot field currently being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEditField {
    Nickname,
    SteamId,
}

/// View component for modifying application-wide preferences.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SettingsView {
    /// Active settings category tab.
    pub active_category: SettingsCategory,
    /// Active page for Players tab (0 for Players 1-16, 1 for Players 17-32).
    pub player_page: usize,
    /// Hotkey identifier currently in interactive key-press recording mode (e.g. "lock", "close").
    pub recording_hotkey: Option<String>,
    /// Hotkey identifier currently displaying quick select dropdown (e.g. "lock", "close").
    pub dropdown_hotkey: Option<String>,
    /// Search filter for settings options.
    pub search_query: String,
    /// Player slot field currently in interactive typing/edit mode.
    pub editing_player: Option<(usize, PlayerEditField)>,
    /// Audio instance row currently displaying device selection dropdown.
    pub dropdown_audio_instance: Option<usize>,
    /// Active page for Audio tab (0 for Instances 1-8, 1 for Instances 9-16, 2 for Instances 17-24, 3 for Instances 25-32).
    pub audio_page: usize,
    /// Whether user is currently editing the custom Windows account password.
    pub editing_password: bool,
}

/// Settings categories available in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsCategory {
    /// General application settings, language, network, and isolation.
    #[default]
    General,
    /// Dedicated hotkeys, shortcuts, and input locking center.
    Hotkeys,
    /// Player slot nicknames and Steam IDs (1-32).
    Players,
    /// Per-instance audio device routing (1-8).
    Audio,
    /// Windows Merger and custom splitscreen layouts.
    Layout,
    /// Visual appearance, themes, OSD color, and directories.
    Appearance,
    /// Network loopback IP isolation and Goldberg Steam emulator configuration.
    Network,
}

impl SettingsCategory {
    /// Human-readable label for settings category.
    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::General => "General & Isolation",
            SettingsCategory::Hotkeys => "Hotkeys & Shortcuts",
            SettingsCategory::Players => "Players (1 - 32)",
            SettingsCategory::Audio => "Audio Routing",
            SettingsCategory::Layout => "Layout & Merger",
            SettingsCategory::Appearance => "Appearance & Paths",
            SettingsCategory::Network => "Network & Emulation",
        }
    }

    /// Primary asset icon for settings category.
    pub fn icon(&self) -> IconName {
        match self {
            SettingsCategory::General => IconName::SlidersHorizontal,
            SettingsCategory::Hotkeys => IconName::Keyboard,
            SettingsCategory::Players => IconName::Users,
            SettingsCategory::Audio => IconName::Volume2,
            SettingsCategory::Layout => IconName::LayoutGrid,
            SettingsCategory::Appearance => IconName::Palette,
            SettingsCategory::Network => IconName::Globe,
        }
    }
}

impl SettingsView {
    /// Creates a new `SettingsView`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Standard shortcuts configuration metadata.
pub struct ShortcutMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default_val: &'static str,
}

pub const APP_SHORTCUTS: &[ShortcutMeta] = &[
    ShortcutMeta {
        id: "close",
        name: "Close Game Instances",
        description: "Force terminate child game instances and release handles",
        default_val: "Ctrl+Q",
    },
    ShortcutMeta {
        id: "stop",
        name: "Stop Session",
        description: "Cleanly tear down the active session, hooks, and instances",
        default_val: "Ctrl+E",
    },
    ShortcutMeta {
        id: "topmost",
        name: "Top-Most Windows",
        description: "Toggle always-on-top borderless window flag for instances",
        default_val: "Ctrl+T",
    },
    ShortcutMeta {
        id: "set_focus",
        name: "Cycle Window Focus",
        description: "Cycle active foreground focus between game viewports",
        default_val: "Ctrl+H",
    },
    ShortcutMeta {
        id: "reset_windows",
        name: "Reset Window Viewports",
        description: "Re-assert calculated window dimensions, positions and borderless style",
        default_val: "Ctrl+R",
    },
    ShortcutMeta {
        id: "cutscenes",
        name: "Cutscenes Aspect Ratio",
        description: "Toggle aspect-ratio preserved cutscenes display mode",
        default_val: "Ctrl+M",
    },
    ShortcutMeta {
        id: "switch",
        name: "Switch Splitscreen Layout",
        description: "Dynamically cycle between splitscreen screen partition layouts",
        default_val: "Ctrl+N",
    },
    ShortcutMeta {
        id: "shortcuts_reminder",
        name: "Shortcuts Reminder HUD",
        description: "Toggle the on-screen shortcuts reminder HUD overlay",
        default_val: "Alt+R",
    },
    ShortcutMeta {
        id: "merger_switch",
        name: "Merger Switch Viewport",
        description: "Switch active child viewport in Windows Merger multi-game view",
        default_val: "Alt+A",
    },
    ShortcutMeta {
        id: "couch_osd",
        name: "Couch In-Game Menu",
        description: "Toggle Couch Big Picture in-game OSD menu overlay",
        default_val: "F10",
    },
];

pub fn get_hotkey_value(settings: &nucleus_profile::AppSettings, id: &str) -> String {
    match id {
        "lock" => settings.lock_inputs_key.clone(),
        "close" => settings.hotkey_close.clone(),
        "stop" => settings.hotkey_stop.clone(),
        "topmost" => settings.hotkey_topmost.clone(),
        "set_focus" => settings.hotkey_set_focus.clone(),
        "reset_windows" => settings.hotkey_reset_windows.clone(),
        "cutscenes" => settings.hotkey_cutscenes.clone(),
        "switch" => settings.hotkey_switch.clone(),
        "shortcuts_reminder" => settings.hotkey_shortcuts_reminder.clone(),
        "merger_switch" => settings.hotkey_merger_switch.clone(),
        "couch_osd" => settings.hotkey_couch_osd.clone(),
        _ => String::new(),
    }
}

pub fn set_hotkey_value(settings: &mut nucleus_profile::AppSettings, id: &str, val: String) {
    match id {
        "lock" => settings.lock_inputs_key = val,
        "close" => settings.hotkey_close = val,
        "stop" => settings.hotkey_stop = val,
        "topmost" => settings.hotkey_topmost = val,
        "set_focus" => settings.hotkey_set_focus = val,
        "reset_windows" => settings.hotkey_reset_windows = val,
        "cutscenes" => settings.hotkey_cutscenes = val,
        "switch" => settings.hotkey_switch = val,
        "shortcuts_reminder" => settings.hotkey_shortcuts_reminder = val,
        "merger_switch" => settings.hotkey_merger_switch = val,
        "couch_osd" => settings.hotkey_couch_osd = val,
        _ => {}
    }
}

pub fn get_default_hotkey(id: &str) -> &'static str {
    match id {
        "lock" => "End",
        "close" => "Ctrl+Q",
        "stop" => "Ctrl+E",
        "topmost" => "Ctrl+T",
        "set_focus" => "Ctrl+H",
        "reset_windows" => "Ctrl+R",
        "cutscenes" => "Ctrl+M",
        "switch" => "Ctrl+N",
        "shortcuts_reminder" => "Alt+R",
        "merger_switch" => "Alt+A",
        "couch_osd" => "F10",
        _ => "Ctrl+Q",
    }
}

pub fn normalize_key_input(key_raw: &str) -> String {
    let lower = key_raw.to_lowercase();
    match lower.as_str() {
        "end" => "End".to_string(),
        "home" => "Home".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "insert" | "ins" => "Insert".to_string(),
        "pageup" | "prior" => "PageUp".to_string(),
        "pagedown" | "next" => "PageDown".to_string(),
        "arrowup" | "up" => "Up".to_string(),
        "arrowdown" | "down" => "Down".to_string(),
        "arrowleft" | "left" => "Left".to_string(),
        "arrowright" | "right" => "Right".to_string(),
        "multiply" | "*" => "Multiply".to_string(),
        "tab" => "Tab".to_string(),
        "space" => "Space".to_string(),
        "return" | "enter" => "Enter".to_string(),
        "backspace" => "Backspace".to_string(),
        "escape" | "esc" => "Escape".to_string(),
        "pause" => "Pause".to_string(),
        "f1" => "F1".to_string(),
        "f2" => "F2".to_string(),
        "f3" => "F3".to_string(),
        "f4" => "F4".to_string(),
        "f5" => "F5".to_string(),
        "f6" => "F6".to_string(),
        "f7" => "F7".to_string(),
        "f8" => "F8".to_string(),
        "f9" => "F9".to_string(),
        "f10" => "F10".to_string(),
        "f11" => "F11".to_string(),
        "f12" => "F12".to_string(),
        "numpad0" | "num0" => "Numpad 0".to_string(),
        "numpad1" | "num1" => "Numpad 1".to_string(),
        "numpad2" | "num2" => "Numpad 2".to_string(),
        "numpad3" | "num3" => "Numpad 3".to_string(),
        "numpad4" | "num4" => "Numpad 4".to_string(),
        "numpad5" | "num5" => "Numpad 5".to_string(),
        "numpad6" | "num6" => "Numpad 6".to_string(),
        "numpad7" | "num7" => "Numpad 7".to_string(),
        "numpad8" | "num8" => "Numpad 8".to_string(),
        "numpad9" | "num9" => "Numpad 9".to_string(),
        _ => {
            if let Some(c) = key_raw.chars().next() {
                c.to_ascii_uppercase().to_string()
            } else {
                key_raw.to_uppercase()
            }
        }
    }
}

pub fn is_standalone_bindable_key(key_str: &str) -> bool {
    let lower = key_str.to_lowercase();
    if lower.starts_with('f') && lower.len() > 1 && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return true; // F1 - F24
    }
    matches!(
        lower.as_str(),
        "pause"
            | "printscreen"
            | "scrolllock"
            | "insert"
            | "delete"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
    )
}

pub fn format_shortcut_combo(modifiers: &gpui::Modifiers, key_str: &str) -> String {
    let mut mods = Vec::new();
    if modifiers.control {
        mods.push("Ctrl");
    }
    if modifiers.alt {
        mods.push("Alt");
    }
    if modifiers.shift {
        mods.push("Shift");
    }
    if mods.is_empty() && !is_standalone_bindable_key(key_str) {
        mods.push("Ctrl");
    }
    if mods.is_empty() {
        key_str.to_string()
    } else {
        format!("{}+{}", mods.join("+"), key_str)
    }
}

/// Helper function to render an Apple-style sidebar navigation item.
fn render_sidebar_nav_item(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .h(px(36.0))
        .px_2p5()
        .rounded_lg()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2p5()
        .when(is_active, |this| {
            this.bg(theme::accent().opacity(0.12))
                .border_1()
                .border_color(theme::accent().opacity(0.25))
        })
        .when(!is_active, |this| {
            this.border_1()
                .border_color(gpui::transparent_black())
                .hover(|s| s.bg(theme::bg_hover()).text_color(theme::text_primary()))
        })
        .on_click(on_click)
        // Left vertical indicator bar
        .child(
            div()
                .w(px(3.0))
                .h(px(16.0))
                .rounded_full()
                .when(is_active, |this| this.bg(theme::accent()))
                .when(!is_active, |this| this.bg(gpui::transparent_black())),
        )
        // Icon
        .child(
            Icon::new(icon)
                .size_4()
                .when(is_active, |this| this.text_color(theme::accent()))
                .when(!is_active, |this| this.text_color(theme::text_muted())),
        )
        // Label text
        .child(
            div()
                .text_xs()
                .when(is_active, |this| {
                    this.text_color(theme::text_primary())
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!is_active, |this| this.text_color(theme::text_muted()))
                .child(label),
        )
        .into_any_element()
}

/// Definition of an individual settings card for rendering and search indexing.
pub struct SettingsCardDefinition {
    pub id: &'static str,
    pub category: SettingsCategory,
    pub title: &'static str,
    pub description: &'static str,
    pub render: fn(&MainWindow, &mut Context<MainWindow>) -> AnyElement,
}

/// Catalog of all settings cards across categories for live search filtering.
pub static SETTINGS_CARDS: &[SettingsCardDefinition] = &[
    SettingsCardDefinition {
        id: "card-general-lang-network",
        category: SettingsCategory::General,
        title: "Languages & Network Adapter",
        description: "Configure Steam Goldberg emulation language and network adapter binding",
        render: render_card_general_lang_network,
    },
    SettingsCardDefinition {
        id: "card-general-isolation",
        category: SettingsCategory::General,
        title: "Isolation & Behavior Controls",
        description: "Toggle nickname injection, debug logging, and instance sandbox rules",
        render: render_card_general_isolation,
    },
    SettingsCardDefinition {
        id: "card-general-integrations",
        category: SettingsCategory::General,
        title: "Community Hub & Advanced Integrations",
        description: "Hub account credentials, AutoHotkey runtime, and extensions",
        render: render_card_general_integrations,
    },
    SettingsCardDefinition {
        id: "card-hotkeys-lock-key",
        category: SettingsCategory::Hotkeys,
        title: "Input Lock Toggle Key",
        description: "Global physical key monitored during gameplay to lock or unlock mouse and keyboard focus",
        render: render_card_hotkeys_lock_key,
    },
    SettingsCardDefinition {
        id: "card-hotkeys-controller",
        category: SettingsCategory::Hotkeys,
        title: "Controller Shortcuts & Gamepad UI Navigation",
        description: "Configure Guide button combos (Guide+Back, Guide+Y, etc.) and in-game UI navigation for controllers",
        render: render_card_hotkeys_controller,
    },
    SettingsCardDefinition {
        id: "card-hotkeys-global",
        category: SettingsCategory::Hotkeys,
        title: "In-Game Global Shortcuts",
        description: "Keyboard hotkeys captured by the background engine thread during multi-instance sessions",
        render: render_card_hotkeys_global,
    },
    SettingsCardDefinition {
        id: "card-players-controls",
        category: SettingsCategory::Players,
        title: "Player Management & Options",
        description: "Switch between player slots 1-16 and 17-32, toggle assignment rules, and batch reset",
        render: render_card_players_controls,
    },
    SettingsCardDefinition {
        id: "card-players-slots",
        category: SettingsCategory::Players,
        title: "Player Profiles",
        description: "Customize player nicknames and Steam IDs (1 - 32) passed into emulators",
        render: render_card_players_slots,
    },
    SettingsCardDefinition {
        id: "card-audio-mode",
        category: SettingsCategory::Audio,
        title: "Audio Output Device Mode",
        description: "Select between system default audio or separate routing for each player instance",
        render: render_card_audio_mode,
    },
    SettingsCardDefinition {
        id: "card-audio-endpoints",
        category: SettingsCategory::Audio,
        title: "Per-Instance Audio Endpoints (Instance 1 through 8)",
        description: "Assign physical or virtual audio output playback devices to individual player instances",
        render: render_card_audio_endpoints,
    },
    SettingsCardDefinition {
        id: "card-layout-merger",
        category: SettingsCategory::Layout,
        title: "Windows Merger Tool",
        description: "Composite all instance viewports into a single master container window",
        render: render_card_layout_merger,
    },
    SettingsCardDefinition {
        id: "card-layout-dividers",
        category: SettingsCategory::Layout,
        title: "Visual Viewport Dividers",
        description: "Render customizable visual separation bars between game viewports",
        render: render_card_layout_dividers,
    },
    SettingsCardDefinition {
        id: "card-layout-cutscenes",
        category: SettingsCategory::Layout,
        title: "Cutscenes Presentation Flags",
        description: "Maintain original aspect ratio and handle audio during cutscene transitions",
        render: render_card_layout_cutscenes,
    },
    SettingsCardDefinition {
        id: "card-appearance-theme",
        category: SettingsCategory::Appearance,
        title: "Visual Theme & Palette (Adobe Spectrum 2)",
        description: "Choose interface appearance and base light/dark color scheme",
        render: render_card_appearance_theme,
    },
    SettingsCardDefinition {
        id: "card-appearance-accent",
        category: SettingsCategory::Appearance,
        title: "Accent Color Palette",
        description: "Select global primary accent highlight across buttons, badges, and focus rings",
        render: render_card_appearance_accent,
    },
    SettingsCardDefinition {
        id: "card-appearance-osd",
        category: SettingsCategory::Appearance,
        title: "On-Screen Display (OSD) HUD Banner",
        description: "Show floating transparent status banners during game launch, repositioning, and hotkey presses",
        render: render_card_appearance_osd,
    },
    SettingsCardDefinition {
        id: "card-appearance-handlers-dir",
        category: SettingsCategory::Appearance,
        title: "Game Handlers Directory",
        description: "Custom filesystem path where downloaded and local .js handlers reside",
        render: render_card_appearance_handlers_dir,
    },
    SettingsCardDefinition {
        id: "card-appearance-script-editor",
        category: SettingsCategory::Appearance,
        title: "Handler Script Text Editor",
        description: "External editor used to inspect and modify JavaScript (.js) game handler scripts",
        render: render_card_appearance_script_editor,
    },
    SettingsCardDefinition {
        id: "card-appearance-autoscan",
        category: SettingsCategory::Appearance,
        title: "Game Discovery",
        description: "Automatically search local system storage drives for installed games matching discovered handlers",
        render: render_card_appearance_autoscan,
    },
    SettingsCardDefinition {
        id: "card-peerparty-config",
        category: SettingsCategory::Network,
        title: "PeerParty P2P Online Multiplayer",
        description: "Decentralized BitTorrent DHT multiplayer co-op, Opus voice chat, and in-game overlay",
        render: render_card_peerparty_config,
    },
    SettingsCardDefinition {
        id: "card-network-isolation",
        category: SettingsCategory::Network,
        title: "Network & Loopback IP Isolation",
        description: "Assign unique loopback IP addresses (127.0.0.X) to each player instance to avoid port collisions",
        render: render_card_network_isolation,
    },
    SettingsCardDefinition {
        id: "card-goldberg-config",
        category: SettingsCategory::Network,
        title: "Goldberg Steam Emulator Settings",
        description: "Offline LAN multiplayer emulation flags, broadcast IP, and lobby discovery",
        render: render_card_goldberg_config,
    },
    SettingsCardDefinition {
        id: "card-launch-timer-config",
        category: SettingsCategory::Network,
        title: "Instance Launch Countdown Timer",
        description: "Configure automatic proceeding when game handlers request setup actions between instances",
        render: render_card_launch_timer_config,
    },
];

/// Filters settings cards across all categories matching title or description text (case-insensitive).
pub fn filter_settings_cards(query: &str) -> Vec<&'static SettingsCardDefinition> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    SETTINGS_CARDS
        .iter()
        .filter(|c| {
            c.title.to_lowercase().contains(&q) || c.description.to_lowercase().contains(&q)
        })
        .collect()
}

/// Renders empty state card when search query returns no matching settings.
pub fn render_settings_empty_state() -> AnyElement {
    div()
        .id("settings-search-empty-container")
        .w_full()
        .child(
            NefosCard::surface("card-settings-search-empty").child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .py_12()
                    .gap_3()
                    .child(
                        div().p_3().rounded_full().bg(theme::bg_subtle()).child(
                            Icon::new(IconName::Search)
                                .size_8()
                                .text_color(theme::text_muted()),
                        ),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text_primary())
                            .child("No settings found"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child("Try searching for a different keyword"),
                    ),
            ),
        )
        .into_any_element()
}

/// Renders the complete settings view with all configuration categories.
pub fn render_settings_view(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let active_cat = win.settings_view.active_category;
    let search_query = win.settings_view.search_query.clone();

    let search_focus = SETTINGS_SEARCH_FOCUS.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(cx.focus_handle());
        }
        opt.clone().unwrap()
    });

    // Sidebar Header: Title and Search input
    let sidebar_header = div()
        .flex()
        .flex_col()
        .w_full()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Settings)
                        .size_5()
                        .text_color(theme::accent()),
                )
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_primary())
                        .child("Settings"),
                ),
        )
        .child(
            CouchSearchBar::new("settings-search-bar")
                .placeholder("Search settings...")
                .query(search_query.clone())
                .focus_handle(&search_focus)
                .on_key_down(cx.listener(
                    |this: &mut MainWindow, event: &gpui::KeyDownEvent, _window, cx| {
                        if event.keystroke.modifiers.control
                            || event.keystroke.modifiers.alt
                            || event.keystroke.modifiers.platform
                        {
                            return;
                        }
                        let key = event.keystroke.key.as_str();
                        if key.eq_ignore_ascii_case("backspace") {
                            this.settings_view.search_query.pop();
                            cx.notify();
                        } else if key.eq_ignore_ascii_case("escape") {
                            this.settings_view.search_query.clear();
                            cx.notify();
                        } else if key.eq_ignore_ascii_case("space") {
                            this.settings_view.search_query.push(' ');
                            cx.notify();
                        } else if let Some(ref s) = event.keystroke.key_char {
                            this.settings_view.search_query.push_str(s);
                            cx.notify();
                        } else if key.chars().count() == 1 {
                            this.settings_view.search_query.push_str(key);
                            cx.notify();
                        }
                    },
                ))
                .on_clear(cx.listener(|this, _, _, cx| {
                    this.settings_view.search_query.clear();
                    cx.notify();
                })),
        );

    // Sidebar Navigation Menu with 7 configuration categories
    let sidebar_menu = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(render_sidebar_nav_item(
            "nav-cat-general",
            "General & Isolation",
            IconName::SlidersHorizontal,
            active_cat == SettingsCategory::General,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::General;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ))
        .child(render_sidebar_nav_item(
            "nav-cat-hotkeys",
            "Hotkeys & Shortcuts",
            IconName::Keyboard,
            active_cat == SettingsCategory::Hotkeys,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::Hotkeys;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ))
        .child(render_sidebar_nav_item(
            "nav-cat-players",
            "Players (1 - 32)",
            IconName::Users,
            active_cat == SettingsCategory::Players,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::Players;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ))
        .child(render_sidebar_nav_item(
            "nav-cat-audio",
            "Audio Routing",
            IconName::Volume2,
            active_cat == SettingsCategory::Audio,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::Audio;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ))
        .child(render_sidebar_nav_item(
            "nav-cat-layout",
            "Layout & Merger",
            IconName::LayoutGrid,
            active_cat == SettingsCategory::Layout,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::Layout;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ))
        .child(render_sidebar_nav_item(
            "nav-cat-appearance",
            "Appearance & Paths",
            IconName::Palette,
            active_cat == SettingsCategory::Appearance,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::Appearance;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ))
        .child(render_sidebar_nav_item(
            "nav-cat-network",
            "Network & Emulation",
            IconName::Globe,
            active_cat == SettingsCategory::Network,
            cx.listener(|this, _, _, cx| {
                this.settings_view.active_category = SettingsCategory::Network;
                this.settings_view.recording_hotkey = None;
                this.settings_view.dropdown_hotkey = None;
                this.settings_view.search_query.clear();
                cx.notify();
            }),
        ));

    // Sidebar Top Container (Header + Navigation)
    let sidebar_top = div()
        .flex()
        .flex_col()
        .gap_4()
        .p_4()
        .child(sidebar_header)
        .child(sidebar_menu);

    // Sidebar Footer: Reset Defaults button
    let sidebar_footer = div()
        .p_4()
        .border_t_1()
        .border_color(theme::border_color())
        .child(
            div()
                .id("btn-reset-defaults-sidebar")
                .w_full()
                .h(px(34.0))
                .px_3()
                .rounded_lg()
                .border_1()
                .border_color(theme::border_subtle())
                .bg(theme::bg_panel())
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .text_xs()
                .text_color(theme::text_muted())
                .hover(|s| {
                    s.bg(theme::danger().opacity(0.12))
                        .border_color(theme::danger().opacity(0.35))
                        .text_color(theme::danger())
                })
                .child(Icon::new(IconName::RotateCcw).size_3p5())
                .child(div().child("Reset Defaults"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.confirm_dialog = Some(crate::ConfirmDialogState {
                        title: "Reset All Preferences?".to_string(),
                        message: "Are you sure you want to restore all application preferences, hotkeys, and player nicknames to default values? Any custom configurations will be reset.".to_string(),
                        confirm_label: "Reset to Defaults".to_string(),
                        is_destructive: true,
                        action: crate::ConfirmAction::ResetDefaults,
                    });
                    cx.notify();
                })),
        );

    // Left column: Apple-style Sidebar Container
    let sidebar = div()
        .id("settings-sidebar")
        .w(px(240.0))
        .h_full()
        .bg(theme::bg_panel())
        .border_r_1()
        .border_color(theme::border_color())
        .flex()
        .flex_col()
        .justify_between()
        .child(sidebar_top)
        .child(sidebar_footer);

    // Active category meta for right top bar
    let (cat_title, cat_desc): (SharedString, SharedString) = if !search_query.trim().is_empty() {
        (
            "Search Results".into(),
            format!("Settings matching \"{}\"", search_query.trim()).into(),
        )
    } else {
        match active_cat {
            SettingsCategory::General => (
                "General & Isolation".into(),
                "Application preferences, language, network adapter, and instance isolation flags"
                    .into(),
            ),
            SettingsCategory::Hotkeys => (
                "Hotkeys & Shortcuts".into(),
                "Input lock keys, window controls, and quick action shortcuts".into(),
            ),
            SettingsCategory::Players => (
                "Players (1 - 32)".into(),
                "Player nicknames, Steam IDs, and virtual slot configurations".into(),
            ),
            SettingsCategory::Audio => (
                "Audio Routing".into(),
                "Per-instance audio endpoint device assignment and volume routing".into(),
            ),
            SettingsCategory::Layout => (
                "Layout & Windows Merger".into(),
                "Custom splitscreen partitions, Windows Merger settings, and viewport scaling"
                    .into(),
            ),
            SettingsCategory::Appearance => (
                "Appearance & Paths".into(),
                "UI theme, OSD notifications, and Nucleus Co-op directory locations".into(),
            ),
            SettingsCategory::Network => (
                "Network & Emulation".into(),
                "Loopback IP address isolation (127.0.0.X) and Goldberg Steam Emulator parameters"
                    .into(),
            ),
        }
    };

    // Right column: Main settings content panel
    let content_panel = div()
        .id("settings-main-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .h_full()
        .bg(theme::bg_main())
        .overflow_hidden()
        // Top bar
        .child(
            div()
                .id("settings-category-header")
                .flex()
                .items_center()
                .justify_between()
                .px_6()
                .py_4()
                .bg(theme::bg_panel())
                .border_b_1()
                .border_color(theme::border_color())
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
                                .child(cat_title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(cat_desc),
                        ),
                )
                .child(
                    CouchButton::primary("btn-save-settings", "Save Settings")
                        .icon(IconName::Save)
                        .size(CouchButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Err(e) = this.app_settings.save() {
                                this.status_message = format!("Error saving settings: {}", e);
                            } else {
                                this.status_message = "Settings Saved Successfully".to_string();
                                this.show_osd("Settings Saved", 600, cx);
                            }
                            cx.notify();
                        })),
                ),
        )
        // Scrollable tab content
        .child(
            div()
                .id("settings-content-scroll")
                .overflow_y_scroll()
                .flex_1()
                .min_h_0()
                .bg(theme::bg_main())
                .p_6()
                .flex()
                .flex_col()
                .gap_5()
                .child(if !search_query.trim().is_empty() {
                    let matching_cards = filter_settings_cards(&search_query);
                    if matching_cards.is_empty() {
                        render_settings_empty_state()
                    } else {
                        div()
                            .id("settings-search-results-list")
                            .flex()
                            .flex_col()
                            .gap_6()
                            .children(matching_cards.into_iter().map(|card| {
                                div()
                                    .id(format!("search-result-{}", card.id))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div().flex().items_center().gap_2().child(
                                            NefosTag::new(card.category.label())
                                                .tone(NefosTagTone::Accent)
                                                .icon(card.category.icon()),
                                        ),
                                    )
                                    .child((card.render)(win, cx))
                            }))
                            .into_any_element()
                    }
                } else {
                    match active_cat {
                        SettingsCategory::General => render_general_tab(win, cx),
                        SettingsCategory::Hotkeys => render_hotkeys_tab(win, cx),
                        SettingsCategory::Players => render_players_tab(win, cx),
                        SettingsCategory::Audio => render_audio_tab(win, cx),
                        SettingsCategory::Layout => render_layout_tab(win, cx),
                        SettingsCategory::Appearance => render_appearance_tab(win, cx),
                        SettingsCategory::Network => render_network_tab(win, cx),
                    }
                }),
        );

    // Root two-column container
    div()
        .id("settings-view-root")
        .flex()
        .flex_row()
        .size_full()
        .min_w_0()
        .bg(theme::bg_main())
        .child(sidebar)
        .child(content_panel)
        .into_any_element()
}

/// Renders Card: Languages & Network Adapter.
pub fn render_card_general_lang_network(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let settings = &win.app_settings;
    NefosCard::surface("card-general-lang-network")
        .title_text("Languages & Network Adapter")
        .description_text("Configure Steam Goldberg emulation language and network adapter binding")
        // Steam Language
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Steam Language (passed to Goldberg / Steam API emulators):"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1p5()
                        .children([
                            "Automatic", "English", "French", "German", "Spanish",
                            "Italian", "Japanese", "Russian", "Simplified Chinese",
                        ].iter().enumerate().map(|(idx, &lang)| {
                            let is_active = settings.steam_lang.eq_ignore_ascii_case(lang);
                            let lang_val = lang.to_string();
                            div()
                                .id(("steam-lang", idx))
                                .cursor_pointer()
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .when(is_active, |this| {
                                    this.bg(theme::accent())
                                        .border_color(theme::accent())
                                        .text_color(theme::text_on_accent())
                                        .font_weight(FontWeight::SEMIBOLD)
                                })
                                .when(!is_active, |this| {
                                    this.bg(theme::bg_subtle())
                                        .border_color(theme::border_subtle())
                                        .text_color(theme::text_muted())
                                        .hover(|s| s.bg(theme::bg_hover()).text_color(theme::text_primary()))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.app_settings.steam_lang = lang_val.clone();
                                    cx.notify();
                                }))
                                .child(lang)
                        })),
                ),
        )
        // Epic Language
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Epic Games Language:"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1p5()
                        .children([
                            "English", "French", "German", "Spanish", "Italian", "Japanese", "Russian",
                        ].iter().enumerate().map(|(idx, &lang)| {
                            let is_active = settings.epic_lang.eq_ignore_ascii_case(lang);
                            let lang_val = lang.to_string();
                            div()
                                .id(("epic-lang", idx))
                                .cursor_pointer()
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .when(is_active, |this| {
                                    this.bg(theme::accent())
                                        .border_color(theme::accent())
                                        .text_color(theme::text_on_accent())
                                        .font_weight(FontWeight::SEMIBOLD)
                                })
                                .when(!is_active, |this| {
                                    this.bg(theme::bg_subtle())
                                        .border_color(theme::border_subtle())
                                        .text_color(theme::text_muted())
                                        .hover(|s| s.bg(theme::bg_hover()).text_color(theme::text_primary()))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.app_settings.epic_lang = lang_val.clone();
                                    cx.notify();
                                }))
                                .child(lang)
                        })),
                ),
        )
        // Network Adapter
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2p5()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(220.0))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text_primary())
                                .child("Network Binding (ForceBindIP)"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Select network adapter or loopback for multiplayer instance virtualization"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .gap_2()
                        .children(["Automatic", "Localhost (127.0.0.1)"].iter().enumerate().map(|(idx, &net)| {
                            let is_active = settings.network.eq_ignore_ascii_case(net);
                            let net_val = net.to_string();
                            div()
                                .id(("net-opt", idx))
                                .cursor_pointer()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .when(is_active, |this| {
                                    this.bg(theme::accent())
                                        .border_color(theme::accent())
                                        .text_color(theme::text_on_accent())
                                })
                                .when(!is_active, |this| {
                                    this.bg(theme::bg_subtle())
                                        .border_color(theme::border_subtle())
                                        .text_color(theme::text_muted())
                                        .hover(|s| s.bg(theme::bg_hover()))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.app_settings.network = net_val.clone();
                                    cx.notify();
                                }))
                                .child(net)
                        })),
                ),
        )
        .into_any_element()
}

/// Renders Card: Isolation & Behavior Controls.
pub fn render_card_general_isolation(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;
    NefosCard::surface("card-general-isolation")
        .title_text("Isolation & Behavior Controls")
        .description_text("Toggle nickname injection, debug logging, and instance sandbox rules")
        .child(render_toggle(
            "Use Nicknames in game",
            "Inject custom per-player nicknames into emulator configs and save files",
            settings.use_nicks_in_game,
            cx.listener(|this, _, _, cx| {
                this.app_settings.use_nicks_in_game = !this.app_settings.use_nicks_in_game;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Debug Log",
            "Enable verbose engine diagnostic logging for process and mutex monitoring",
            settings.debug_log,
            cx.listener(|this, _, _, cx| {
                this.app_settings.debug_log = !this.app_settings.debug_log;
                this.status_message = format!(
                    "Debug log {}",
                    if this.app_settings.debug_log {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Keep Accounts",
            "Preserve generated Windows player user accounts between launches",
            settings.keep_accounts,
            cx.listener(|this, _, _, cx| {
                this.app_settings.keep_accounts = !this.app_settings.keep_accounts;
                cx.notify();
            }),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3p5()
                .py_2p5()
                .bg(theme::bg_subtle())
                .rounded_lg()
                .border_1()
                .border_color(theme::border_subtle())
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
                                .child("Windows User Account Password"),
                        )
                        .child(div().text_xs().text_color(theme::text_muted()).child(
                            "Password for generated Windows player user accounts (C# Parity)",
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        // Editable text input box
                        .child({
                            let is_editing = win.settings_view.editing_password;
                            let pass_val = settings.nucleus_account_password.clone();
                            div()
                                .id("custom-password-input")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(if is_editing {
                                    theme::bg_panel()
                                } else {
                                    theme::bg_subtle()
                                })
                                .border_1()
                                .border_color(if is_editing {
                                    theme::accent()
                                } else {
                                    theme::border_color()
                                })
                                .when(is_editing, |box_el| {
                                    box_el
                                        .child(
                                            div()
                                                .id("input-field-custom-pwd")
                                                .key_context("settings_custom_password")
                                                .on_key_down(cx.listener(
                                                    |this, event: &gpui::KeyDownEvent, _, cx| {
                                                        if event.keystroke.modifiers.control
                                                            || event.keystroke.modifiers.alt
                                                            || event.keystroke.modifiers.platform
                                                        {
                                                            return;
                                                        }
                                                        let key = event.keystroke.key.as_str();
                                                        match key {
                                                            "backspace" => {
                                                                this.app_settings
                                                                    .nucleus_account_password
                                                                    .pop();
                                                                let _ = this.app_settings.save();
                                                                cx.notify();
                                                            }
                                                            "escape" | "enter" => {
                                                                this.settings_view
                                                                    .editing_password = false;
                                                                let _ = this.app_settings.save();
                                                                cx.notify();
                                                            }
                                                            _ => {
                                                                if let Some(ref s) =
                                                                    event.keystroke.key_char
                                                                {
                                                                    this.app_settings
                                                                        .nucleus_account_password
                                                                        .push_str(s);
                                                                    let _ =
                                                                        this.app_settings.save();
                                                                    cx.notify();
                                                                } else if key.chars().count() == 1 {
                                                                    this.app_settings
                                                                        .nucleus_account_password
                                                                        .push_str(key);
                                                                    let _ =
                                                                        this.app_settings.save();
                                                                    cx.notify();
                                                                }
                                                            }
                                                        }
                                                    },
                                                ))
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child(format!("{}|", pass_val)),
                                        )
                                        .child(
                                            div()
                                                .id("btn-done-custom-password")
                                                .cursor_pointer()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(theme::accent())
                                                .text_xs()
                                                .text_color(theme::text_on_accent())
                                                .font_weight(FontWeight::BOLD)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.settings_view.editing_password = false;
                                                    let _ = this.app_settings.save();
                                                    cx.notify();
                                                }))
                                                .child("Done"),
                                        )
                                })
                                .when(!is_editing, |box_el| {
                                    box_el
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.settings_view.editing_password = true;
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child(if pass_val.is_empty() {
                                                    "Click to type password...".to_string()
                                                } else {
                                                    pass_val
                                                }),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .text_xs()
                                                .text_color(theme::accent())
                                                .child(Icon::new(IconName::Pencil).size(px(11.0)))
                                                .child("Custom"),
                                        )
                                })
                        })
                        .children(
                            ["12345", "nucleus2026", "splitscreen", "gamerpass"]
                                .iter()
                                .enumerate()
                                .map(|(p_idx, &preset)| {
                                    let is_act = settings.nucleus_account_password == preset;
                                    div()
                                        .id(("pass-preset", p_idx))
                                        .cursor_pointer()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_md()
                                        .text_xs()
                                        .border_1()
                                        .when(is_act, |s| {
                                            s.bg(theme::accent())
                                                .border_color(theme::accent())
                                                .text_color(theme::text_on_accent())
                                                .font_weight(FontWeight::BOLD)
                                        })
                                        .when(!is_act, |s| {
                                            s.bg(theme::bg_panel())
                                                .border_color(theme::border_subtle())
                                                .text_color(theme::text_muted())
                                                .hover(|h| {
                                                    h.bg(theme::bg_hover())
                                                        .text_color(theme::text_primary())
                                                })
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.app_settings.nucleus_account_password =
                                                preset.to_string();
                                            this.settings_view.editing_password = false;
                                            let _ = this.app_settings.save();
                                            this.status_message =
                                                format!("Account password set to: {}", preset);
                                            cx.notify();
                                        }))
                                        .child(preset)
                                }),
                        ),
                ),
        )
        .child(render_toggle(
            "Allow Multiple Launcher Instances",
            "Allow running multiple concurrent Nucleus Co-op launcher windows",
            settings.nucleus_multi_instances,
            cx.listener(|this, _, _, cx| {
                this.app_settings.nucleus_multi_instances =
                    !this.app_settings.nucleus_multi_instances;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Disable Game Profiles",
            "Ignore cached userprofile.json configurations and always use default handler splits",
            settings.disable_game_profiles,
            cx.listener(|this, _, _, cx| {
                this.app_settings.disable_game_profiles = !this.app_settings.disable_game_profiles;
                let _ = this.app_settings.save();
                this.show_osd(
                    &format!(
                        "Game Profiles {}",
                        if this.app_settings.disable_game_profiles {
                            "Disabled"
                        } else {
                            "Enabled"
                        }
                    ),
                    600,
                    cx,
                );
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Ignore Input Lock Reminder",
            "Suppress the OSD reminder asking to lock inputs with the End key",
            settings.ignore_input_lock_reminder,
            cx.listener(|this, _, _, cx| {
                this.app_settings.ignore_input_lock_reminder =
                    !this.app_settings.ignore_input_lock_reminder;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Auto Desktop Scaling",
            "Automatically configure monitor DPI display scaling factor during launch",
            settings.auto_desktop_scaling,
            cx.listener(|this, _, _, cx| {
                this.app_settings.auto_desktop_scaling = !this.app_settings.auto_desktop_scaling;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Virtual Gamepad Mode Only",
            "Only assign simulated / virtual gamepads (useful for headless testing)",
            settings.vgm_only,
            cx.listener(|this, _, _, cx| {
                this.app_settings.vgm_only = !this.app_settings.vgm_only;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Developer Mode",
            "Enable developer tools, virtual gamepad injection, and advanced diagnostics",
            settings.developer_mode,
            cx.listener(|this, _, _, cx| {
                this.app_settings.developer_mode = !this.app_settings.developer_mode;
                let _ = this.app_settings.save();
                this.status_message = format!(
                    "Developer mode {}",
                    if this.app_settings.developer_mode {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Card: Community Hub Account & Ecosystem Integrations.
pub fn render_card_general_integrations(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    NefosCard::surface("card-general-integrations")
        .title_text("Community Hub & Advanced Integrations")
        .description_text("Hub account credentials, AutoHotkey runtime, and extensions")
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .py_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text_primary())
                                .child("Splitscreen.Me Hub Account"),
                        )
                        .child(div().text_xs().text_color(theme::text_muted()).child(
                            if let Some(ref acct) = win.hub_account_summary {
                                format!("Signed in as: {}", acct.username)
                            } else {
                                "Guest (Sign in to upload handlers, comment, and rate)".to_string()
                            },
                        )),
                )
                .child(
                    CouchButton::secondary("btn-settings-hub-account", "Manage Account")
                        .size(CouchButtonSize::Sm)
                        .icon(IconName::Globe)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.hub_account_modal.open();
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .py_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text_primary())
                                .child("Extensions & Runtime Add-ons"),
                        )
                        .child(div().text_xs().text_color(theme::text_muted()).child(
                            "Manage sandboxed WASM extensions, AutoHotkey scripts, and JS add-ons",
                        )),
                )
                .child(
                    CouchButton::secondary("btn-settings-extensions", "Open Extensions")
                        .size(CouchButtonSize::Sm)
                        .icon(IconName::LayoutGrid)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.active_tab = ActiveTab::Extensions;
                            this.refresh_extensions();
                            cx.notify();
                        })),
                ),
        )
        .into_any_element()
}

/// Renders Tab 1: General Settings, Languages, Network, and Isolation Flags.
fn render_general_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        // Shortcut Banner Jump
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .items_center()
                .justify_between()
                .p_4()
                .bg(theme::accent_subtle())
                .rounded_xl()
                .border_1()
                .border_color(theme::border_focus())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .flex_1()
                        .min_w(px(240.0))
                        .gap_3()
                        .child(div().text_base().child("⌨️"))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Custom Input Lock & Keyboard Shortcuts"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::text_muted())
                                        .child("Bind your own keys with live keypress detection or quick dropdowns"),
                                ),
                        ),
                )
                .child(
                    CouchButton::secondary("btn-jump-hotkeys", "Configure Hotkeys →")
                        .size(CouchButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_view.active_category = SettingsCategory::Hotkeys;
                            cx.notify();
                        })),
                ),
        )
        .child(render_card_general_lang_network(win, cx))
        .child(render_card_general_isolation(win, cx))
        .child(render_card_general_integrations(win, cx))
        .into_any_element()
}

/// Renders Card: Input Lock Toggle Key.
pub fn render_card_hotkeys_lock_key(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;
    let recording_target = win.settings_view.recording_hotkey.clone();
    let dropdown_target = win.settings_view.dropdown_hotkey.clone();
    let recording = recording_target.as_deref() == Some("lock");

    NefosCard::surface("card-hotkeys-lock-key")
        .title_text("Input Lock Toggle Key")
        .description_text("Global physical key monitored during gameplay to lock or unlock mouse and keyboard focus")
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .p_3()
                .bg(theme::bg_subtle())
                .rounded_lg()
                .border_1()
                .border_color(if recording_target.as_deref() == Some("lock") { theme::border_focus() } else { theme::border_subtle() })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(if recording_target.as_deref() == Some("lock") { theme::accent_subtle() } else { theme::bg_panel() })
                                .border_1()
                                .border_color(if recording_target.as_deref() == Some("lock") { theme::border_focus() } else { theme::border_color() })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if recording_target.as_deref() == Some("lock") { theme::accent() } else { theme::text_primary() })
                                .child(if recording_target.as_deref() == Some("lock") {
                                    "Press any key...".to_string()
                                } else {
                                    format!("[ {} ]", settings.lock_inputs_key)
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Default key is End (splitscreenme standard)"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            CouchButton::secondary(
                                "btn-record-lock-key",
                                if recording {
                                    "Listening for keypress..."
                                } else {
                                    "⌨️ Record New Key"
                                },
                            )
                            .size(CouchButtonSize::Sm)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.settings_view.recording_hotkey = Some("lock".to_string());
                                this.settings_view.dropdown_hotkey = None;
                                this.status_message = "Press any key on your keyboard to set Lock Inputs Key...".to_string();
                                if let Some(ref f) = this.hotkey_record_focus {
                                    f.focus(window, cx);
                                }
                                cx.notify();
                            })),
                        )
                        .child(
                            CouchButton::secondary("btn-dropdown-lock-key", "▾ Select Key")
                                .size(CouchButtonSize::Sm)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let cur = this.settings_view.dropdown_hotkey.as_deref();
                                    this.settings_view.dropdown_hotkey = if cur == Some("lock") { None } else { Some("lock".to_string()) };
                                    this.settings_view.recording_hotkey = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            CouchButton::secondary("btn-reset-lock-key", "Default ↺")
                                .size(CouchButtonSize::Sm)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.app_settings.lock_inputs_key = "End".to_string();
                                    this.status_message = "Lock key reset to default: End".to_string();
                                    this.show_osd("Lock Key: End", 600, cx);
                                    cx.notify();
                                })),
                        ),
                ),
        )
        // Dropdown key picker for lock key
        .when(dropdown_target.as_deref() == Some("lock"), |this| {
            this.child(render_lock_key_dropdown(&settings.lock_inputs_key, cx))
        })
        .into_any_element()
}

/// Renders Card: Controller Shortcuts & Gamepad UI Navigation.
pub fn render_card_hotkeys_controller(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let settings = &win.app_settings;
    NefosCard::surface("card-hotkeys-controller")
        .title_text("Controller Shortcuts & Gamepad UI Navigation")
        .description_text("Configure Guide button combos (Guide+Back, Guide+Y, etc.) and in-game UI navigation for controllers")
        .header_action(
            CouchButton::primary("btn-open-controller-shortcuts-modal", "Configure Shortcuts →")
                .icon(IconName::Gamepad2)
                .size(CouchButtonSize::Sm)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_controller_shortcuts_modal = true;
                    cx.notify();
                }))
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .p_3()
                .bg(theme::bg_main())
                .rounded_md()
                .border_1()
                .border_color(theme::border_color())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child("Enable Gamepad Shortcuts"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(format!("Active Gamepad Style: {} • Deadzone: {}", settings.gamepad_type, settings.gamepad_deadzone)),
                        ),
                )
                .child(
                    div()
                        .id("chk-gamepad-shortcuts-enabled")
                        .cursor_pointer()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(if settings.gamepad_shortcuts_enabled { theme::success_subtle() } else { theme::bg_active() })
                        .border_1()
                        .border_color(if settings.gamepad_shortcuts_enabled { theme::success() } else { theme::border_color() })
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if settings.gamepad_shortcuts_enabled { theme::success() } else { theme::text_muted() })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.app_settings.gamepad_shortcuts_enabled = !this.app_settings.gamepad_shortcuts_enabled;
                            this.status_message = format!("Controller shortcuts {}", if this.app_settings.gamepad_shortcuts_enabled { "enabled" } else { "disabled" });
                            cx.notify();
                        }))
                        .child(if settings.gamepad_shortcuts_enabled { "✓ Enabled" } else { "Disabled" }),
                ),
        )
        .into_any_element()
}

/// Renders Card: In-Game Global Shortcuts.
pub fn render_card_hotkeys_global(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;
    let recording_target = win.settings_view.recording_hotkey.clone();
    let dropdown_target = win.settings_view.dropdown_hotkey.clone();

    NefosCard::surface("card-hotkeys-global")
        .title_text("In-Game Global Shortcuts")
        .description_text("Keyboard hotkeys captured by the background engine thread during multi-instance sessions")
        .header_action(
            CouchButton::danger("btn-reset-all-shortcuts", "Reset to Defaults")
                .icon(IconName::RotateCcw)
                .size(CouchButtonSize::Sm)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.app_settings.hotkey_close = "Ctrl+Q".to_string();
                    this.app_settings.hotkey_stop = "Ctrl+E".to_string();
                    this.app_settings.hotkey_topmost = "Ctrl+T".to_string();
                    this.app_settings.hotkey_set_focus = "Ctrl+H".to_string();
                    this.app_settings.hotkey_reset_windows = "Ctrl+R".to_string();
                    this.app_settings.hotkey_cutscenes = "Ctrl+M".to_string();
                    this.app_settings.hotkey_switch = "Ctrl+N".to_string();
                    this.app_settings.hotkey_shortcuts_reminder = "Alt+R".to_string();
                    this.app_settings.hotkey_merger_switch = "Alt+A".to_string();
                    this.app_settings.hotkey_couch_osd = "F10".to_string();
                    this.status_message = "All shortcuts reset to defaults".to_string();
                    this.show_osd("Shortcuts Reset to Defaults", 800, cx);
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(APP_SHORTCUTS.iter().enumerate().map(|(idx, meta)| {
                    let cur_val = get_hotkey_value(settings, meta.id);
                    let is_rec = recording_target.as_deref() == Some(meta.id);
                    let is_drop = dropdown_target.as_deref() == Some(meta.id);
                    render_shortcut_row(idx, meta, cur_val, is_rec, is_drop, cx)
                })),
        )
        .into_any_element()
}

/// Renders Tab 2: Dedicated Hotkeys & Input Lock Command Center.
fn render_hotkeys_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let recording_target = win.settings_view.recording_hotkey.clone();

    div()
        .flex()
        .flex_col()
        .gap_5()
        // Keyboard Listener Banner (active when recording any hotkey via keypress)
        .when(recording_target.is_some(), |this| {
            let target_name = match recording_target.as_deref().unwrap_or("") {
                "lock" => "Lock Inputs Toggle Key".to_string(),
                other => APP_SHORTCUTS
                    .iter()
                    .find(|s| s.id == other)
                    .map(|s| s.name.to_string())
                    .unwrap_or_else(|| "Shortcut".to_string()),
            };

            this.child(
                div()
                    .id("hotkey-active-recorder")
                    .p_4()
                    .rounded_xl()
                    .border_2()
                    .border_color(theme::border_focus())
                    .bg(theme::accent_subtle())
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme::danger())
                                    .text_color(theme::text_on_accent())
                                    .font_weight(FontWeight::BOLD)
                                    .text_xs()
                                    .child(div().size(px(8.0)).rounded_full().bg(theme::text_on_accent()))
                                    .child("RECORDING"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::text_primary())
                                            .child(format!("Press any key on your keyboard for: {}", target_name)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child("Press desired key or key combination (e.g. End, F10, Ctrl+Q, Alt+W). Press Esc to cancel."),
                                    ),
                            ),
                    )
                    .child(
                        CouchButton::secondary("btn-cancel-recording", "Cancel (Esc)")
                            .size(CouchButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings_view.recording_hotkey = None;
                                this.status_message = "Key recording cancelled.".to_string();
                                cx.notify();
                            })),
                    ),
            )
        })
        .child(render_card_hotkeys_lock_key(win, cx))
        .child(render_card_hotkeys_controller(win, cx))
        .child(render_card_hotkeys_global(win, cx))
        .into_any_element()
}

/// Helper to render the quick dropdown picker for lock key.
fn render_lock_key_dropdown(current_key: &str, cx: &mut Context<MainWindow>) -> AnyElement {
    let keys = [
        "End", "Home", "Delete", "Multiply", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9",
        "F10", "F11", "F12",
    ];
    let key_items = keys.iter().enumerate().map(|(idx, &k)| {
        let is_current = current_key.eq_ignore_ascii_case(k);
        let k_val = k.to_string();
        div()
            .id(("lock-key-opt", idx))
            .cursor_pointer()
            .px_3()
            .py_1()
            .rounded_md()
            .text_xs()
            .border_1()
            .when(is_current, |this| {
                this.bg(theme::accent())
                    .border_color(theme::accent())
                    .text_color(theme::text_on_accent())
                    .font_weight(FontWeight::BOLD)
            })
            .when(!is_current, |this| {
                this.bg(theme::bg_panel())
                    .border_color(theme::border_color())
                    .text_color(theme::text_primary())
                    .hover(|s| s.bg(theme::bg_hover()))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.app_settings.lock_inputs_key = k_val.clone();
                this.settings_view.dropdown_hotkey = None;
                this.status_message = format!("Lock key set to {}", k_val);
                this.show_osd(&format!("Lock Key: {}", k_val), 700, cx);
                cx.notify();
            }))
            .child(k)
    });

    div()
        .p_3()
        .rounded_md()
        .bg(theme::bg_subtle())
        .border_1()
        .border_color(theme::border_color())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_muted())
                .child("Choose Lock Inputs Key:"),
        )
        .child(div().flex().flex_wrap().gap_1p5().children(key_items))
        .into_any_element()
}

/// Helper to render the quick dropdown picker for a shortcut.
fn render_shortcut_dropdown(
    sc_id: &'static str,
    cur_val: &str,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let combos = [
        "Ctrl+Q", "Ctrl+S", "Ctrl+T", "Ctrl+F", "Ctrl+R", "Ctrl+M", "Ctrl+W", "Ctrl+H", "Ctrl+Tab",
        "Alt+Q", "Alt+W", "Alt+F4", "Shift+F1", "Shift+F2", "Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4",
    ];
    let items = combos.iter().enumerate().map(|(p_idx, &combo)| {
        let is_sel = cur_val.eq_ignore_ascii_case(combo);
        let combo_str = combo.to_string();
        div()
            .id(("combo-opt", p_idx))
            .cursor_pointer()
            .px_2p5()
            .py_1()
            .rounded_md()
            .text_xs()
            .border_1()
            .when(is_sel, |this| {
                this.bg(theme::accent())
                    .border_color(theme::accent())
                    .text_color(theme::text_on_accent())
                    .font_weight(FontWeight::BOLD)
            })
            .when(!is_sel, |this| {
                this.bg(theme::bg_subtle())
                    .border_color(theme::border_subtle())
                    .text_color(theme::text_primary())
                    .hover(|s| s.bg(theme::bg_hover()))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                set_hotkey_value(&mut this.app_settings, sc_id, combo_str.clone());
                this.settings_view.dropdown_hotkey = None;
                this.status_message = format!("Shortcut set to {}", combo_str);
                this.show_osd(&format!("Shortcut: {}", combo_str), 700, cx);
                cx.notify();
            }))
            .child(combo)
    });

    div()
        .p_3()
        .rounded_md()
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_color())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_muted())
                .child("Quick Select Hotkey Key:"),
        )
        .child(div().flex().flex_wrap().gap_1p5().children(items))
        .into_any_element()
}

/// Helper to render an individual shortcut row.
fn render_shortcut_row(
    idx: usize,
    meta: &'static ShortcutMeta,
    cur_val: String,
    is_rec: bool,
    is_drop: bool,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let sc_id = meta.id;
    let name = meta.name;
    let description = meta.description;
    let cur_val_display = cur_val.clone();

    let mut row = div().flex().flex_col().gap_1().child(
        div()
            .id(("shortcut-row", idx))
            .flex()
            .items_center()
            .justify_between()
            .px_3p5()
            .py_2()
            .bg(theme::bg_subtle())
            .rounded_lg()
            .border_1()
            .border_color(if is_rec {
                theme::border_focus()
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
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text_primary())
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(description),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(if is_rec {
                                theme::accent_subtle()
                            } else {
                                theme::bg_panel()
                            })
                            .border_1()
                            .border_color(if is_rec {
                                theme::border_focus()
                            } else {
                                theme::border_color()
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_rec {
                                theme::accent()
                            } else {
                                theme::text_primary()
                            })
                            .child(if is_rec {
                                "Press keys...".to_string()
                            } else {
                                cur_val_display
                            }),
                    )
                    .child(
                        CouchButton::secondary(
                            ("btn-record-sc", idx),
                            if is_rec { "Listening..." } else { "Record" },
                        )
                        .size(CouchButtonSize::Sm)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.settings_view.recording_hotkey = Some(sc_id.to_string());
                                this.settings_view.dropdown_hotkey = None;
                                this.status_message =
                                    format!("Press key or shortcut combination for {}...", name);
                                if let Some(ref f) = this.hotkey_record_focus {
                                    f.focus(window, cx);
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        CouchButton::secondary(("btn-drop-sc", idx), "Select ▾")
                            .size(CouchButtonSize::Sm)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let cur = this.settings_view.dropdown_hotkey.as_deref();
                                this.settings_view.dropdown_hotkey = if cur == Some(sc_id) {
                                    None
                                } else {
                                    Some(sc_id.to_string())
                                };
                                this.settings_view.recording_hotkey = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        CouchButton::secondary(("btn-reset-sc", idx), "↺")
                            .size(CouchButtonSize::Sm)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let def = get_default_hotkey(sc_id).to_string();
                                set_hotkey_value(&mut this.app_settings, sc_id, def.clone());
                                this.status_message = format!("Shortcut reset to: {}", def);
                                this.show_osd(&format!("Shortcut: {}", def), 600, cx);
                                cx.notify();
                            })),
                    ),
            ),
    );

    if is_drop {
        row = row.child(render_shortcut_dropdown(sc_id, &cur_val, cx));
    }

    row.into_any_element()
}

/// Renders Card: Player Management & Options.
pub fn render_card_players_controls(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;
    let page = win.settings_view.player_page;

    NefosCard::surface("card-players-controls")
        .title_text("Player Management & Options")
        .description_text(
            "Switch between player slots 1-16 and 17-32, toggle assignment rules, and batch reset",
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id("btn-player-page-1")
                                .cursor_pointer()
                                .px_3p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .when(page == 0, |this| {
                                    this.bg(theme::accent()).text_color(theme::text_on_accent())
                                })
                                .when(page != 0, |this| {
                                    this.bg(theme::bg_subtle())
                                        .text_color(theme::text_muted())
                                        .hover(|s| {
                                            s.bg(theme::bg_hover())
                                                .text_color(theme::text_primary())
                                        })
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings_view.player_page = 0;
                                    cx.notify();
                                }))
                                .child("Page 1 (Players 1 - 16)"),
                        )
                        .child(
                            div()
                                .id("btn-player-page-2")
                                .cursor_pointer()
                                .px_3p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .when(page == 1, |this| {
                                    this.bg(theme::accent()).text_color(theme::text_on_accent())
                                })
                                .when(page != 1, |this| {
                                    this.bg(theme::bg_subtle())
                                        .text_color(theme::text_muted())
                                        .hover(|s| {
                                            s.bg(theme::bg_hover())
                                                .text_color(theme::text_primary())
                                        })
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings_view.player_page = 1;
                                    cx.notify();
                                }))
                                .child("Page 2 (Players 17 - 32)"),
                        )
                        .child(
                            CouchButton::danger(
                                "btn-reset-players",
                                "Reset All 32 Players to Defaults",
                            )
                            .icon(IconName::RotateCcw)
                            .size(CouchButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                for p in 0..32 {
                                    this.app_settings
                                        .set_player_nickname(p, format!("Player{}", p + 1));
                                    this.app_settings.set_player_steam_id(
                                        p,
                                        format!("765611980000000{:02}", p + 1),
                                    );
                                }
                                this.status_message =
                                    "All 32 player profiles reset to defaults".to_string();
                                this.show_osd("Profiles Reset to Default", 800, cx);
                                cx.notify();
                            })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(render_mini_toggle(
                            "Assign by Button Press",
                            settings.profile_assign_gamepad_by_button_press,
                            cx.listener(|this, _, _, cx| {
                                this.app_settings.profile_assign_gamepad_by_button_press =
                                    !this.app_settings.profile_assign_gamepad_by_button_press;
                                cx.notify();
                            }),
                        ))
                        .child(render_mini_toggle(
                            "Use XInput Index",
                            settings.use_xinput_index,
                            cx.listener(|this, _, _, cx| {
                                this.app_settings.use_xinput_index =
                                    !this.app_settings.use_xinput_index;
                                cx.notify();
                            }),
                        )),
                ),
        )
        .into_any_element()
}

/// Renders Card: Player Profiles.
pub fn render_card_players_slots(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;
    let page = win.settings_view.player_page;
    let (start_idx, end_idx) = if page == 0 { (0, 16) } else { (16, 32) };

    NefosCard::surface("card-players-slots")
        .title_text(format!(
            "Player Profiles (Slots {} - {})",
            start_idx + 1,
            end_idx
        ))
        .description_text("Customize player nicknames and Steam IDs (1 - 32) passed into emulators")
        .child(
            // Table Header
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_1p5()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_muted())
                .child(div().w(px(80.0)).child("Player Slot"))
                .child(div().flex_1().child("In-Game Nickname"))
                .child(div().w(px(320.0)).child("Steam App ID / Unique User ID")),
        )
        .children((start_idx..end_idx).map(|idx| {
            let nick = settings.get_player_nickname(idx);
            let steam_id = settings.get_player_steam_id(idx);
            render_player_row(idx, nick, steam_id, win.settings_view.editing_player, cx)
        }))
        .into_any_element()
}

/// Renders Tab 3: Players (1 to 32) Nicknames and Steam IDs.
fn render_players_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(render_card_players_controls(win, cx))
        .child(render_card_players_slots(win, cx))
        .into_any_element()
}

/// Helper to render an individual player profile slot row.
fn render_player_row(
    idx: usize,
    nick: String,
    steam_id: String,
    editing_player: Option<(usize, PlayerEditField)>,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let is_valid = nucleus_profile::AppSettings::is_valid_steam_id(&steam_id);
    let nick_val = nick.clone();
    let steam_id_val = steam_id.clone();

    div()
        .id(("player-row", idx))
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .bg(theme::bg_subtle())
        .rounded_lg()
        .border_1()
        .border_color(theme::border_subtle())
        .text_xs()
        .child(
            div().w(px(80.0)).flex().items_center().gap_1p5().child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(theme::accent())
                    .text_color(theme::text_on_accent())
                    .font_weight(FontWeight::BOLD)
                    .child(format!("#{}", idx + 1)),
            ),
        )
        .child({
            let is_editing_nick = editing_player == Some((idx, PlayerEditField::Nickname));
            div()
                .id(("input-nick-box", idx))
                .flex_1()
                .cursor_text()
                .flex()
                .items_center()
                .justify_between()
                .px_2p5()
                .py_1()
                .rounded_md()
                .border_1()
                .when(is_editing_nick, |this| {
                    this.bg(theme::bg_panel())
                        .border_color(theme::accent())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_color(theme::text_primary())
                                        .font_weight(FontWeight::BOLD)
                                        .child(format!("{}|", nick_val)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::accent())
                                        .child("(Type to rename)"),
                                ),
                        )
                        .child(
                            div()
                                .id(("btn-done-nick", idx))
                                .cursor_pointer()
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .bg(theme::accent())
                                .text_xs()
                                .text_color(theme::text_on_accent())
                                .font_weight(FontWeight::BOLD)
                                .child("Done"),
                        )
                })
                .when(!is_editing_nick, |this| {
                    this.bg(theme::bg_active())
                        .border_color(theme::border_color())
                        .hover(|s| s.bg(theme::bg_hover()).border_color(theme::border_color()))
                        .child(
                            div()
                                .text_color(theme::text_primary())
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(nick_val),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(Icon::new(IconName::Pencil).size(px(12.0)))
                                .child("Edit"),
                        )
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.settings_view.editing_player == Some((idx, PlayerEditField::Nickname)) {
                        this.settings_view.editing_player = None;
                    } else {
                        this.settings_view.editing_player = Some((idx, PlayerEditField::Nickname));
                    }
                    cx.notify();
                }))
        })
        .child({
            let is_editing_sid = editing_player == Some((idx, PlayerEditField::SteamId));
            div()
                .id(("input-sid-box", idx))
                .w(px(320.0))
                .cursor_text()
                .flex()
                .items_center()
                .justify_between()
                .px_2p5()
                .py_1()
                .rounded_md()
                .border_1()
                .when(is_editing_sid, |this| {
                    this.bg(theme::bg_panel())
                        .border_color(theme::accent())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_color(theme::text_primary())
                                        .font_weight(FontWeight::BOLD)
                                        .child(format!("{}|", steam_id_val)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::accent())
                                        .child("(17 digits)"),
                                ),
                        )
                        .child(
                            div()
                                .id(("btn-done-sid", idx))
                                .cursor_pointer()
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .bg(theme::accent())
                                .text_xs()
                                .text_color(theme::text_on_accent())
                                .font_weight(FontWeight::BOLD)
                                .child("Done"),
                        )
                })
                .when(!is_editing_sid, |this| {
                    this.bg(theme::bg_active())
                        .border_color(if is_valid {
                            theme::border_color()
                        } else {
                            theme::danger()
                        })
                        .hover(|s| s.bg(theme::bg_hover()).border_color(theme::border_color()))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_color(if is_valid {
                                            theme::text_muted()
                                        } else {
                                            theme::danger()
                                        })
                                        .child(steam_id_val),
                                )
                                .when(!is_valid, |el| {
                                    el.child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_md()
                                            .bg(theme::danger())
                                            .text_color(theme::text_on_accent())
                                            .text_xs()
                                            .child("Invalid"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(Icon::new(IconName::Pencil).size(px(12.0)))
                                .child("Edit"),
                        )
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.settings_view.editing_player == Some((idx, PlayerEditField::SteamId)) {
                        this.settings_view.editing_player = None;
                    } else {
                        this.settings_view.editing_player = Some((idx, PlayerEditField::SteamId));
                    }
                    cx.notify();
                }))
        })
        .into_any_element()
}

/// Renders Card: Audio Output Device Mode.
pub fn render_card_audio_mode(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-audio-mode")
        .title_text("Audio Output Device Mode")
        .description_text(
            "Select between system default audio or separate routing for each player instance",
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .w_full()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Output routing preference:"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id("audio-mode-default")
                                .cursor_pointer()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .when(!settings.audio_custom, |this| {
                                    this.bg(theme::accent())
                                        .border_color(theme::accent())
                                        .text_color(theme::text_on_accent())
                                        .font_weight(FontWeight::BOLD)
                                })
                                .when(settings.audio_custom, |this| {
                                    this.bg(theme::bg_subtle())
                                        .border_color(theme::border_subtle())
                                        .text_color(theme::text_muted())
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.app_settings.audio_custom = false;
                                    cx.notify();
                                }))
                                .child("Default Output Device"),
                        )
                        .child(
                            div()
                                .id("audio-mode-custom")
                                .cursor_pointer()
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .when(settings.audio_custom, |this| {
                                    this.bg(theme::accent())
                                        .border_color(theme::accent())
                                        .text_color(theme::text_on_accent())
                                        .font_weight(FontWeight::BOLD)
                                })
                                .when(!settings.audio_custom, |this| {
                                    this.bg(theme::bg_subtle())
                                        .border_color(theme::border_subtle())
                                        .text_color(theme::text_muted())
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.app_settings.audio_custom = true;
                                    cx.notify();
                                }))
                                .child("Custom Per-Instance Routing"),
                        ),
                ),
        )
        .into_any_element()
}

/// Renders Card: Per-Instance Audio Endpoints.
pub fn render_card_audio_endpoints(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;
    let current_page = win.settings_view.audio_page.min(3);
    let start_idx = current_page * 8;
    let end_idx = start_idx + 8;

    NefosCard::surface("card-audio-endpoints")
        .title_text(format!("Per-Instance Audio Endpoints (Instance {} through {})", start_idx + 1, end_idx))
        .description_text("Assign physical or virtual audio output playback devices to individual player instances (up to 32 players)")
        .header_action(
            div()
                .flex()
                .items_center()
                .gap_2()
                .children([0, 1, 2, 3].into_iter().map(|page| {
                    let is_active = page == current_page;
                    let label = format!("{}-{}", page * 8 + 1, (page + 1) * 8);
                    div()
                        .id(("audio-page-btn", page))
                        .cursor_pointer()
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .bg(if is_active { theme::accent() } else { theme::bg_subtle() })
                        .text_color(if is_active { theme::text_on_accent() } else { theme::text_muted() })
                        .border_1()
                        .border_color(if is_active { theme::accent() } else { theme::border_color() })
                        .hover(|s| s.bg(theme::bg_hover()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_view.audio_page = page;
                            this.settings_view.dropdown_audio_instance = None;
                            cx.notify();
                        }))
                        .child(label)
                }))
                .child(
                    CouchButton::secondary("btn-refresh-audio", "Refresh")
                        .icon(IconName::RefreshCw)
                        .size(CouchButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.status_message = "Audio endpoints refreshed".to_string();
                            this.show_osd("Audio Devices Refreshed", 800, cx);
                            cx.notify();
                        }))
                )
        )
        .children((start_idx..end_idx).map(|idx| {
            let dev = settings.get_audio_instance(idx);
            let is_open = win.settings_view.dropdown_audio_instance == Some(idx);
            render_audio_instance_row(idx, dev.to_string(), is_open, cx)
        }))
        .into_any_element()
}

/// Renders Tab 4: Audio Routing.
fn render_audio_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(render_card_audio_mode(win, cx))
        .child(render_card_audio_endpoints(win, cx))
        .into_any_element()
}

/// Helper to render an individual audio instance routing slot row.
fn render_audio_instance_row(
    idx: usize,
    dev: String,
    is_dropdown_open: bool,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let devices = [
        "Default",
        "Speakers (Realtek High Definition Audio)",
        "Headphones (Realtek High Definition Audio)",
        "Digital Audio (S/PDIF)",
        "CABLE Input (VB-Audio Virtual Cable)",
        "Steam Streaming Speakers",
    ];

    div()
        .id(("audio-inst", idx))
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .bg(theme::bg_subtle())
        .rounded_lg()
        .border_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .flex_shrink_0()
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .bg(theme::bg_active())
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child(format!("Instance #{}", idx + 1)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(format!("Audio endpoint assignment for Player {}", idx + 1)),
                        ),
                )
                // Dropdown trigger button showing currently selected audio device
                .child(
                    div()
                        .id(("btn-audio-select", idx))
                        .flex_shrink_0()
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
                            this.settings_view.dropdown_audio_instance =
                                if this.settings_view.dropdown_audio_instance == Some(idx) {
                                    None
                                } else {
                                    Some(idx)
                                };
                            cx.notify();
                        }))
                        .child(div().max_w(px(240.0)).child(dev.clone()))
                        .child(
                            div()
                                .text_color(theme::accent())
                                .child(if is_dropdown_open { "▲" } else { "▼" }),
                        ),
                ),
        )
        // Expandable dropdown options list when open
        .when(is_dropdown_open, |this| {
            let cur_dev = dev.clone();
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .p_2()
                    .rounded_md()
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(theme::accent())
                    .gap_1()
                    .children(devices.iter().enumerate().map(|(d_idx, &device_name)| {
                        let is_selected = device_name == cur_dev;
                        let d_name = device_name.to_string();
                        div()
                            .id(("audio-opt", idx * 100 + d_idx))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .when(is_selected, |s| {
                                s.bg(theme::accent().opacity(0.15))
                                    .text_color(theme::accent())
                                    .font_weight(FontWeight::BOLD)
                            })
                            .when(!is_selected, |s| {
                                s.text_color(theme::text_primary())
                                    .hover(|h| h.bg(theme::bg_hover()))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.app_settings.set_audio_instance(idx, &d_name);
                                this.settings_view.dropdown_audio_instance = None;
                                this.status_message =
                                    format!("Instance {} audio set to {}", idx + 1, d_name);
                                cx.notify();
                            }))
                            .child(device_name)
                            .child(if is_selected { "✓" } else { "" })
                    })),
            )
        })
        .into_any_element()
}

/// Renders Card: Windows Merger Tool.
pub fn render_card_layout_merger(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-layout-merger")
        .title_text("Windows Merger Tool")
        .description_text("Composite all instance viewports into a single master container window")
        .child(render_toggle(
            "Enable Windows Merger",
            "Composite all instance viewports into a single master container window",
            settings.windows_merger,
            cx.listener(|this, _, _, cx| {
                this.app_settings.windows_merger = !this.app_settings.windows_merger;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Lossless Hook",
            "Capture game backbuffers with lossless zero-copy shared memory hook",
            settings.lossless_hook,
            cx.listener(|this, _, _, cx| {
                this.app_settings.lossless_hook = !this.app_settings.lossless_hook;
                cx.notify();
            }),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Merger Target Window Resolution:"),
                )
                .child(
                    div().flex().flex_wrap().gap_1p5().children(
                        [
                            "Screen Native",
                            "1920x1080",
                            "2560x1440",
                            "3840x2160",
                            "1280x720",
                        ]
                        .iter()
                        .enumerate()
                        .map(|(idx, &res)| {
                            let is_active = settings.merger_resolution.eq_ignore_ascii_case(res)
                                || (settings.merger_resolution.is_empty()
                                    && res.eq_ignore_ascii_case("Screen Native"));
                            let res_val = res.to_string();
                            div()
                                .id(("merger-res", idx))
                                .cursor_pointer()
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .border_1()
                                .when(is_active, |this| {
                                    this.bg(theme::accent())
                                        .border_color(theme::accent())
                                        .text_color(theme::text_on_accent())
                                        .font_weight(FontWeight::SEMIBOLD)
                                })
                                .when(!is_active, |this| {
                                    this.bg(theme::bg_subtle())
                                        .border_color(theme::border_subtle())
                                        .text_color(theme::text_muted())
                                        .hover(|s| s.bg(theme::bg_hover()))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.app_settings.merger_resolution = res_val.clone();
                                    this.status_message =
                                        format!("Merger resolution set to {}", res_val);
                                    cx.notify();
                                }))
                                .child(res)
                        }),
                    ),
                ),
        )
        .into_any_element()
}

/// Renders Card: Visual Viewport Dividers.
pub fn render_card_layout_dividers(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-layout-dividers")
        .title_text("Visual Viewport Dividers")
        .description_text("Render customizable visual separation bars between game viewports")
        .child(render_toggle(
            "Enable Split Dividers",
            "Render customizable visual separation bars between game viewports",
            settings.split_div,
            cx.listener(|this, _, _, cx| {
                this.app_settings.split_div = !this.app_settings.split_div;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Hide Windows Desktop",
            "Conceal desktop icons, wallpapers, and taskbar during active session",
            settings.hide_desktop,
            cx.listener(|this, _, _, cx| {
                this.app_settings.hide_desktop = !this.app_settings.hide_desktop;
                cx.notify();
            }),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Divider Bar Color:"),
                )
                .child(
                    div().flex().flex_wrap().gap_1p5().children(
                        ["Black", "White", "Gray", "Red", "Green", "Blue", "Cyan"]
                            .iter()
                            .enumerate()
                            .map(|(idx, &col)| {
                                let is_active = settings.split_div_color.eq_ignore_ascii_case(col);
                                let col_val = col.to_string();
                                div()
                                    .id(("split-col", idx))
                                    .cursor_pointer()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .border_1()
                                    .when(is_active, |this| {
                                        this.bg(theme::accent())
                                            .border_color(theme::accent())
                                            .text_color(theme::text_on_accent())
                                            .font_weight(FontWeight::SEMIBOLD)
                                    })
                                    .when(!is_active, |this| {
                                        this.bg(theme::bg_subtle())
                                            .border_color(theme::border_subtle())
                                            .text_color(theme::text_muted())
                                            .hover(|s| s.bg(theme::bg_hover()))
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.app_settings.split_div_color = col_val.clone();
                                        this.status_message =
                                            format!("Divider color set to {}", col_val);
                                        cx.notify();
                                    }))
                                    .child(col)
                            }),
                    ),
                ),
        )
        .child(render_stepper_row(
            "Horizontal Dividers (Rows)",
            "Number of horizontal divider lines across display screens (0 - 10)",
            "hor-lines",
            settings.horizontal_lines,
            cx.listener(|this, _, _, cx| {
                this.app_settings.horizontal_lines =
                    this.app_settings.horizontal_lines.saturating_sub(1);
                cx.notify();
            }),
            cx.listener(|this, _, _, cx| {
                this.app_settings.horizontal_lines =
                    (this.app_settings.horizontal_lines + 1).min(10);
                cx.notify();
            }),
        ))
        .child(render_stepper_row(
            "Vertical Dividers (Columns)",
            "Number of vertical divider lines across display screens (0 - 10)",
            "ver-lines",
            settings.vertical_lines,
            cx.listener(|this, _, _, cx| {
                this.app_settings.vertical_lines =
                    this.app_settings.vertical_lines.saturating_sub(1);
                cx.notify();
            }),
            cx.listener(|this, _, _, cx| {
                this.app_settings.vertical_lines = (this.app_settings.vertical_lines + 1).min(10);
                cx.notify();
            }),
        ))
        .child(render_stepper_row(
            "Maximum Players in Custom Layout",
            "Upper limit on player slots partitioned across layouts (1 - 32)",
            "max-players",
            settings.max_players,
            cx.listener(|this, _, _, cx| {
                this.app_settings.max_players =
                    this.app_settings.max_players.saturating_sub(1).max(1);
                cx.notify();
            }),
            cx.listener(|this, _, _, cx| {
                this.app_settings.max_players = (this.app_settings.max_players + 1).min(32);
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Card: Cutscenes Presentation Flags.
pub fn render_card_layout_cutscenes(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-layout-cutscenes")
        .title_text("Cutscenes Presentation Flags")
        .description_text(
            "Maintain original aspect ratio and handle audio during cutscene transitions",
        )
        .child(render_toggle(
            "Keep Aspect Ratio",
            "Maintain original aspect ratio during cutscene full-screen transitions",
            settings.cts_keep_aspect_ratio,
            cx.listener(|this, _, _, cx| {
                this.app_settings.cts_keep_aspect_ratio = !this.app_settings.cts_keep_aspect_ratio;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Mute Inactive Audio Only",
            "Only mute secondary player audio during cutscenes without altering focus",
            settings.cts_mute_audio_only,
            cx.listener(|this, _, _, cx| {
                this.app_settings.cts_mute_audio_only = !this.app_settings.cts_mute_audio_only;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Unfocus Viewports on Cutscene",
            "Temporarily relinquish focus from game instances during cinematic cutscenes",
            settings.cts_unfocus,
            cx.listener(|this, _, _, cx| {
                this.app_settings.cts_unfocus = !this.app_settings.cts_unfocus;
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Tab 5: Layout & Windows Merger.
fn render_layout_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(render_card_layout_merger(win, cx))
        .child(render_card_layout_dividers(win, cx))
        .child(render_card_layout_cutscenes(win, cx))
        .into_any_element()
}

/// Renders Card: Visual Theme & Palette.
pub fn render_card_appearance_theme(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let current_theme = win.theme_name.clone();

    NefosCard::surface("card-appearance-theme")
        .title_text("Visual Theme & Palette (Adobe Spectrum 2)")
        .description_text("Choose interface appearance and base light/dark color scheme")
        .child(
            div().flex().gap_3().children(
                [
                    (
                        "Spectrum 2 Dark",
                        "Default dark palette with Spectrum blue accents",
                        rgb(0x1d1d1f),
                        rgb(0x2680eb),
                    ),
                    (
                        "Slate Dark",
                        "Cool slate dark palette with cyan highlights",
                        rgb(0x0f172a),
                        rgb(0x38bdf8),
                    ),
                    (
                        "Spectrum 2 Light",
                        "Clean, high-contrast light palette with deep blue",
                        rgb(0xf5f5f7),
                        rgb(0x0265dc),
                    ),
                ]
                .into_iter()
                .enumerate()
                .map(|(idx, (tname, desc, bg_dot, acc_dot))| {
                    let is_active = current_theme.eq_ignore_ascii_case(tname)
                        || (tname == "Spectrum 2 Dark"
                            && (current_theme.eq_ignore_ascii_case("Dark")
                                || current_theme.is_empty()))
                        || (tname == "Slate Dark"
                            && (current_theme.eq_ignore_ascii_case("Slate")
                                || current_theme.to_lowercase().contains("slate")))
                        || (tname == "Spectrum 2 Light"
                            && (current_theme.eq_ignore_ascii_case("Light")
                                || current_theme.to_lowercase().contains("light")));
                    let name_str = tname.to_string();
                    div()
                        .id(("theme-opt", idx))
                        .cursor_pointer()
                        .flex_1()
                        .p_3p5()
                        .rounded_lg()
                        .border_2()
                        .when(is_active, |this| {
                            this.bg(theme::accent_subtle())
                                .border_color(theme::border_focus())
                        })
                        .when(!is_active, |this| {
                            this.bg(theme::bg_subtle())
                                .border_color(theme::border_subtle())
                                .hover(|s| {
                                    s.bg(theme::bg_hover()).border_color(theme::border_color())
                                })
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.theme_name = name_str.clone();
                            this.app_settings.theme = name_str.clone();
                            theme::set_active_theme(&name_str);
                            if let Some(ref cb) = this.callbacks.on_theme_changed {
                                cb(&name_str);
                            }
                            this.status_message = format!("Theme switched to {}", name_str);
                            cx.notify();
                        }))
                        .child(
                            div()
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
                                                .text_color(theme::text_primary())
                                                .child(tname),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .size(px(10.0))
                                                        .rounded_full()
                                                        .bg(bg_dot)
                                                        .border_1()
                                                        .border_color(theme::border_color()),
                                                )
                                                .child(
                                                    div().size(px(10.0)).rounded_full().bg(acc_dot),
                                                ),
                                        ),
                                )
                                .child(div().text_xs().text_color(theme::text_muted()).child(desc)),
                        )
                }),
            ),
        )
        .into_any_element()
}

/// Renders Card: Accent Color Palette.
pub fn render_card_appearance_accent(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-appearance-accent")
        .title_text("Accent Color Palette")
        .description_text(
            "Select global primary accent highlight across buttons, badges, and focus rings",
        )
        .header_action(
            NefosTag::new(format!("Active: {}", theme::active_accent_name()))
                .id("badge-active-accent")
                .tone(NefosTagTone::Accent),
        )
        .child(
            div().flex().flex_wrap().gap_2p5().children(
                [
                    ("Blue", "Classic Spectrum Blue", rgb(0x2680eb)),
                    ("Purple", "Royal Violet", rgb(0x8b5cf6)),
                    ("Emerald", "Vibrant Green", rgb(0x10b981)),
                    ("Rose", "Crimson Red", rgb(0xf43f5e)),
                    ("Amber", "Golden Orange", rgb(0xf59e0b)),
                    ("Cyan", "Electric Teal", rgb(0x06b6d4)),
                    ("Indigo", "Deep Indigo", rgb(0x6366f1)),
                ]
                .into_iter()
                .enumerate()
                .map(|(idx, (cname, cdesc, color_dot))| {
                    let is_active = settings.accent_color.eq_ignore_ascii_case(cname)
                        || (settings.accent_color.is_empty() && cname == "Blue");
                    let color_name = cname.to_string();
                    div()
                        .id(("accent-opt", idx))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .px_3p5()
                        .py_2()
                        .rounded_lg()
                        .border_2()
                        .when(is_active, |this| {
                            this.bg(theme::bg_active()).border_color(color_dot)
                        })
                        .when(!is_active, |this| {
                            this.bg(theme::bg_subtle())
                                .border_color(theme::border_subtle())
                                .hover(|s| {
                                    s.bg(theme::bg_hover()).border_color(theme::border_color())
                                })
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.app_settings.accent_color = color_name.clone();
                            theme::set_active_accent(&color_name);
                            this.status_message = format!("Accent color set to {}", color_name);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(14.0))
                                .rounded_full()
                                .bg(color_dot)
                                .shadow_sm(),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .when(is_active, |this| this.text_color(color_dot))
                                        .when(!is_active, |this| {
                                            this.text_color(theme::text_primary())
                                        })
                                        .child(cname),
                                )
                                .child(
                                    div().text_xs().text_color(theme::text_muted()).child(cdesc),
                                ),
                        )
                        .when(is_active, |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(color_dot)
                                    .child("✓"),
                            )
                        })
                }),
            ),
        )
        .into_any_element()
}

/// Renders Card: On-Screen Display (OSD) HUD Banner.
pub fn render_card_appearance_osd(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-appearance-osd")
        .title_text("On-Screen Display (OSD) HUD Banner")
        .description_text("Show floating transparent status banners during game launch, repositioning, and hotkey presses")
        .child(render_toggle(
            "Enable In-Game On-Screen Display (OSD)",
            "Show floating transparent status banners during game launch, repositioning, and hotkey presses",
            settings.osd_enabled,
            cx.listener(|this, _, _, cx| {
                this.app_settings.osd_enabled = !this.app_settings.osd_enabled;
                this.status_message = format!("In-Game OSD {}", if this.app_settings.osd_enabled { "enabled" } else { "disabled" });
                cx.notify();
            }),
        ))
        // Live OSD HUD Preview
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Live OSD Banner Preview & Color:"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_3()
                        .bg(theme::bg_subtle())
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border_subtle())
                        .child(
                            div()
                                .px_5()
                                .py_2()
                                .rounded_md()
                                .bg(theme::overlay_scrim())
                                .border_2()
                                .border_color(parse_osd_color(&settings.osd_color))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(parse_osd_color(&settings.osd_color))
                                .child("  Nucleus Co-op: Inputs Locked (End)  "),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1p5()
                                .children([
                                    ("White", "255,255,255", rgb(0xffffff)),
                                    ("Cyan", "0,200,255", rgb(0x00c8ff)),
                                    ("Green", "0,255,128", rgb(0x00ff80)),
                                    ("Amber", "255,180,0", rgb(0xffb400)),
                                ].into_iter().enumerate().map(|(idx, (label, color_code, col_val))| {
                                    let is_active = settings.osd_color.trim() == color_code;
                                    let c_val = color_code.to_string();
                                    div()
                                        .id(("osd-col", idx))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .border_1()
                                        .when(is_active, |this| {
                                            this.bg(theme::accent_subtle()).border_color(theme::border_focus()).text_color(theme::accent()).font_weight(FontWeight::BOLD)
                                        })
                                        .when(!is_active, |this| {
                                            this.bg(theme::bg_panel()).border_color(theme::border_color()).text_color(theme::text_muted())
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.app_settings.osd_color = c_val.clone();
                                            this.status_message = format!("OSD color set to {}", c_val);
                                            this.show_osd("OSD Color Updated", 800, cx);
                                            cx.notify();
                                        }))
                                        .child(div().size(px(8.0)).rounded_full().bg(col_val))
                                        .child(label)
                                })),
                        ),
                )
        )
        .child(render_stepper_row(
            "Background Blur Strength",
            "Visual backdrop blur radius applied during modal dialog presentations (0 - 20)",
            "blur-val",
            settings.blur,
            cx.listener(|this, _, _, cx| {
                this.app_settings.blur = this.app_settings.blur.saturating_sub(1);
                cx.notify();
            }),
            cx.listener(|this, _, _, cx| {
                this.app_settings.blur = (this.app_settings.blur + 1).min(20);
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Card: Game Handlers Directory.
pub fn render_card_appearance_handlers_dir(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let handlers_path = win.handlers_dir.clone();

    NefosCard::surface("card-appearance-handlers-dir")
        .title_text("Game Handlers Directory")
        .description_text("Custom filesystem path where downloaded and local .js handlers reside")
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .p_2p5()
                        .bg(theme::bg_subtle())
                        .rounded_md()
                        .border_1()
                        .border_color(theme::border_subtle())
                        .text_xs()
                        .text_color(theme::text_primary())
                        .child(handlers_path),
                )
                .child(
                    CouchButton::secondary("btn-browse-handlers", "Browse...")
                        .icon(IconName::FolderOpen)
                        .size(CouchButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.status_message =
                                "Browsing for game handlers directory...".to_string();
                            cx.notify();
                            cx.spawn(async move |this, cx| {
                                let folder_opt = crate::dialogs::pick_folder_async(
                                    "Select Game Handlers Directory",
                                )
                                .await;
                                if let Some(folder) = folder_opt {
                                    let folder_str = folder.to_string_lossy().to_string();
                                    let _ = this.update(&mut *cx, |this, cx| {
                                        this.handlers_dir = folder_str.clone();
                                        this.app_settings.handlers_directory = folder_str.clone();
                                        if let Some(ref cb) = this.callbacks.on_handlers_dir_changed
                                        {
                                            let updated = cb(&folder_str);
                                            if !updated.is_empty() {
                                                this.handlers = updated.clone();
                                                this.handler_names = updated
                                                    .iter()
                                                    .map(|h| h.game_name.clone())
                                                    .collect();
                                            }
                                        }
                                        this.status_message =
                                            format!("Handlers directory set to: {}", folder_str);
                                        cx.notify();
                                    });
                                } else {
                                    let _ = this.update(&mut *cx, |this, cx| {
                                        this.status_message =
                                            "Handlers directory selection cancelled.".to_string();
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        })),
                ),
        )
        .into_any_element()
}

/// Renders Card: Handler Script Text Editor.
pub fn render_card_appearance_script_editor(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-appearance-script-editor")
        .title_text("Handler Script Text Editor")
        .description_text(
            "External editor used to inspect and modify JavaScript (.js) game handler scripts",
        )
        .child(
            div().flex().gap_2().children(
                ["notepad.exe", "code.cmd", "notepad++.exe"]
                    .into_iter()
                    .enumerate()
                    .map(|(idx, editor)| {
                        let is_active = settings.text_editor_path.eq_ignore_ascii_case(editor);
                        let ed_str = editor.to_string();
                        div()
                            .id(("editor-opt", idx))
                            .cursor_pointer()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .border_1()
                            .text_xs()
                            .when(is_active, |this| {
                                this.bg(theme::accent())
                                    .border_color(theme::accent())
                                    .text_color(theme::text_on_accent())
                                    .font_weight(FontWeight::BOLD)
                            })
                            .when(!is_active, |this| {
                                this.bg(theme::bg_subtle())
                                    .border_color(theme::border_subtle())
                                    .text_color(theme::text_muted())
                                    .hover(|s| {
                                        s.bg(theme::bg_hover()).text_color(theme::text_primary())
                                    })
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.app_settings.text_editor_path = ed_str.clone();
                                this.status_message = format!("Script editor set to {}", ed_str);
                                cx.notify();
                            }))
                            .child(editor)
                    }),
            ),
        )
        .into_any_element()
}

/// Renders Card: Game Discovery.
pub fn render_card_appearance_autoscan(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let auto_scan_enabled = win.auto_scan;

    NefosCard::surface("card-appearance-autoscan")
        .title_text("Game Discovery")
        .description_text("Automatically search local system storage drives for installed games matching discovered handlers")
        .child(render_toggle(
            "Auto-Scan for Games on Startup",
            "Automatically search local system storage drives for installed games matching discovered handlers",
            auto_scan_enabled,
            cx.listener(|this, _, _, cx| {
                this.auto_scan = !this.auto_scan;
                this.app_settings.auto_scan_on_start = this.auto_scan;
                if let Some(ref cb) = this.callbacks.on_auto_scan_changed {
                    cb(this.auto_scan);
                }
                this.status_message = format!("Auto-scan on startup {}", if this.auto_scan { "enabled" } else { "disabled" });
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Tab 6: Appearance, Theme, OSD Color, and Paths.
fn render_appearance_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(render_card_appearance_theme(win, cx))
        .child(render_card_appearance_accent(win, cx))
        .child(render_card_appearance_osd(win, cx))
        .child(render_card_appearance_handlers_dir(win, cx))
        .child(render_card_appearance_script_editor(win, cx))
        .child(render_card_appearance_autoscan(win, cx))
        .into_any_element()
}

/// Renders Card: Network & Loopback IP Isolation.
pub fn render_card_network_isolation(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-network-isolation")
        .title_text("Network & Loopback IP Isolation")
        .description_text("Assign unique loopback IP addresses (127.0.0.X) to each player instance to avoid port collisions")
        .child(render_toggle(
            "Enable Loopback IP Isolation",
            "Automatically assign 127.0.0.X IPs starting from Base IP to avoid localhost collisions",
            settings.loopback_isolation_enabled,
            cx.listener(|this, _, _, cx| {
                this.app_settings.loopback_isolation_enabled = !this.app_settings.loopback_isolation_enabled;
                cx.notify();
            }),
        ))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .py_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child("Base Loopback IP"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Player 0 binds to 127.0.0.2, Player 1 binds to 127.0.0.3, avoiding 127.0.0.1"),
                        ),
                )
                .child(
                    NefosTag::new(settings.loopback_base_ip.clone())
                        .tone(NefosTagTone::Accent)
                )
        )
        .into_any_element()
}

/// Renders Card: Goldberg Steam Emulator Settings.
pub fn render_card_goldberg_config(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-goldberg-config")
        .title_text("Goldberg Steam Emulator Settings")
        .description_text(
            "Offline LAN multiplayer emulation flags, broadcast IP, and lobby discovery",
        )
        .child(render_toggle(
            "Force Offline Mode",
            "Prevent Goldberg emulator from making any external network requests",
            settings.goldberg_offline_mode,
            cx.listener(|this, _, _, cx| {
                this.app_settings.goldberg_offline_mode = !this.app_settings.goldberg_offline_mode;
                cx.notify();
            }),
        ))
        .child(render_stepper_row(
            "Goldberg Listen Port",
            "UDP/TCP port used for offline LAN lobby broadcasts (default 47584)",
            "goldberg-port",
            settings.goldberg_listen_port as u32,
            cx.listener(|this, _, _, cx| {
                this.app_settings.goldberg_listen_port =
                    this.app_settings.goldberg_listen_port.saturating_sub(1);
                cx.notify();
            }),
            cx.listener(|this, _, _, cx| {
                this.app_settings.goldberg_listen_port =
                    this.app_settings.goldberg_listen_port.saturating_add(1);
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Card: Instance Launch Countdown Timer.
pub fn render_card_launch_timer_config(
    win: &MainWindow,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-launch-timer-config")
        .title_text("Instance Launch Countdown Timer")
        .description_text("Configure automatic proceeding when game handlers request setup actions between instances")
        .child(render_toggle(
            "Enable Auto-Proceed Countdown",
            "When enabled, launch prompts auto-proceed after the countdown timer expires",
            settings.auto_proceed_countdown,
            cx.listener(|this, _, _, cx| {
                this.app_settings.auto_proceed_countdown = !this.app_settings.auto_proceed_countdown;
                cx.notify();
            }),
        ))
        .child(render_stepper_row(
            "Countdown Duration (Seconds)",
            "Number of seconds to wait before auto-proceeding with instance launch",
            "launch-seconds",
            settings.auto_proceed_seconds,
            cx.listener(|this, _, _, cx| {
                this.app_settings.auto_proceed_seconds = this.app_settings.auto_proceed_seconds.saturating_sub(1).max(3);
                cx.notify();
            }),
            cx.listener(|this, _, _, cx| {
                this.app_settings.auto_proceed_seconds = (this.app_settings.auto_proceed_seconds + 1).min(60);
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Card: PeerParty P2P Online Multiplayer Settings.
pub fn render_card_peerparty_config(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let settings = &win.app_settings;

    NefosCard::surface("card-peerparty-config")
        .title_text("PeerParty P2P Multiplayer Settings")
        .description_text("Decentralized multiplayer emulation via BitTorrent DHT, Opus voice chat, and in-game overlay")
        .child(render_toggle(
            "Enable PeerParty P2P Multiplayer",
            "Enable decentralized multiplayer emulation and BitTorrent DHT room discovery",
            settings.peerparty_enabled,
            cx.listener(|this, _, _, cx| {
                this.app_settings.peerparty_enabled = !this.app_settings.peerparty_enabled;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "Public DHT Session Discovery",
            "Announce and browse game sessions publicly on BitTorrent DHT",
            settings.peerparty_public_sessions,
            cx.listener(|this, _, _, cx| {
                this.app_settings.peerparty_public_sessions = !this.app_settings.peerparty_public_sessions;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "PeerParty In-Game Voice Chat",
            "Enable low-latency spatial Opus voice chat across peer instances",
            settings.peerparty_voice_chat,
            cx.listener(|this, _, _, cx| {
                this.app_settings.peerparty_voice_chat = !this.app_settings.peerparty_voice_chat;
                cx.notify();
            }),
        ))
        .child(render_toggle(
            "PeerParty In-Game Overlay",
            "Display peer connection status and player list overlay inside game viewports",
            settings.peerparty_overlay,
            cx.listener(|this, _, _, cx| {
                this.app_settings.peerparty_overlay = !this.app_settings.peerparty_overlay;
                cx.notify();
            }),
        ))
        .into_any_element()
}

/// Renders Tab 7: Network & Loopback Emulation.
fn render_network_tab(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(render_card_peerparty_config(win, cx))
        .child(render_card_network_isolation(win, cx))
        .child(render_card_goldberg_config(win, cx))
        .child(render_card_launch_timer_config(win, cx))
        .into_any_element()
}

/// Helper function to render a modern Spectrum 2 toggle switch.
fn render_toggle<F>(title: &str, desc: &str, enabled: bool, on_click: F) -> AnyElement
where
    F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
{
    let row_id = format!("toggle-row-{}", title.to_lowercase().replace(' ', "-"));
    crate::components::CouchToggle::new(row_id, title.to_string(), enabled)
        .description(desc.to_string())
        .on_click(on_click)
        .into_any_element()
}

/// Helper function to render a compact mini toggle.
fn render_mini_toggle<F>(title: &str, enabled: bool, on_click: F) -> AnyElement
where
    F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
{
    let mini_id = SharedString::from(format!(
        "mini-toggle-{}",
        title.to_lowercase().replace(' ', "-")
    ));
    div()
        .id(mini_id)
        .flex_shrink_0()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .when(enabled, |this| {
            this.bg(theme::accent_subtle())
                .border_color(theme::border_focus())
                .text_color(theme::accent())
                .font_weight(FontWeight::BOLD)
        })
        .when(!enabled, |this| {
            this.bg(theme::bg_subtle())
                .border_color(theme::border_subtle())
                .text_color(theme::text_muted())
        })
        .on_click(on_click)
        .child(div().text_xs().child(if enabled { "✓" } else { "○" }))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .into_any_element()
}

/// Helper function to render an interactive numeric stepper row.
fn render_stepper_row<F1, F2>(
    title: &str,
    desc: &str,
    id_tag: &str,
    value: u32,
    on_dec: F1,
    on_inc: F2,
) -> AnyElement
where
    F1: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    F2: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
{
    CouchOptionRow::new(
        format!("stepper-{}", id_tag),
        title.to_string(),
        desc.to_string(),
        value.to_string(),
    )
    .on_prev(on_dec)
    .on_next(on_inc)
    .into_any_element()
}

/// Helper function to parse comma-separated RGB OSD color string.
fn parse_osd_color(color_str: &str) -> Hsla {
    let parts: Vec<&str> = color_str.split(',').map(|s| s.trim()).collect();
    if parts.len() == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            parts[0].parse::<u8>(),
            parts[1].parse::<u8>(),
            parts[2].parse::<u8>(),
        ) {
            return rgb((r as u32) << 16 | (g as u32) << 8 | (b as u32)).into();
        }
    }
    theme::accent()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn test_settings_view_category_navigation() {
        let mut view = SettingsView::new();
        assert_eq!(view.active_category, SettingsCategory::General);

        view.active_category = SettingsCategory::Hotkeys;
        assert_eq!(view.active_category, SettingsCategory::Hotkeys);

        view.active_category = SettingsCategory::Players;
        assert_eq!(view.active_category, SettingsCategory::Players);

        view.active_category = SettingsCategory::Audio;
        assert_eq!(view.active_category, SettingsCategory::Audio);

        view.active_category = SettingsCategory::Layout;
        assert_eq!(view.active_category, SettingsCategory::Layout);

        view.active_category = SettingsCategory::Appearance;
        assert_eq!(view.active_category, SettingsCategory::Appearance);
    }

    #[test]
    fn test_settings_view_player_pages() {
        let mut view = SettingsView::new();
        assert_eq!(view.player_page, 0);

        view.player_page = 1;
        assert_eq!(view.player_page, 1);
    }

    #[test]
    fn test_settings_view_hotkey_recording_and_dropdown() {
        let mut view = SettingsView::new();
        assert_eq!(view.recording_hotkey, None);
        assert_eq!(view.dropdown_hotkey, None);

        view.recording_hotkey = Some("lock".to_string());
        assert_eq!(view.recording_hotkey.as_deref(), Some("lock"));

        view.dropdown_hotkey = Some("close".to_string());
        assert_eq!(view.dropdown_hotkey.as_deref(), Some("close"));

        let normalized_end = normalize_key_input("end");
        assert_eq!(normalized_end, "End");

        let normalized_f10 = normalize_key_input("f10");
        assert_eq!(normalized_f10, "F10");

        let normalized_del = normalize_key_input("del");
        assert_eq!(normalized_del, "Delete");

        let mut mods = gpui::Modifiers::default();
        mods.control = true;
        let combo = format_shortcut_combo(&mods, "Q");
        assert_eq!(combo, "Ctrl+Q");

        let empty_mods = gpui::Modifiers::default();
        let combo_f10 = format_shortcut_combo(&empty_mods, "F10");
        assert_eq!(combo_f10, "F10");

        let combo_plain_q = format_shortcut_combo(&empty_mods, "Q");
        assert_eq!(combo_plain_q, "Ctrl+Q");

        assert!(is_standalone_bindable_key("F1"));
        assert!(is_standalone_bindable_key("F12"));
        assert!(is_standalone_bindable_key("Pause"));
        assert!(!is_standalone_bindable_key("Q"));

        assert_eq!(get_default_hotkey("switch"), "Ctrl+N");
        assert_eq!(get_default_hotkey("stop"), "Ctrl+E");
        assert_eq!(get_default_hotkey("set_focus"), "Ctrl+H");
        assert_eq!(get_default_hotkey("shortcuts_reminder"), "Alt+R");
        assert_eq!(get_default_hotkey("merger_switch"), "Alt+A");

        let normalized_arrow = normalize_key_input("arrowup");
        assert_eq!(normalized_arrow, "Up");
    }

    #[test]
    fn test_settings_view_app_settings_integration() {
        let mut settings = nucleus_profile::AppSettings::default();

        // Verify default account password matches C#
        assert_eq!(settings.nucleus_account_password, "12345");

        // Verify lock key cycling
        let initial_lock = settings.lock_inputs_key.clone();
        settings.cycle_lock_key();
        assert_ne!(settings.lock_inputs_key, initial_lock);

        // Verify hotkey modifier & key cycling
        let initial_close = settings.hotkey_close.clone();
        settings.cycle_hotkey_modifier("close");
        assert_ne!(settings.hotkey_close, initial_close);
        let mod_cycled = settings.hotkey_close.clone();
        settings.cycle_hotkey_key("close");
        assert_ne!(settings.hotkey_close, mod_cycled);

        // Verify player nicknames and steam id validation
        assert_eq!(settings.get_player_nickname(0), "Player1");
        assert!(nucleus_profile::AppSettings::is_valid_steam_id(
            &settings.get_player_steam_id(0)
        ));
        assert!(!nucleus_profile::AppSettings::is_valid_steam_id("12345"));

        // Verify audio endpoint assignment
        assert_eq!(settings.get_audio_instance(0), "Default");
        settings.set_audio_instance(0, "Headphones");
        assert_eq!(settings.get_audio_instance(0), "Headphones");

        // Verify layout dividers defaults
        assert_eq!(settings.horizontal_lines, 1);
        assert_eq!(settings.vertical_lines, 1);
        assert_eq!(settings.max_players, 4);
        assert_eq!(settings.split_div_color, "Black");
        assert_eq!(settings.merger_resolution, "");
    }

    #[test]
    fn test_settings_search_filtering() {
        assert_eq!(SETTINGS_CARDS.len(), 22);

        // Empty query returns no filtered cards
        assert!(filter_settings_cards("").is_empty());
        assert!(filter_settings_cards("   ").is_empty());

        // Matching "hotkey"
        let hotkey_matches = filter_settings_cards("hotkey");
        assert!(!hotkey_matches.is_empty());
        assert!(hotkey_matches.iter().any(|c| c.id == "card-hotkeys-global"));

        // Case insensitivity
        let upper_matches = filter_settings_cards("HOTKEY");
        assert_eq!(hotkey_matches.len(), upper_matches.len());

        // Matching "goldberg"
        let goldberg_matches = filter_settings_cards("goldberg");
        assert!(
            goldberg_matches
                .iter()
                .any(|c| c.id == "card-goldberg-config")
        );

        // Matching "isolation"
        let iso_matches = filter_settings_cards("isolation");
        assert!(
            iso_matches
                .iter()
                .any(|c| c.id == "card-general-isolation" || c.id == "card-network-isolation")
        );

        // Nonexistent query returns 0 matches
        let none_matches = filter_settings_cards("nonexistentxyz999");
        assert!(none_matches.is_empty());

        // PeerParty card match
        let pp_matches = filter_settings_cards("peerparty");
        assert!(pp_matches.iter().any(|c| c.id == "card-peerparty-config"));
        let dht_matches = filter_settings_cards("DHT");
        assert!(dht_matches.iter().any(|c| c.id == "card-peerparty-config"));
    }

    #[test]
    fn test_peerparty_settings_card_registration() {
        let card = SETTINGS_CARDS
            .iter()
            .find(|c| c.id == "card-peerparty-config");
        assert!(
            card.is_some(),
            "card-peerparty-config must be registered in SETTINGS_CARDS"
        );

        let card = card.unwrap();
        assert_eq!(card.category, SettingsCategory::Network);
        assert!(card.title.contains("PeerParty"));
    }
}
