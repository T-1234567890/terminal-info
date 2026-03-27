# 🧭 Terminal Info Roadmap

This document outlines the public development roadmap for **terminal-info** from v1.0 to v1.2.

> This roadmap reflects high-level direction and may evolve as the project grows.

---

## 🎯 Vision

terminal-info aims to become a **modular, extensible system diagnostics toolbox**, powered by a flexible plugin ecosystem and a lightweight dashboard experience.

---

# 🚀 Roadmap Overview

## ✅ v1.0.x — Foundation (Completed / Stabilizing)

**Status:** ✅

- Core CLI commands (system, network, storage, diagnostics)
- Plugin system (plugin_api = 1)
- Plugin SDK (Rust)
- Plugin registry & metadata
- Basic dashboard support (experimental widgets)
- JSON / structured output
- Release + CI pipeline

---

## 🔧 v1.1.0 — Dashboard & Widgets (Next Major Step)

**Status:** ✅

### 🎯 Goals
Make the dashboard a **first-class feature**

### ✨ Planned Features

- Full dashboard rendering system
- Stable widget lifecycle (load / refresh / render)
- Widget layout system
- Built-in widgets (system, network, storage)
- Plugin-based widgets (trusted plugins)

### 🔌 Widgets API

- Widget definition schema
- Structured widget output (JSON-based)
- Refresh hints & update intervals
- Compact / full rendering modes
- Plugin SDK support for widgets

---

## ⏱️ v1.2.x — Productivity Tools

**Status:** In progress

### 🎯 Goals
Add simple, useful tools that improve everyday terminal workflows

### ✨ Features

- Timer
  - `tinfo timer start`
  - `tinfo timer stop`
  - `tinfo timer <duration>`
  - simple countdown/stopwatch

- Task manager
  - `tinfo task add`
  - `tinfo task list`
  - `tinfo task done`
  - lightweight CLI todo system

- Quick notes
  - `tinfo note add`
  - `tinfo note list`
  - store short notes directly in terminal

- Command history helper
  - show recent commands
  - quick recall/reuse

- Simple reminders
  - `tinfo remind <time>`
  - notify user after delay

---

## 🔮 Future Direction (Beyond v1.2)

- Better UI
- Cloud integration
- Features from issues

---

## ⚠️ Notes

- This roadmap is **directional, not guaranteed**
- Priorities may shift based on feedback and real-world usage

---

## 💡 Contributing

Contributions, ideas, and feedback are welcome.  
Feel free to open issues or discussions.
