//! Handler step-by-step runtime debugger and integrated script editor view.

use crate::components::badge::{CouchTag, TagTone};
use crate::components::button::{CouchButton, CouchButtonSize};
use crate::theme;
use crate::window::MainWindow;
use gpui::prelude::*;
use gpui::*;
use gpui_component::Icon;
use gpui_kit_assets::IconName;
use nucleus_debug::protocol::{Breakpoint, DebugLaunchMode, PipelineStage};

/// Presentation state for the debugger panel and integrated handler script editor.
#[derive(Debug, Clone)]
pub struct DebuggerViewState {
    pub is_attached: bool,
    pub is_paused: bool,
    pub current_file: String,
    pub current_line: u32,
    pub current_col: u32,
    pub active_stage: PipelineStage,
    pub launch_mode: DebugLaunchMode,
    pub breakpoints: Vec<Breakpoint>,
    pub call_stack: Vec<String>,
    pub inspector_variables: Vec<(String, String, String)>, // (Name, Type, Value)
    pub eval_input: String,
    pub eval_output: Option<String>,
    pub script_source_lines: Vec<String>,
    pub is_edit_mode: bool,
    pub editing_line: Option<usize>,
    pub editing_line_buffer: String,
    pub is_dirty: bool,
    pub selected_handler_guid: Option<String>,
    pub selected_game_title: Option<String>,
    pub status_message: Option<String>,
    pub dropdown_handlers_open: bool,
    pub available_handlers: Vec<(String, String)>,
}

impl Default for DebuggerViewState {
    fn default() -> Self {
        Self {
            is_attached: false,
            is_paused: false,
            current_file: "handler.js".to_string(),
            current_line: 1,
            current_col: 1,
            active_stage: PipelineStage::ScriptEvaluate,
            launch_mode: DebugLaunchMode::Normal,
            breakpoints: Vec::new(),
            call_stack: vec!["<top-level>".to_string()],
            inspector_variables: Vec::new(),
            eval_input: String::new(),
            eval_output: None,
            script_source_lines: Vec::new(),
            is_edit_mode: false,
            editing_line: None,
            editing_line_buffer: String::new(),
            is_dirty: false,
            selected_handler_guid: None,
            selected_game_title: None,
            status_message: None,
            dropdown_handlers_open: false,
            available_handlers: Vec::new(),
        }
    }
}

impl DebuggerViewState {
    /// Loads a new script into the editor buffer and clears dirty state.
    pub fn load_script(&mut self, file_name: &str, content: &str) {
        self.current_file = file_name.to_string();
        self.script_source_lines = content.lines().map(|s| s.to_string()).collect();
        if self.script_source_lines.is_empty() {
            self.script_source_lines.push(String::new());
        }
        self.is_dirty = false;
        self.editing_line = None;
    }

    /// Returns the combined multiline script source.
    pub fn full_script_content(&self) -> String {
        self.script_source_lines.join("\n")
    }

    /// Inserts a new empty line directly after the specified line index.
    pub fn insert_line_after(&mut self, idx: usize) {
        let insert_at = (idx + 1).min(self.script_source_lines.len());
        self.script_source_lines.insert(insert_at, String::new());
        self.editing_line = Some(insert_at);
        self.editing_line_buffer.clear();
        self.is_dirty = true;
    }

    /// Deletes the line at the specified index.
    pub fn delete_line(&mut self, idx: usize) {
        if idx < self.script_source_lines.len() {
            self.script_source_lines.remove(idx);
            if self.script_source_lines.is_empty() {
                self.script_source_lines.push(String::new());
            }
            self.is_dirty = true;
            if self.editing_line == Some(idx) {
                self.editing_line = None;
            }
        }
    }
}

