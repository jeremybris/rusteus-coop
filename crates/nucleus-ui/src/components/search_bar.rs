use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::Icon;
use gpui_kit_assets::IconName;

use crate::theme;

/// A standardized search bar component with icon, placeholder, focus tracking,
/// keystroke routing, and clear button.
pub struct CouchSearchBar {
    id: ElementId,
    placeholder: SharedString,
    query: SharedString,
    focus_handle: Option<FocusHandle>,
    is_focused: bool,
    on_clear: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    on_key_down: Option<Box<dyn Fn(&KeyDownEvent, &mut Window, &mut App) + 'static>>,
}

impl CouchSearchBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            placeholder: "Search...".into(),
            query: String::new().into(),
            focus_handle: None,
            is_focused: false,
            on_clear: None,
            on_key_down: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }

    pub fn focus_handle(mut self, handle: &FocusHandle) -> Self {
        self.focus_handle = Some(handle.clone());
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn on_clear<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_clear = Some(Box::new(handler));
        self
    }

    pub fn on_key_down<F>(mut self, handler: F) -> Self
    where
        F: Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    {
        self.on_key_down = Some(Box::new(handler));
        self
    }

    pub fn into_any_element(self) -> AnyElement {
        let is_empty = self.query.trim().is_empty();

        let mut input_box = div()
            .id(self.id.clone())
            .cursor_text()
            .flex_1()
            .min_w_0()
            .h(px(38.0))
            .px_3()
            .bg(theme::bg_panel())
            .rounded_lg()
            .border_1()
            .border_color(if self.is_focused {
                theme::accent()
            } else {
                theme::border_color()
            })
            .hover(|s| s.border_color(theme::accent()))
            .flex()
            .items_center()
            .justify_between()
            .gap_3();

        if let Some(ref focus) = self.focus_handle {
            input_box = input_box.track_focus(focus);
            let focus_clone = focus.clone();
            input_box = input_box.on_click(move |_event, window, cx| {
                focus_clone.focus(window, cx);
                cx.stop_propagation();
            });
            let focus_clone_down = focus.clone();
            input_box = input_box.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                focus_clone_down.focus(window, cx);
                cx.stop_propagation();
            });
        }

        if let Some(h) = self.on_key_down {
            input_box = input_box.on_key_down(move |event, window, cx| {
                h(event, window, cx);
            });
        }

        // Left side: Magnifying glass icon + text / placeholder
        let left_part = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2p5()
            .child(
                Icon::new(IconName::Search)
                    .size_4()
                    .text_color(if self.is_focused {
                        theme::accent()
                    } else {
                        theme::text_muted()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .truncate()
                    .when(is_empty, |this| {
                        if self.is_focused {
                            this.text_color(theme::text_primary()).child("█")
                        } else {
                            this.text_color(theme::text_muted()).child(self.placeholder)
                        }
                    })
                    .when(!is_empty, |this| {
                        if self.is_focused {
                            this.text_color(theme::text_primary())
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{} █", self.query))
                        } else {
                            this.text_color(theme::text_primary())
                                .font_weight(FontWeight::MEDIUM)
                                .child(self.query)
                        }
                    }),
            );

        input_box = input_box.child(left_part);

        // Right side: Clear button (when query is present)
        if !is_empty {
            let mut clear_btn = div()
                .id((self.id.clone(), "search-clear-btn"))
                .cursor_pointer()
                .flex_shrink_0()
                .p_1()
                .rounded_md()
                .hover(|s| s.bg(theme::bg_hover()))
                .child(
                    Icon::new(IconName::X)
                        .size_3p5()
                        .text_color(theme::text_muted()),
                );

            if let Some(h) = self.on_clear {
                clear_btn = clear_btn.on_click(move |ev, win, cx| h(ev, win, cx));
            }
            input_box = input_box.child(clear_btn);
        }

        input_box.into_any_element()
    }
}

impl IntoElement for CouchSearchBar {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.into_any_element()
    }
}
