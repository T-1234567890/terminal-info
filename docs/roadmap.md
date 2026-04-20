# Terminal Info Roadmap

This roadmap tracks the next major feature direction for Terminal Info after `v1.4.3`.

The core CLI, dashboard, widgets, plugin platform, diagnostics, AI helpers, productivity tools, and calendar/task attachment flows already exist. Future work should build on those foundations without turning Terminal Info into a heavy desktop app or cloud service.

## Completion Status

| Feature | Command area | Status | Target |
| --- | --- | --- | --- |
| Session Briefs | `tinfo brief` | Planned | `v1.5` |
| Project Mode | `tinfo project` | Planned | `v1.5` |
| Watch Rules | `tinfo watch` | Planned | `v1.6` |
| Release Assistant | `tinfo release` | Planned | `v1.5` |
| Environment Snapshot | `tinfo snapshot` | Planned | `v1.6` |
| Command Pins | `tinfo pin` | Planned | `v1.6` |
| Focus Mode | `tinfo focus` | Planned | `v1.7` |
| Local Knowledge Base | `tinfo kb` | Planned | `v1.7` |
| Personal Daily Brief | `tinfo today` | Planned | `v1.5` |

Status values:

- `Planned` means design and implementation have not started.
- `In progress` means code or docs are actively being built.
- `Complete` means the feature is implemented, documented, and covered by basic checks.
- `Deferred` means the feature is intentionally postponed.

## Vision

Terminal Info should become a local-first terminal command center for system awareness, developer workflow, lightweight productivity, and extensible plugin-based automation.

Guiding principles:

- Keep commands CLI-native and scriptable.
- Prefer local files and explicit user action over background services.
- Reuse existing config, dashboard, storage, and widget patterns.
- Keep new features small enough to work well in a single binary.
- Avoid external sync, accounts, or cloud dependencies unless the user explicitly configures them.

## v1.5 - Daily And Developer Workflow

### Personal Daily Brief

Command area:

```bash
tinfo today
tinfo today --plain
tinfo today --json
```

Goal:

Provide a focused daily view that is more intentional than the general dashboard.

Planned output:

- local date and time
- current weather summary
- calendar events for today
- open tasks
- active reminders
- recent notes
- active timer or stopwatch
- important system warnings

Completion checklist:

- Add `today` command.
- Reuse existing task, calendar, reminder, note, timer, weather, and system data.
- Support plain, compact, color, and JSON output where practical.
- Document in `README.md`, `docs/commands.md`, and `docs/productivity.md`.

### Session Briefs

Command area:

```bash
tinfo brief
tinfo brief --since yesterday
tinfo brief --workdir .
```

Goal:

Summarize what changed recently on the machine or in the current workspace.

Planned output:

- recent shell commands
- current Git branch and dirty state when inside a repo
- recent notes
- active or upcoming productivity items
- dashboard-relevant warnings
- plugin-provided brief sections when available

Completion checklist:

- Add `brief` command.
- Support time windows such as `today`, `yesterday`, and simple durations.
- Add project-aware mode through `--workdir`.
- Keep output concise by default.

### Project Mode

Command area:

```bash
tinfo project
tinfo project doctor
tinfo project dashboard
```

Goal:

Detect the current project and show useful local development context.

Planned detection:

- Rust via `Cargo.toml`
- Node via `package.json`
- Python via `pyproject.toml`, `requirements.txt`, or virtual environment markers
- Git repository state

Planned output:

- project type
- important scripts or commands
- dependency file status
- Git branch, dirty files, and unpushed commits
- detected test/build commands
- project-specific warnings

Completion checklist:

- Add project detection utilities.
- Add `project` summary command.
- Add `project doctor`.
- Avoid running expensive build/test commands unless explicitly requested.

### Release Assistant

Command area:

```bash
tinfo release check
tinfo release notes
tinfo release tag
tinfo release doctor
```

Goal:

