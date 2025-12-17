// src/theme.rs
// Neon Green & Black Hacker Aesthetic Theme

use ratatui::style::{Color, Modifier, Style};

// Core Theme Colors
pub const COLOR_NEON_GREEN: Color = Color::Rgb(57, 255, 20);  // Bright Neon Green
pub const COLOR_DARK_BG: Color = Color::Black;                // Deep Black
pub const COLOR_DIM_GREEN: Color = Color::Rgb(20, 100, 20);   // Dimmer Green for secondary text
pub const COLOR_ERROR_RED: Color = Color::Red;                // Red for errors (safety signal)

// Base Styles
pub const STYLE_BASE: Style = Style::new().fg(COLOR_NEON_GREEN).bg(COLOR_DARK_BG);
pub const STYLE_BORDER: Style = Style::new().fg(COLOR_NEON_GREEN);
pub const STYLE_HIGHLIGHT: Style = Style::new().bg(COLOR_NEON_GREEN).fg(COLOR_DARK_BG);
pub const STYLE_DIM: Style = Style::new().fg(COLOR_DIM_GREEN);

// Category and List Styles
pub const STYLE_CATEGORY_TITLE: Style = Style::new()
    .fg(COLOR_NEON_GREEN)
    .add_modifier(Modifier::BOLD);

pub const STYLE_CATEGORY_COUNT: Style = Style::new().fg(COLOR_DIM_GREEN);

pub const STYLE_OBJECT_NAME: Style = Style::new().fg(COLOR_NEON_GREEN);

pub const STYLE_OBJECT_CLSID: Style = Style::new().fg(COLOR_DIM_GREEN);

pub const STYLE_LIST_HIGHLIGHT: Style = Style::new()
    .bg(COLOR_NEON_GREEN)
    .fg(COLOR_DARK_BG)
    .add_modifier(Modifier::BOLD);

// Details/Metadata Styles
pub const STYLE_METADATA_LABEL: Style = Style::new()
    .fg(COLOR_NEON_GREEN)
    .add_modifier(Modifier::BOLD);

pub const STYLE_METADATA_TEXT: Style = Style::new().fg(COLOR_NEON_GREEN);

pub const STYLE_HINT_TEXT: Style = Style::new().fg(COLOR_DIM_GREEN);

// Member List Styles
pub const STYLE_METHOD_MARKER: Style = Style::new()
    .fg(COLOR_NEON_GREEN)
    .add_modifier(Modifier::BOLD);

pub const STYLE_PROPERTY_MARKER: Style = Style::new().fg(COLOR_DIM_GREEN);

pub const STYLE_MEMBER_NAME: Style = Style::new().fg(COLOR_NEON_GREEN);

// Status Bar Styles
pub const STYLE_STATUS_BAR: Style = Style::new()
    .bg(COLOR_NEON_GREEN)
    .fg(COLOR_DARK_BG)
    .add_modifier(Modifier::BOLD);

// Error Styles
pub const STYLE_ERROR_TITLE: Style = Style::new()
    .fg(COLOR_ERROR_RED)
    .add_modifier(Modifier::BOLD);

pub const STYLE_ERROR_TEXT: Style = Style::new().fg(COLOR_ERROR_RED);

// Modal/Notification Styles
pub const STYLE_NOTIFICATION_BG: Style = Style::new()
    .bg(COLOR_NEON_GREEN)
    .fg(COLOR_DARK_BG)
    .add_modifier(Modifier::BOLD);
