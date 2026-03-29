use std::sync::{OnceLock, RwLock};

use crossterm::style::{Color, Stylize};
use serde::{Deserialize, Serialize};

use crate::output::{OutputMode, output_mode};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    #[default]
    Sharp,
    Rounded,
}

impl BorderStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sharp => "sharp",
            Self::Rounded => "rounded",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AccentColor {
    #[default]
    Auto,
    Blue,
    Cyan,
    Green,
    Magenta,
    Red,
    Yellow,
}

impl AccentColor {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Blue => "blue",
            Self::Cyan => "cyan",
            Self::Green => "green",
            Self::Magenta => "magenta",
            Self::Red => "red",
            Self::Yellow => "yellow",
        }
    }

    fn terminal_color(self) -> Color {
        match self {
            Self::Auto | Self::Cyan => Color::Cyan,
            Self::Blue => Color::Blue,
            Self::Green => Color::Green,
            Self::Magenta => Color::Magenta,
            Self::Red => Color::Red,
            Self::Yellow => Color::Yellow,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
pub struct ThemeConfig {
    #[serde(default)]
    pub border_style: BorderStyle,
    #[serde(default)]
    pub accent_color: AccentColor,
    #[serde(default)]
    pub ascii_only: bool,
}

impl ThemeConfig {
    pub fn unicode_enabled(self) -> bool {
        !self.ascii_only
    }
}

#[derive(Clone, Copy)]
struct BorderChars {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    junction_left: char,
    junction_right: char,
}

static THEME_CONFIG: OnceLock<RwLock<ThemeConfig>> = OnceLock::new();

pub fn set_theme(theme: ThemeConfig) {
    let lock = THEME_CONFIG.get_or_init(|| RwLock::new(ThemeConfig::default()));
    if let Ok(mut current) = lock.write() {
        *current = theme;
    }
}

pub fn theme_config() -> ThemeConfig {
    THEME_CONFIG
        .get_or_init(|| RwLock::new(ThemeConfig::default()))
        .read()
        .map(|theme| *theme)
        .unwrap_or_default()
}

pub fn format_box_table<T, U>(title: &str, rows: &[(T, U)]) -> String
where
    T: AsRef<str>,
    U: AsRef<str>,
{
    format_box_table_with_width(title, rows, None)
}

pub fn format_box_table_with_width<T, U>(
    title: &str,
    rows: &[(T, U)],
    max_content_width: Option<usize>,
) -> String
where
    T: AsRef<str>,
    U: AsRef<str>,
{
    let rendered_rows = rows
        .iter()
        .map(|(label, value)| render_row(label.as_ref(), value.as_ref()))
        .collect::<Vec<_>>();
    let mut width = rendered_rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0)
        .max(title.chars().count());
    if let Some(limit) = max_content_width.filter(|limit| *limit > 0) {
        width = width.min(limit);
    }
    let title = truncate_line(title, width);
    let border = border_chars(theme_config());
    let vertical = accentize(&border.vertical.to_string());
    let mut lines = vec![
        accentize(&format!(
            "{}{}{}",
            border.top_left,
            repeat(border.horizontal, width + 2),
            border.top_right
        )),
        format!("{} {} {}", vertical, accentize(&center_line(&title, width)), vertical),
        accentize(&format!(
            "{}{}{}",
            border.junction_left,
            repeat(border.horizontal, width + 2),
            border.junction_right
        )),
    ];
    for row in rendered_rows {
        lines.push(format!(
            "{} {} {}",
            vertical,
            pad_line(&truncate_line(&row, width), width),
            vertical
        ));
    }
    lines.push(accentize(&format!(
        "{}{}{}",
        border.bottom_left,
        repeat(border.horizontal, width + 2),
        border.bottom_right
    )));
    format!("{}\n", lines.join("\n"))
}

fn render_row(label: &str, value: &str) -> String {
    if label.trim().is_empty() {
        value.to_string()
    } else {
        format!("{label}: {value}")
    }
}

fn border_chars(theme: ThemeConfig) -> BorderChars {
    if theme.ascii_only {
        return BorderChars {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            junction_left: '+',
            junction_right: '+',
        };
    }

    match theme.border_style {
        BorderStyle::Sharp => BorderChars {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
            junction_left: '├',
            junction_right: '┤',
        },
        BorderStyle::Rounded => BorderChars {
            top_left: '╭',
            top_right: '╮',
            bottom_left: '╰',
            bottom_right: '╯',
            horizontal: '─',
            vertical: '│',
            junction_left: '├',
            junction_right: '┤',
        },
    }
}

fn accentize(value: &str) -> String {
    if !matches!(output_mode(), OutputMode::Color) {
        return value.to_string();
    }

    format!("{}", value.with(theme_config().accent_color.terminal_color()))
}

fn repeat(character: char, count: usize) -> String {
    std::iter::repeat_n(character, count).collect()
}

fn pad_line(value: &str, width: usize) -> String {
    format!("{value:<width$}")
}

fn center_line(value: &str, width: usize) -> String {
    let value_width = value.chars().count();
    let padding = width.saturating_sub(value_width);
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
}

fn truncate_line(value: &str, width: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return value.to_string();
    }
    if width <= 1 {
        return "…".repeat(width);
    }
    let mut output = chars
        .into_iter()
        .take(width.saturating_sub(1))
        .collect::<String>();
    output.push('…');
    output
}
