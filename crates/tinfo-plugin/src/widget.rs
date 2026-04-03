use serde::{Deserialize, Serialize};

/// Dashboard render mode requested by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetMode {
    Compact,
    Full,
}

/// Structured widget payload returned by plugins.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Widget {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_interval_secs: Option<u64>,
    pub full: WidgetBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact: Option<WidgetBody>,
}

impl Widget {
    /// Create a widget with a required full-mode body.
    pub fn new(title: impl Into<String>, full: WidgetBody) -> Self {
        Self {
            title: title.into(),
            refresh_interval_secs: None,
            full,
            compact: None,
        }
    }

    /// Set a compact-mode body.
    pub fn compact(mut self, compact: WidgetBody) -> Self {
        self.compact = Some(compact);
        self
    }

    /// Hint how often the host should refresh this widget.
    pub fn refresh_interval_secs(mut self, secs: u64) -> Self {
        self.refresh_interval_secs = Some(secs.max(1));
        self
    }
}

/// Supported structured widget body kinds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WidgetBody {
    Text {
        content: String,
    },
    List {
        items: Vec<String>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

impl WidgetBody {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
        }
    }

    pub fn list<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::List {
            items: items.into_iter().map(Into::into).collect(),
        }
    }

    pub fn table<H, R, C>(headers: H, rows: R) -> Self
    where
        H: IntoIterator<Item = C>,
        R: IntoIterator,
        R::Item: IntoIterator<Item = C>,
        C: Into<String>,
    {
        Self::Table {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().map(Into::into).collect())
                .collect(),
        }
    }
}