/// Lightweight pure-Rust JavaScript tokenizer that yields (token, color) pairs
/// matching VS Code / Monaco editor syntax semantics.
pub fn highlight_js_tokens(code: &str) -> Vec<(String, Hsla)> {
    let mut tokens = Vec::new();
    let trimmed = code.trim_start();
    if trimmed.starts_with("//") {
        tokens.push((code.to_string(), theme::text_muted()));
        return tokens;
    }

    let keywords = [
        "function", "var", "let", "const", "return", "if", "else", "for", "while", "break",
        "continue", "switch", "case", "default", "new", "this", "typeof", "delete", "throw", "try",
        "catch", "finally", "class", "extends", "import", "export",
    ];
    let globals = [
        "Game", "Nucleus", "Hub", "Context", "Player", "Audio", "Display", "console", "Math",
        "JSON",
    ];
    let literals = ["true", "false", "null", "undefined"];

    let chars: Vec<char> = code.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        if ch == '/' && i + 1 < len && chars[i + 1] == '/' {
            let comment: String = chars[i..].iter().collect();
            tokens.push((comment, theme::text_muted()));
            break;
        } else if ch == '"' || ch == '\'' || ch == '`' {
            let quote = ch;
            let mut s = String::new();
            s.push(quote);
            i += 1;
            while i < len {
                let c = chars[i];
                s.push(c);
                if c == quote && chars[i - 1] != '\\' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push((s, rgb(0x4ade80).into())); // Green strings
        } else if ch.is_alphabetic() || ch == '_' || ch == '$' {
            let mut ident = String::new();
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                ident.push(chars[i]);
                i += 1;
            }
            let color = if globals.contains(&ident.as_str()) {
                rgb(0xfbbf24).into() // Gold/yellow for Game & Nucleus globals
            } else if keywords.contains(&ident.as_str()) {
                rgb(0x38bdf8).into() // Cyan/blue for JS keywords
            } else if literals.contains(&ident.as_str()) {
                rgb(0xc084fc).into() // Purple for booleans & null
            } else {
                theme::text_primary()
            };
            tokens.push((ident, color));
        } else if ch.is_ascii_digit() {
            let mut num = String::new();
            while i < len
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'x'
                    || chars[i] == 'X')
            {
                num.push(chars[i]);
                i += 1;
            }
            tokens.push((num, rgb(0xfb923c).into())); // Orange for numbers
        } else {
            let mut punct = String::new();
            while i < len
                && !chars[i].is_alphanumeric()
                && chars[i] != '_'
                && chars[i] != '$'
                && chars[i] != '"'
                && chars[i] != '\''
                && chars[i] != '`'
                && !(chars[i] == '/' && i + 1 < len && chars[i + 1] == '/')
            {
                punct.push(chars[i]);
                i += 1;
            }
            tokens.push((punct, theme::text_subtle()));
        }
    }

    if tokens.is_empty() {
        tokens.push((code.to_string(), theme::text_primary()));
    }
    tokens
}

