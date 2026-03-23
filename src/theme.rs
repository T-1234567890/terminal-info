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
    let width = rows
        .iter()
        .map(|(label, value)| label.as_ref().len() + 2 + value.as_ref().len())
        .max()
        .unwrap_or(0)
        .max(title.len());
    let border = border_chars(theme_config());
    let vertical = accentize(&border.vertical.to_string());
    let mut lines = vec![
        accentize(&format!(
            "{}{}{}",
            border.top_left,
            repeat(border.horizontal, width + 2),
            border.top_right
        )),
        format!("{} {} {}", vertical, accentize(&center_line(title, width)), vertical),
        accentize(&format!(
            "{}{}{}",
            border.junction_left,
            repeat(border.horizontal, width + 2),
            border.junction_right
        )),
    ];
    for (label, value) in rows {
        lines.push(format!(
            "{} {} {}",
            vertical,
            pad_line(&format!("{}: {}", label.as_ref(), value.as_ref()), width),
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
    let padding = width.saturating_sub(value.len());
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
}
