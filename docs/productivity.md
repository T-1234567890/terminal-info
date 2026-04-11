# Productivity

Terminal Info includes small local-first productivity tools for timers, stopwatches, tasks, calendar events, notes, shell history, and reminders.

All productivity state is stored locally under:

```text
~/.tinfo/data/
```

The feature set is intentionally lightweight. Calendar events are task records with a date or datetime attached; reminders are separate countdown-style alerts; there is no recurrence, notification daemon, external calendar sync, or timezone management.

## Timers

```bash
tinfo timer
tinfo timer start
tinfo timer start 25m
tinfo timer stop
```

- `timer` opens a live timer dashboard by default.
- `timer start [duration]` starts a countdown.
- When duration is omitted, Terminal Info uses the configured default timer duration.
- Durations support compact forms such as `15m`, `1h30m`, `45s`, and bare numbers interpreted as minutes.
- Timer state is stored in `~/.tinfo/data/timer.json`.
- The timer widget shows active countdowns and stopwatches in the dashboard.

## Stopwatch

```bash
tinfo stopwatch start
tinfo stopwatch stop
```

- The stopwatch is separate from countdown timers.
- Stopwatch state shares `~/.tinfo/data/timer.json`.
- The dashboard timer widget can show countdown and stopwatch state together.

## Tasks

```bash
tinfo task
tinfo task add finish README
tinfo task add --event 1 bring notes
tinfo task list
tinfo task done 1
tinfo task delete 1
```

- `task` opens the interactive task menu.
- `task add ...` creates a local task with no event attachment by default.
- `task add --event <id> ...` creates a task attached to an existing calendar event by copying that event's date/time onto the new task.
- `task list` prints local tasks.
- `task done <id>` completes a task.
- `task delete <id>` moves a task into the recoverable deleted-task area.
- Deleted tasks can be recovered from the interactive menu for 7 days.
- Expired deleted tasks are purged automatically when the task store is loaded.
- Task state is stored in `~/.tinfo/data/tasks.json`.

The interactive task menu includes:

- current task list
- `List all tasks`
- `Deleted tasks`
- `Add task`
- `Delete task`
- `Exit`

When adding a task interactively, Terminal Info asks which existing event to attach when calendar events exist. `No event` is the default selection.

## Calendar

```bash
tinfo calendar
tinfo calendar add "Planning" 2026-04-12
tinfo calendar add "Planning" 2026-04-12 --time 14:30
tinfo calendar add "Planning" 2026-04-12T14:30
tinfo calendar attach 2 2026-04-12
tinfo calendar attach 2 2026-04-12 --time 14:30
tinfo calendar list
tinfo calendar list --today
tinfo calendar list --upcoming --limit 3
tinfo calendar remove 1
```

- `calendar` with no subcommand lists scheduled task events.
- `calendar add` creates a task-backed calendar event.
- `calendar add` requires a date in `YYYY-MM-DD` format.
- `--time HH:MM` is optional.
- The combined legacy-compatible form `YYYY-MM-DDTHH:MM` is also accepted.
- `calendar attach <task-id> <date>` attaches an existing task to a calendar date.
- `calendar attach` also accepts optional `--time HH:MM`.
- `calendar list` shows only tasks with calendar dates.
- `calendar list` sorts events by date/time ascending.
- `calendar list --today` shows events dated today.
- `calendar list --upcoming` shows events today or later, and timed events later than the current local time.
- `calendar list --limit <n>` truncates output.
- `calendar remove <id>` removes the task using the existing task delete/recovery behavior.

Calendar event fields are minimal:

- `id`
- `title`
- `datetime`, stored as either `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`
- optional `description`

The calendar widget reads from `~/.tinfo/data/tasks.json`, ignores tasks without a calendar date, shows the next upcoming event in compact mode, and shows up to three events in full mode.

## Notes

```bash
tinfo note add remember to rotate keys
tinfo note list
```

- `note add ...` appends a quick local note.
- `note list` prints stored notes.
- Notes are stored in `~/.tinfo/data/notes.json`.
- The notes widget shows recent notes in the dashboard.

## History

```bash
tinfo history --limit 10
```

- `history` shows recent shell history lines from the detected shell history file.
- It does not require shell integration beyond a normal history file.
- `--limit <n>` controls how many entries are shown.

## Reminders

```bash
tinfo remind
tinfo remind 15m
tinfo remind 14:30 stand up
tinfo remind 30m stand up
```

- `remind` schedules a local reminder.
- If the time argument is omitted, Terminal Info uses the configured default reminder duration.
- Reminder delays support forms such as `15m`, `1h30m`, and `45s`.
- Clock times such as `14:30` schedule the next matching local time.
- Reminders are stored in `~/.tinfo/data/reminders.json`.
- After scheduling, Terminal Info prints: `Note: reminders trigger while the dashboard is running.`
- `tinfo remind ...` opens the live dashboard so the reminder scheduler is active immediately.

## Dashboard Widgets

Productivity widgets are built into the dashboard:

- `timer`
- `tasks`
- `calendar`
- `notes`
- `history`
- `reminders`

Widget order is configured in `~/.tinfo/config.toml`:

```toml
[dashboard]
widgets = ["weather", "time", "network", "system", "timer", "tasks", "calendar", "notes", "history", "reminders", "plugins"]
```

Quick widget commands:

```bash
tinfo config widgets show
tinfo config widgets add calendar
tinfo config widgets remove calendar
tinfo config widgets set weather time system timer tasks calendar notes history reminders plugins
tinfo config widgets reset
```

See [widgets.md](widgets.md) for dashboard rendering behavior and plugin widget details.