Make Terminal Info releases safer and more repeatable.

Planned checks:

- Git worktree cleanliness
- Cargo version and lockfile consistency
- changelog or roadmap update hints
- tests/checks status
- existing local and remote tag detection
- generated release notes from recent commits
- plugin registry metadata validation when relevant

Completion checklist:

- Add non-destructive `release check`.
- Add release notes generation.
- Add tag helper that refuses to overwrite existing tags.
- Document release workflow.

## v1.6 - Local Monitoring And State

### Watch Rules

Command area:

```bash
tinfo watch add disk "storage --compact"
tinfo watch add api "ping api.example.com"
tinfo watch list
tinfo watch run
```

Goal:

Let users define lightweight local checks that can run on demand and appear in dashboard output.

Planned behavior:

- Store named watch rules locally.
- Run commands only when explicitly invoked by the user or dashboard refresh.
- Capture status, exit code, output summary, and last run time.
- Keep failures visible but non-fatal to the rest of the dashboard.

Completion checklist:

- Add local watch storage.
- Add watch command group.
- Add dashboard widget integration.
- Add timeout and output truncation safeguards.

### Environment Snapshot

Command area:

```bash
tinfo snapshot save before-upgrade
tinfo snapshot diff before-upgrade now
tinfo snapshot list
tinfo snapshot remove before-upgrade
```

Goal:

Save and compare machine state over time.

Snapshot contents:

- OS and shell summary
- relevant tool versions
- Terminal Info config summary
- installed plugins
- disk and storage summary
- network summary
- optional project summary when run inside a project

Completion checklist:

- Add snapshot storage format.
- Add save/list/diff/remove commands.
- Keep snapshots small and human-readable.
- Avoid storing secrets or full environment dumps.

### Command Pins

Command area:

```bash
tinfo pin add "cargo test"
tinfo pin add "git status --short"
tinfo pin list
tinfo pin run
tinfo pin remove 1
```

Goal:

Let users pin important commands or command outputs into their workflow.

Planned behavior:

- Store pinned commands locally.
- Show last exit status, output summary, and run age.
- Optionally expose pinned commands as a dashboard widget.
- Require explicit user action before running pinned commands.

Completion checklist:

- Add pin command group.
- Add dashboard widget.
- Add timeout and output truncation.
- Document safe usage.

## v1.7 - Focus And Local Knowledge

### Focus Mode

Command area:

```bash
tinfo focus start "release 1.5"
tinfo focus status
tinfo focus note "check plugin docs"
tinfo focus stop
```

Goal:

Group current work context into a lightweight session.

Planned behavior:

- Track active focus title.
- Associate notes, tasks, calendar items, timers, and recent commands with the focus session.
- Show current branch and project context when available.
- Provide a focused dashboard section.

Completion checklist:

- Add focus state storage.
- Add start/status/note/stop commands.
- Integrate with task and note flows where useful.
- Add dashboard display.

### Local Knowledge Base

Command area:

```bash
tinfo kb add "How to release"
tinfo kb search release
tinfo kb show 1
tinfo kb edit 1
```

Goal:

Provide a small searchable local knowledge base for longer notes, runbooks, and references.

Planned behavior:

- Store markdown entries locally.
- Search title and body text.
- Keep entries editable in the user's editor.
- Support linking knowledge entries from tasks or focus sessions later.

Completion checklist:

- Add local markdown-backed storage.
- Add add/search/show/edit/remove commands.
- Add optional dashboard summary for recent entries.
- Document privacy and local-only storage.

## Not Planned

These are intentionally out of scope for the near-term roadmap:

- external calendar sync
- recurring calendar events
- background notification daemon
- cloud account system
- always-on monitoring agent
- replacing dedicated project management tools

## Maintenance Expectations

Every completed roadmap item should include:

- command help text
- README or docs update
- basic tests or smoke checks
- no secrets, local paths, or user-specific data in committed examples
- compatibility with existing config and local storage patterns
