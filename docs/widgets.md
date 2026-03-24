# Widgets

Terminal Info widgets are small dashboard data sources. They do not own layout or terminal rendering. The dashboard loop collects widget data, chooses compact or full mode, and renders everything centrally.

Widgets come in two forms:

- built-in widgets such as `weather`, `time`, `network`, `system`, and `notes`
- plugin widgets returned by trusted plugins through `--widget`

## Dashboard Configuration

Widget order is configured in `~/.tinfo/config.toml`:

```toml
[dashboard]
widgets = ["weather", "time", "network", "system", "notes", "plugins"]
refresh_interval = 1
compact_mode = false
freeze = false
```

Supported built-in widget names:

- `weather`
- `time`
- `network`
- `system`
- `notes`
- `plugins`

Notes:

- `plugins` renders all trusted plugin widgets
- `notes` renders only when `~/.tinfo/dashboard-notes.txt` exists and contains non-empty lines
- `freeze = true` captures one snapshot and reuses it instead of refreshing live
- `refresh_interval` controls the dashboard loop; plugin widgets can also provide their own refresh hint

Quick widget commands:

```bash
tinfo config widgets show
tinfo config widgets add notes
tinfo config widgets remove network
tinfo config widgets set weather time system notes plugins
tinfo config widgets reset
```

Interactive widget configuration is available from:

```bash
tinfo config
```

Then open the `Widgets` submenu.

## Notes Widget

The built-in notes widget stores plain text in:

```text
~/.tinfo/dashboard-notes.txt
```

Manage it with:

```bash
tinfo dashboard notes show
tinfo dashboard notes set remember to rotate keys
tinfo dashboard notes clear
```

Rendering behavior:

- full mode shows up to a few note lines as a list
- compact mode shows only the first note line
- notes are re-read on a short interval and do not block the rest of the dashboard

## Plugin Widget API

Plugins should return structured widget data, not terminal UI.

In the Rust SDK, a plugin registers a widget handler with:

```rust
Plugin::new("weather")
    .widget(dashboard_widget)
    .dispatch();
```

The handler signature is:

```rust
fn dashboard_widget(ctx: tinfo_plugin::Context, mode: tinfo_plugin::WidgetMode)
    -> tinfo_plugin::PluginResult<tinfo_plugin::Widget>
```

Available widget mode values:

- `WidgetMode::Compact`
- `WidgetMode::Full`

Available widget body types:

- `WidgetBody::text(...)`
- `WidgetBody::list(...)`
- `WidgetBody::table(...)`

Minimal example:

```rust
use tinfo_plugin::{Plugin, PluginResult, Widget, WidgetBody, WidgetMode};

fn dashboard_widget(ctx: tinfo_plugin::Context, _mode: WidgetMode) -> PluginResult<Widget> {
    let city = ctx.config.string("location")?.unwrap_or_else(|| "auto".to_string());

    Ok(
        Widget::new(
            "Weather",
            WidgetBody::table(
                ["Field", "Value"],
                [["City", city.as_str()], ["Host", ctx.host.version()]],
            ),
        )
        .compact(WidgetBody::text(format!("city={city}")))
        .refresh_interval_secs(30),
    )
}

fn main() {
    Plugin::new("weather")
        .description("Weather information plugin")
        .widget(dashboard_widget)
        .dispatch();
}
```

## Widget JSON Schema

The host reads widget JSON from:

```bash
tinfo-<plugin-name> --widget
tinfo-<plugin-name> --widget --compact
```

Stable schema:

```json
{
  "title": "CPU",
  "refresh_interval_secs": 2,
  "full": {
    "type": "table",
    "headers": ["Metric", "Value"],
    "rows": [["Usage", "18%"], ["Load", "1.42"]]
  },
  "compact": {
    "type": "text",
    "content": "18%"
  }
}
```

Body variants:

`text`

```json
{
  "type": "text",
  "content": "18%"
}
```

`list`

```json
{
  "type": "list",
  "items": ["task one", "task two"]
}
```

`table`

```json
{
  "type": "table",
  "headers": ["Metric", "Value"],
  "rows": [["Usage", "18%"], ["Load", "1.42"]]
}
```

Semantics:

- `title` is required
- `full` is required
- `compact` is optional; if omitted, the host falls back to `full`
- `refresh_interval_secs` is optional and treated as a hint, not a hard contract

## Legacy Compatibility

Older plugins can still return:

```json
{
  "title": "News",
  "content": "3 unread items"
}
```

The host converts that into a text widget automatically. New plugins should use the structured schema.

## Render Model

The dashboard remains intentionally simple:

- the dashboard loop owns refresh timing
- each widget provides data, not layout
- compact and full rendering are selected by the host
- widget refresh hints are used for caching, not for independent background threads

This keeps the widget system stable, easy to document, and easy to implement in both built-in and plugin widgets.
