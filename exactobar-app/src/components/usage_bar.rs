//! Usage progress bar component.

use gpui::*;

/// Progress bar showing usage percentage.
pub struct UsageBar {
    /// Percentage remaining (0-100).
    percent: f32,
    /// Height of the bar.
    height: Pixels,
}

impl UsageBar {
    /// Creates a new usage bar.
    pub fn new(percent_remaining: f32) -> Self {
        Self {
            percent: percent_remaining.clamp(0.0, 100.0),
            height: px(8.0),
        }
    }

    /// Sets the height of the bar.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// Sets this element to flex grow.
    pub fn flex_1(self) -> UsageBarWithFlex {
        UsageBarWithFlex { bar: self }
    }

    fn fill_color(&self) -> Hsla {
        if self.percent > 50.0 {
            hsla(142.0 / 360.0, 0.71, 0.45, 1.0)  // Green
        } else if self.percent > 20.0 {
            hsla(38.0 / 360.0, 0.92, 0.50, 1.0)   // Yellow
        } else {
            hsla(0.0, 0.84, 0.60, 1.0)            // Red
        }
    }
}

impl IntoElement for UsageBar {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let fill_width_percent = self.percent;
        let fill_color = self.fill_color();

        div()
            .h(self.height)
            .w_full()
            .bg(hsla(0.0, 0.0, 0.9, 1.0))
            .rounded(self.height / 2.0)
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(fill_color)
                    .rounded(self.height / 2.0)
                    .w(relative(fill_width_percent / 100.0)),
            )
    }
}

/// Usage bar with flex-1 styling.
pub struct UsageBarWithFlex {
    bar: UsageBar,
}

impl IntoElement for UsageBarWithFlex {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let fill_width_percent = self.bar.percent;
        let fill_color = self.bar.fill_color();

        div()
            .flex_1()
            .h(self.bar.height)
            .bg(hsla(0.0, 0.0, 0.9, 1.0))
            .rounded(self.bar.height / 2.0)
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(fill_color)
                    .rounded(self.bar.height / 2.0)
                    .w(relative(fill_width_percent / 100.0)),
            )
    }
}
