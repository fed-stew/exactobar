//! Toggle switch component.

use gpui::*;

/// Toggle switch for boolean settings.
pub struct Toggle {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_toggle: Option<Box<dyn Fn(bool, &mut App) + 'static>>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            disabled: false,
            on_toggle: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set a callback to be invoked when the toggle is clicked.
    /// The callback receives the NEW toggled state (after the click).
    pub fn on_toggle(mut self, cb: impl Fn(bool, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(cb));
        self
    }
}



impl IntoElement for Toggle {
    type Element = Stateful<Div>;

    fn into_element(mut self) -> Self::Element {
        let track_color = if self.checked {
            hsla(217.0 / 360.0, 0.91, 0.60, 1.0)  // Blue when checked
        } else {
            hsla(0.0, 0.0, 0.8, 1.0)  // Gray when unchecked
        };

        let knob_offset = if self.checked { px(14.0) } else { px(2.0) };

        // Take ownership of the callback for use in the click handler
        let on_toggle = self.on_toggle.take();
        let new_checked = !self.checked;
        let disabled = self.disabled;

        let mut element = div()
            .id(self.id)
            .w(px(32.0))
            .h(px(18.0))
            .rounded(px(9.0))
            .bg(track_color)
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.7, 1.0))
            .relative()
            .child(
                div()
                    .absolute()
                    .top(px(1.0))
                    .left(knob_offset)
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded_full()
                    .bg(white())
                    .shadow_sm(),
            );

        // Only add click handling if not disabled
        if !disabled {
            element = element
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    if let Some(ref cb) = on_toggle {
                        cb(new_checked, cx);
                    }
                });
        } else {
            element = element.cursor_not_allowed().opacity(0.5);
        }

        element
    }
}
