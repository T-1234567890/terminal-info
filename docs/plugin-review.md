# 🔍 Plugin Review Guidelines

This document describes how plugins are reviewed before being accepted into the registry.

---

## 🎯 Purpose

The goal of plugin review is to ensure:

- Safety
- Basic quality
- Compatibility with terminal-info

This is not a strict code audit, but a **lightweight trust and quality check**.

---

## ✅ Requirements

A plugin must meet the following requirements:

### 1. Public repository

- The repository must be publicly accessible
- The code must be available for review

---

### 2. Valid license

- Must use a recognized open-source license (e.g. MIT, Apache-2.0)
- License must be clearly stated

---

### 3. Clear purpose

- The plugin must have a clear and useful function
- Description must match actual behavior

---

### 4. Basic documentation

- README should explain:
  - What the plugin does
  - How to use it

---

---

## 🔒 Security expectations

Plugins must NOT:

- Contain obvious malicious behavior
- Perform hidden network requests without explanation
- Execute unsafe system operations without user awareness
- Obfuscate critical logic

---

## ⚠️ Important note

> Plugins are reviewed on a best-effort basis.

Acceptance into the registry **does NOT guarantee full security or correctness**.

Users should always review plugins before using them in sensitive environments.

---

## ❌ Reasons for rejection

A plugin may be rejected if:

- Repository is private or inaccessible
- License is missing or unclear
- Description is misleading
- Code appears unsafe or suspicious
- Plugin is incomplete or non-functional

---

## 🔁 Updates and removal

Plugins may be:

- Updated via Pull Request
- Removed if they become unsafe or unmaintained

---

## 🧠 Responsibility

- This repository maintains the plugin index
- Plugin authors are responsible for their own code
- Issues related to plugins should be opened in the plugin repository

---

## 📌 Submission

Plugins must be submitted via Pull Request using the `[plugin]` prefix.
