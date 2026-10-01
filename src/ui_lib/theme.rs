use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
};

mod catppuchin_mocha {
    #![allow(dead_code)]
    use super::*;

    // Base
    pub const ROSEWATER: Color = Color::Rgb(245, 224, 220);
    pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);
    pub const PINK: Color = Color::Rgb(245, 194, 231);
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);
    pub const RED: Color = Color::Rgb(243, 139, 168);
    pub const MAROON: Color = Color::Rgb(235, 160, 172);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);
    pub const GREEN: Color = Color::Rgb(166, 227, 161);
    pub const TEAL: Color = Color::Rgb(148, 226, 213);
    pub const SKY: Color = Color::Rgb(137, 220, 235);
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
    pub const BLUE: Color = Color::Rgb(137, 180, 250);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);

    // Text
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    pub const SUBTEXT_1: Color = Color::Rgb(186, 194, 222);
    pub const SUBTEXT_0: Color = Color::Rgb(166, 173, 200);

    // Surfaces
    pub const OVERLAY_2: Color = Color::Rgb(147, 153, 178);
    pub const OVERLAY_1: Color = Color::Rgb(127, 132, 156);
    pub const OVERLAY_0: Color = Color::Rgb(108, 112, 134);

    pub const SURFACE_2: Color = Color::Rgb(88, 91, 112);
    pub const SURFACE_1: Color = Color::Rgb(69, 71, 90);
    pub const SURFACE_0: Color = Color::Rgb(49, 50, 68);

    // Backgrounds
    pub const BASE: Color = Color::Rgb(30, 30, 46);
    pub const MANTLE: Color = Color::Rgb(24, 24, 37);
    pub const CRUST: Color = Color::Rgb(17, 17, 27);

    pub const BG: Color = BASE;
    pub const PANEL: Color = SURFACE_0;
    pub const PANEL_BORDER: Color = SURFACE_2;

    pub const FG: Color = TEXT;
    pub const FG_MUTED: Color = SUBTEXT_0;

    pub const ACCENT: Color = MAUVE;
    pub const SUCCESS: Color = GREEN;
    pub const WARNING: Color = YELLOW;
    pub const ERROR: Color = RED;
    pub const INFO: Color = BLUE;
}

/// Defines the visual language used throughout the application.
///
/// Widgets should reference semantic styles from this theme instead of
/// constructing colors and modifiers locally. This ensures that equivalent
/// information is rendered consistently across all widgets.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Default style for ordinary application text.
    pub text: Style,

    /// Style for secondary information that should receive less visual
    /// attention than normal text.
    ///
    /// Suitable for placeholders, initialization messages, metadata, and
    /// unavailable values.
    pub muted: Style,

    /// Style for information that should receive additional attention without
    /// indicating a particular status.
    ///
    /// Suitable for selected values, important headings, and active controls.
    pub accent: Style,

    /// Style for labels that describe a value.
    ///
    /// Suitable for prefixes such as "State:", "Error:", or "Port:".
    pub label: Style,

    /// Style for ordinary values shown next to a label.
    ///
    /// Suitable for identifiers, measurements, configuration values, and
    /// status details that have no exceptional meaning.
    pub value: Style,

    /// Style for the border of an unfocused container.
    pub border: Style,

    /// Style for the border of the currently focused container.
    pub border_focused: Style,

    /// Style for container and section titles.
    pub title: Style,

    /// Style used to mark the currently selected item.
    ///
    /// Suitable for selected list entries, table rows, menu items, and tabs.
    pub selected: Style,

    /// Style for controls or information that are currently unavailable.
    pub disabled: Style,

    /// Style for neutral informational messages.
    ///
    /// Suitable for connection messages, informational notices, and progress
    /// descriptions that do not represent success or failure.
    pub info: Style,

    /// Style for low-level diagnostic information used during development.
    ///
    /// Suitable for debug messages, internal state information, and diagnostic
    /// values that should be visible without drawing excessive attention.
    pub debug: Style,

    /// Style for structured or machine-oriented data values.
    ///
    /// Suitable for serialized values, field contents, identifiers, protocol
    /// values, and other structured data shown alongside a label.
    pub data: Style,

    /// Style indicating that a component is active, healthy, or operating
    /// normally.
    ///
    /// Suitable for enabled devices, established connections, completed
    /// operations, and valid states.
    pub success: Style,

    /// Style indicating that attention may be required, while operation can
    /// still continue.
    ///
    /// Suitable for inactive devices, degraded conditions, and non-critical
    /// warnings.
    pub warning: Style,

    /// Style indicating a failure or invalid state.
    ///
    /// Suitable for communication failures, device errors, validation errors,
    /// and failed operations.
    pub error: Style,

    /// Style for the filled part of a progress indicator.
    ///
    /// Suitable for gauges, progress bars, capacity indicators, and other
    /// normalized measurements.
    pub progress: Style,

    /// Style for the label displayed inside a progress indicator.
    pub progress_label: Style,
}

impl Theme {
    pub const fn default_theme() -> Self {
        Self {
            text: Style::new().fg(catppuchin_mocha::FG),

            muted: Style::new()
                .fg(catppuchin_mocha::FG_MUTED)
                .add_modifier(Modifier::DIM),

            accent: Style::new()
                .fg(catppuchin_mocha::ACCENT)
                .add_modifier(Modifier::BOLD),

            label: Style::new()
                .fg(catppuchin_mocha::FG)
                .add_modifier(Modifier::DIM),

            value: Style::new()
                .fg(catppuchin_mocha::FG)
                .add_modifier(Modifier::DIM),

            border: Style::new().fg(catppuchin_mocha::PANEL_BORDER),

            border_focused: Style::new().fg(catppuchin_mocha::ACCENT),

            title: Style::new()
                .fg(catppuchin_mocha::FG)
                .add_modifier(Modifier::BOLD),

            selected: Style::new()
                .fg(catppuchin_mocha::TEXT)
                .bg(catppuchin_mocha::SURFACE_2)
                .add_modifier(Modifier::BOLD),

            disabled: Style::new()
                .fg(catppuchin_mocha::OVERLAY_0)
                .add_modifier(Modifier::DIM),

            info: Style::new().fg(catppuchin_mocha::SKY),

            debug: Style::new().fg(catppuchin_mocha::BLUE),

            data: Style::new().fg(catppuchin_mocha::LAVENDER),

            success: Style::new()
                .fg(catppuchin_mocha::GREEN)
                .add_modifier(Modifier::BOLD),

            warning: Style::new().fg(catppuchin_mocha::YELLOW),

            error: Style::new()
                .fg(catppuchin_mocha::RED)
                .add_modifier(Modifier::BOLD),

            progress: Style::new()
                .fg(catppuchin_mocha::SURFACE_0)
                .add_modifier(Modifier::BOLD),

            progress_label: Style::new()
                .fg(catppuchin_mocha::TEXT)
                .add_modifier(Modifier::BOLD),
        }
    }

    /// Creates a standard bordered container using the default border style.
    pub fn block<'a>(&self, title: &'a str) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(self.border)
            .title_style(self.title)
            .title(format!(" {title} "))
    }

    /// Creates a bordered container using the focused border style.
    pub fn focused_block<'a>(&self, title: &'a str) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_focused)
            .title_style(self.accent)
            .title(format!(" {title} "))
    }
}

/// Default visual theme used by application widgets.
pub const DEFAULT_THEME: Theme = Theme::default_theme();