/// Renders the complete Handler Runtime Debugger & Integrated Script Editor panel.
pub fn render_debugger_view(win: &MainWindow, cx: &mut Context<MainWindow>) -> AnyElement {
    let state = &win.debugger_view_state;
    let eval_query = state.eval_input.clone();
    let current_file_clone = state.current_file.clone();
    let selected_guid = state.selected_handler_guid.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_6()
        .gap_4()
        .bg(theme::bg_main())
        .child(
            // Top Debugger & Editor Unified Toolbar
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .p_3()
                .rounded_lg()
                .bg(theme::bg_panel())
                .border_1()
                .border_color(theme::border_color())
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_primary())
                                .child("Handler Editor & Debugger"),
                        )
                        // Handler File Selector Dropdown
                        .child(
                            div()
                                .relative()
                                .child(
                                    div()
                                        .id("btn-dbg-handler-select")
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(theme::bg_subtle())
                                        .border_1()
                                        .border_color(theme::border_color())
                                        .hover(|s| s.bg(theme::bg_active()).border_color(theme::accent()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.debugger_view_state.dropdown_handlers_open =
                                                !this.debugger_view_state.dropdown_handlers_open;
                                            cx.notify();
                                        }))
                                        .child(
                                            Icon::new(IconName::FileCode)
                                                .size_4()
                                                .text_color(theme::accent()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::text_primary())
                                                .child(state.selected_game_title.clone().unwrap_or_else(|| state.current_file.clone())),
                                        )
                                        .child(
                                            Icon::new(if state.dropdown_handlers_open {
                                                IconName::ChevronUp
                                            } else {
                                                IconName::ChevronDown
                                            })
                                            .size_3()
                                            .text_color(theme::text_muted()),
                                        ),
                                )
                                .when(state.dropdown_handlers_open, |parent| {
                                    parent.child(
                                        div()
                                            .id("dbg-handler-dropdown-popover")
                                            .occlude()
                                            .absolute()
                                            .top(px(36.0))
                                            .left_0()
                                            .min_w(px(240.0))
                                            .max_h(px(260.0))
                                            .overflow_y_scroll()
                                            .p_1p5()
                                            .rounded_lg()
                                            .bg(theme::bg_panel())
                                            .border_1()
                                            .border_color(theme::accent())
                                            .shadow_xl()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .children(win.handlers.iter().map(|h| {
                                                let g_id = h.guid.clone();
                                                let g_title = h.game_name.clone();
                                                let is_sel = selected_guid.as_deref() == Some(&g_id);
                                                div()
                                                    .id(ElementId::Name(format!("opt-handler-{}", g_id).into()))
                                                    .cursor_pointer()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px_2p5()
                                                    .py_1p5()
                                                    .rounded_md()
                                                    .text_xs()
                                                    .when(is_sel, |s| {
                                                        s.bg(theme::accent().opacity(0.15))
                                                            .text_color(theme::accent())
                                                            .font_weight(FontWeight::BOLD)
                                                    })
                                                    .when(!is_sel, |s| {
                                                        s.text_color(theme::text_primary())
                                                            .hover(|h| h.bg(theme::bg_hover()))
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.debugger_view_state.dropdown_handlers_open = false;
                                                        this.load_debugger_handler_script(&g_id, cx);
                                                    }))
                                                    .child(g_title)
                                                    .child(if is_sel { "✓" } else { "" })
                                            })),
                                    )
                                }),
                        )
                        // Dirty / Save Status Tag
                        .child(
                            CouchTag::new(if state.is_dirty { "Unsaved Changes ●" } else { "Saved" })
                                .tone(if state.is_dirty { TagTone::Warning } else { TagTone::Success }),
                        )
                        .child(
                            CouchTag::new(if state.is_paused { "Paused" } else { "Running" })
                                .tone(if state.is_paused { TagTone::Warning } else { TagTone::Success })
                                .icon(if state.is_paused { IconName::Pause } else { IconName::Play }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        // 1. Save Script Button
                        .child(
                            CouchButton::primary("btn-save-handler-script", "Save Script (Ctrl+S)")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::Save)
                                .disabled(!state.is_dirty)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.save_debugger_handler_script(cx);
                                })),
                        )
                        // 2. Revert Script Button
                        .child(
                            CouchButton::secondary("btn-revert-handler-script", "Revert")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::RefreshCw)
                                .on_click(cx.listener({
                                    let g_reload = selected_guid.clone().unwrap_or_else(|| current_file_clone.clone());
                                    move |this, _, _, cx| {
                                        this.load_debugger_handler_script(&g_reload, cx);
                                    }
                                })),
                        )
                        // 3. Edit / View Mode Toggle
                        .child(
                            CouchButton::secondary(
                                "btn-toggle-edit-mode",
                                if state.is_edit_mode { "Inspect Mode" } else { "Edit Mode" },
                            )
                            .size(CouchButtonSize::Sm)
                            .icon(if state.is_edit_mode { IconName::Eye } else { IconName::Pencil })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.debugger_view_state.is_edit_mode = !this.debugger_view_state.is_edit_mode;
                                cx.notify();
                            })),
                        )
                        // 4. Debugger Controls
                        .child(
                            CouchButton::primary("btn-dbg-resume", "Continue")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::Play)
                                .disabled(!state.is_paused)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(ref cb) = this.callbacks.on_debug_resume {
                                        cb();
                                    }
                                    this.debugger_view_state.is_paused = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            CouchButton::secondary("btn-dbg-step-over", "Step Over")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::ChevronRight)
                                .disabled(!state.is_paused)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(ref cb) = this.callbacks.on_debug_step_over {
                                        cb();
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            CouchButton::secondary("btn-dbg-step-into", "Step Into")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::ChevronDown)
                                .disabled(!state.is_paused)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(ref cb) = this.callbacks.on_debug_step_into {
                                        cb();
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            CouchButton::secondary("btn-dbg-step-out", "Step Out")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::ChevronUp)
                                .disabled(!state.is_paused)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(ref cb) = this.callbacks.on_debug_step_out {
                                        cb();
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            CouchButton::secondary("btn-dbg-pause", "Pause")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::Pause)
                                .disabled(state.is_paused)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(ref cb) = this.callbacks.on_debug_pause {
                                        cb();
                                    }
                                    this.debugger_view_state.is_paused = true;
                                    cx.notify();
                                })),
                        )
                        .child(
                            CouchButton::danger("btn-dbg-stop", "Stop")
                                .size(CouchButtonSize::Sm)
                                .icon(IconName::X)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(ref cb) = this.callbacks.on_debug_stop {
                                        cb();
                                    }
                                    this.debugger_view_state.is_paused = false;
                                    this.debugger_view_state.is_attached = false;
                                    cx.notify();
                                })),
                        ),
                ),
        )
        // Pipeline Stage Stepper Breadcrumb Track
        .child(
            div()
                .id("dbg-pipeline-stages-scroll")
                .flex()
                .items_center()
                .gap_2()
                .p_2p5()
                .rounded_md()
                .bg(theme::bg_panel())
                .border_1()
                .border_color(theme::border_color())
                .overflow_x_scroll()
                .children([
                    PipelineStage::ScriptEvaluate,
                    PipelineStage::OptionsUi,
                    PipelineStage::PrepareInstances,
                    PipelineStage::MutexKillSweep,
                    PipelineStage::ProcessSpawn,
                    PipelineStage::HwndPoll,
                    PipelineStage::ApplyBounds,
                    PipelineStage::HeartbeatTick,
                    PipelineStage::Teardown,
                ].iter().map(|stage| {
                    let is_current = *stage == state.active_stage;
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            CouchTag::new(stage.display_label())
                                .tone(if is_current { TagTone::Accent } else { TagTone::Neutral })
                                .icon(if is_current { IconName::CircleCheck } else { IconName::Clock }),
                        )
                        .into_any_element()
                })),
        )
        .child(
            // Main Debugger Workspace Split: Left Panels + Center Editor + Right Inspector
            div()
                .flex()
                .size_full()
                .gap_4()
                .child(
                    // Left Column: Breakpoints & Call Stack
                    div()
                        .flex()
                        .flex_col()
                        .w(px(240.0))
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_color())
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Call Stack"),
                                )
                                .children(state.call_stack.iter().map(|frame| {
                                    div()
                                        .text_xs()
                                        .text_color(theme::text_muted())
                                        .child(format!("• {}", frame))
                                        .into_any_element()
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_color())
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
                                                .child("Breakpoints"),
                                        )
                                        .when(!state.breakpoints.is_empty(), |el| {
                                            el.child(
                                                div()
                                                    .id("btn-dbg-clear-all-bps")
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(theme::danger())
                                                    .hover(|s| s.underline())
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.debugger_view_state.breakpoints.clear();
                                                        cx.notify();
                                                    }))
                                                    .child("Clear"),
                                            )
                                        }),
                                )
                                .children(if state.breakpoints.is_empty() {
                                    vec![div()
                                        .text_xs()
                                        .text_color(theme::text_muted())
                                        .child("Click line numbers in editor to toggle breakpoints.")
                                        .into_any_element()]
                                } else {
                                    state.breakpoints.iter().map(|bp| {
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .text_xs()
                                            .text_color(theme::text_primary())
                                            .child(format!("{}:{}", bp.file_id, bp.line))
                                            .child(CouchTag::new("Active").tone(TagTone::Success))
                                            .into_any_element()
                                    }).collect()
                                }),
                        ),
                )
                .child(
                    // Center Column: Monaco-Style Integrated Handler Script Editor
                    div()
                        .id("dbg-source-code-editor")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .p_3()
                        .rounded_md()
                        .bg(theme::bg_panel())
                        .border_1()
                        .border_color(theme::border_color())
                        .overflow_y_scroll()
                        // Editor Subheader info bar
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb_2()
                                .border_b_1()
                                .border_color(theme::border_subtle())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::accent())
                                                .child(state.current_file.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme::text_muted())
                                                .child(format!("({} lines)", state.script_source_lines.len())),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .when(state.is_edit_mode, |el| {
                                            el.child(
                                                CouchButton::secondary("btn-add-line-bottom", "+ Append Line")
                                                    .size(CouchButtonSize::Sm)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        let last_idx = this.debugger_view_state.script_source_lines.len();
                                                        this.debugger_view_state.insert_line_after(last_idx.saturating_sub(1));
                                                        cx.notify();
                                                    })),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme::text_muted())
                                                .child(if state.is_edit_mode {
                                                    "Click any line to edit inline"
                                                } else {
                                                    "Click line number to set breakpoint"
                                                }),
                                        ),
                                ),
                        )
                        // Source code line gutter + tokens
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .pt_2()
                                .children(if state.script_source_lines.is_empty() {
                                    vec![div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .py_8()
                                        .text_xs()
                                        .text_color(theme::text_muted())
                                        .child("No handler script source loaded. Select a game above.")
                                        .into_any_element()]
                                } else {
                                    state.script_source_lines.iter().enumerate().map(|(idx, line)| {
                                        let line_no = (idx + 1) as u32;
                                        let is_current_line = line_no == state.current_line;
                                        let has_bp = state.breakpoints.iter().any(|b| b.line == line_no);
                                        let is_editing_this_line = state.editing_line == Some(idx);
                                        let file_id = state.current_file.clone();

                                        div()
                                            .id(ElementId::Name(format!("code-line-{}", line_no).into()))
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(if is_current_line {
                                                theme::accent_subtle()
                                            } else if is_editing_this_line {
                                                theme::bg_active()
                                            } else {
                                                theme::bg_panel()
                                            })
                                            // 1. Breakpoint / Execution pointer gutter
                                            .child(
                                                div()
                                                    .id(ElementId::Name(format!("gutter-bp-{}", line_no).into()))
                                                    .cursor_pointer()
                                                    .w(px(16.0))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .on_click(cx.listener({
                                                        let f_id = file_id.clone();
                                                        move |this, _, _, cx| {
                                                            if let Some(pos) = this.debugger_view_state.breakpoints.iter().position(|b| b.line == line_no) {
                                                                this.debugger_view_state.breakpoints.remove(pos);
                                                            } else {
                                                                this.debugger_view_state.breakpoints.push(Breakpoint {
                                                                    file_id: f_id.clone(),
                                                                    line: line_no,
                                                                    column: None,
                                                                    condition: None,
                                                                    hit_count: 0,
                                                                    enabled: true,
                                                                });
                                                            }
                                                            if let Some(ref cb) = this.callbacks.on_debug_toggle_breakpoint {
                                                                cb(&f_id, line_no);
                                                            }
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .child(if has_bp {
                                                        div()
                                                            .size(px(8.0))
                                                            .rounded_full()
                                                            .bg(theme::danger())
                                                            .into_any_element()
                                                    } else if is_current_line {
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme::accent())
                                                            .child("▶")
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .size(px(8.0))
                                                            .rounded_full()
                                                            .hover(|s| s.bg(theme::border_color()))
                                                            .into_any_element()
                                                    }),
                                            )
                                            // 2. Line Number
                                            .child(
                                                div()
                                                    .w(px(36.0))
                                                    .text_right()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::text_muted())
                                                    .child(line_no.to_string()),
                                            )
                                            // 3. Line Content: Interactive Editor vs Syntax Highlighting
                                            .child(if is_editing_this_line {
                                                let line_buf = state.editing_line_buffer.clone();
                                                div()
                                                    .flex_1()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .id(ElementId::Name(format!("input-line-{}", idx).into()))
                                                            .key_context("debugger_line_editor")
                                                            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                                                                if event.keystroke.modifiers.control || event.keystroke.modifiers.alt || event.keystroke.modifiers.platform {
                                                                    return;
                                                                }
                                                                let key = event.keystroke.key.as_str();
                                                                match key {
                                                                    "backspace" => {
                                                                        this.debugger_view_state.editing_line_buffer.pop();
                                                                        cx.notify();
                                                                    }
                                                                    "enter" => {
                                                                        let new_text = this.debugger_view_state.editing_line_buffer.clone();
                                                                        if idx < this.debugger_view_state.script_source_lines.len() {
                                                                            this.debugger_view_state.script_source_lines[idx] = new_text;
                                                                        }
                                                                        this.debugger_view_state.insert_line_after(idx);
                                                                        this.debugger_view_state.is_dirty = true;
                                                                        cx.notify();
                                                                    }
                                                                    "escape" => {
                                                                        this.debugger_view_state.editing_line = None;
                                                                        cx.notify();
                                                                    }
                                                                    _ => {
                                                                        if let Some(ref s) = event.keystroke.key_char {
                                                                            this.debugger_view_state.editing_line_buffer.push_str(s);
                                                                            cx.notify();
                                                                        } else if key.chars().count() == 1 {
                                                                            this.debugger_view_state.editing_line_buffer.push_str(key);
                                                                            cx.notify();
                                                                        }
                                                                    }
                                                                }
                                                            }))
                                                            .flex_1()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(theme::bg_main())
                                                            .border_1()
                                                            .border_color(theme::accent())
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme::text_primary())
                                                            .child(format!("{}|", line_buf)),
                                                    )
                                                    .child(
                                                        div()
                                                            .id(ElementId::Name(format!("btn-dbg-done-line-{}", line_no).into()))
                                                            .cursor_pointer()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(theme::accent())
                                                            .text_xs()
                                                            .text_color(theme::text_on_accent())
                                                            .font_weight(FontWeight::BOLD)
                                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                                let new_text = this.debugger_view_state.editing_line_buffer.clone();
                                                                if idx < this.debugger_view_state.script_source_lines.len() {
                                                                    this.debugger_view_state.script_source_lines[idx] = new_text;
                                                                    this.debugger_view_state.is_dirty = true;
                                                                }
                                                                this.debugger_view_state.editing_line = None;
                                                                cx.notify();
                                                            }))
                                                            .child("Done"),
                                                    )
                                                    .child(
                                                        div()
                                                            .id(ElementId::Name(format!("btn-dbg-del-line-{}", line_no).into()))
                                                            .cursor_pointer()
                                                            .p_0p5()
                                                            .text_xs()
                                                            .text_color(theme::danger())
                                                            .hover(|s| s.text_color(theme::danger()))
                                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                                this.debugger_view_state.delete_line(idx);
                                                                cx.notify();
                                                            }))
                                                            .child(Icon::new(IconName::Trash).size_3()),
                                                    )
                                                    .into_any_element()
                                            } else {
                                                let tokens = highlight_js_tokens(line);
                                                let line_copy = line.clone();
                                                div()
                                                    .id(ElementId::Name(format!("code-line-{}", line_no).into()))
                                                    .cursor_text()
                                                    .flex_1()
                                                    .flex()
                                                    .flex_wrap()
                                                    .items_center()
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if this.debugger_view_state.is_edit_mode {
                                                            this.debugger_view_state.editing_line = Some(idx);
                                                            this.debugger_view_state.editing_line_buffer = line_copy.clone();
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .children(tokens.into_iter().map(|(tok, col)| {
                                                        div()
                                                            .text_xs()
                                                            .text_color(col)
                                                            .child(tok)
                                                    }))
                                                    .into_any_element()
                                            })
                                            .into_any_element()
                                    }).collect()
                                }),
                        ),
                )
                .child(
                    // Right Column: Variable Inspector & Console / Eval REPL
                    div()
                        .flex()
                        .flex_col()
                        .w(px(300.0))
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_color())
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Variable Inspector"),
                                )
                                .children(if state.inspector_variables.is_empty() {
                                    vec![div()
                                        .text_xs()
                                        .text_color(theme::text_muted())
                                        .child("Pause execution to inspect live JS objects and context.")
                                        .into_any_element()]
                                } else {
                                    state.inspector_variables.iter().map(|(name, var_type, val)| {
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .text_xs()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::accent())
                                                    .child(name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme::text_muted())
                                                    .child(format!("({}) {}", var_type, val)),
                                            )
                                            .into_any_element()
                                    }).collect()
                                }),
                        )
                        .child(
                            // Debug REPL / Expression Evaluator
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .bg(theme::bg_panel())
                                .border_1()
                                .border_color(theme::border_color())
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text_primary())
                                        .child("Console / Eval REPL"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex_1()
                                                .h(px(28.0))
                                                .px_2()
                                                .rounded_md()
                                                .bg(theme::bg_main())
                                                .border_1()
                                                .border_color(theme::border_color())
                                                .text_xs()
                                                .text_color(theme::text_primary())
                                                .child(if eval_query.is_empty() {
                                                    "Eval JS expr (e.g. Game.ExecutableName)...".to_string()
                                                } else {
                                                    eval_query.clone()
                                                }),
                                        )
                                        .child(
                                            CouchButton::primary("btn-dbg-eval", "Eval")
                                                .size(CouchButtonSize::Sm)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let query = this.debugger_view_state.eval_input.clone();
                                                    if let Some(ref cb) = this.callbacks.on_debug_eval {
                                                        match cb(&query) {
                                                            Ok(res) => this.debugger_view_state.eval_output = Some(res),
                                                            Err(err) => this.debugger_view_state.eval_output = Some(format!("Error: {}", err)),
                                                        }
                                                    }
                                                    cx.notify();
                                                })),
                                        ),
                                )
                                .when_some(state.eval_output.as_ref(), |this, out| {
                                    this.child(
                                        div()
                                            .p_2()
                                            .rounded_sm()
                                            .bg(theme::bg_main())
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child(out.clone()),
                                    )
                                }),
                        ),
                ),
        )
        .into_any_element()
}
