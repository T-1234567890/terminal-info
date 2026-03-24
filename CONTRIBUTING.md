# Contributing to Terminal Info

Thank you for your interest in contributing to **Terminal Info**!

Terminal Info is an open-source terminal toolbox written in Rust.  
It provides weather information, system diagnostics, network tools, and a plugin ecosystem designed for extensibility.

We welcome contributions of all kinds, including:

- Bug fixes
- New features
- Documentation improvements
- Performance optimizations
- Plugin development
- Developer tooling improvements

# Getting Started

### 1. Fork the Repository

Click **Fork** on GitHub and clone your fork:

```
git clone https://github.com/YOUR_USERNAME/terminal-info.git
cd terminal-info
```

Add the upstream repository:

```
git remote add upstream https://github.com/T-1234567890/terminal-info.git
```

# Development Setup

Install Rust if you have not already:

```
https://rustup.rs
```

Build Terminal Info:

```
cargo build
```

Run Terminal Info locally:

```
cargo run
```

Run tests:

```
cargo test
```

Format the code:

```
cargo fmt
```

Lint the code:

```
cargo clippy
```

Please ensure formatting and lint checks pass before submitting changes.

# Submitting Changes

1. Create a new branch

```
git checkout -b feature/my-feature
```

2. Make your changes

3. Commit your changes

```
git commit -m "feat: add new feature"
```

4. Push the branch

```
git push origin feature/my-feature
```

5. Open a Pull Request on GitHub

Please clearly describe the purpose of your change in the Pull Request.

# Commit Message Guidelines

Terminal Info generally follows a conventional commit style.

Common prefixes include:

```
feat: new feature
fix: bug fix
docs: documentation changes
refactor: internal code improvements
chore: maintenance tasks
```

Example:

```
feat: add weather alerts command
```

# Pull Request Types

Please use the correct PR type when contributing:

- `[core]` for core code changes
- `[plugin]` for plugin metadata submissions
- `[docs]` for documentation changes
- `[maintenance]` for CI, release, and repository maintenance

Plugin submissions must be opened as Pull Requests.

Plugin-related bugs should be reported in the plugin's own repository.


# Plugin Contributions

Terminal Info includes a plugin ecosystem that allows developers to extend the CLI with new commands.

Plugins are typically distributed as separate repositories.

Example plugin command:

```
tinfo plugin install example-plugin
```

General plugin guidelines:

- Plugins should provide clear documentation
- Plugins should avoid conflicting command names
- Plugins should follow the Terminal Info plugin conventions

Plugin documentation will be provided in:

```
docs/plugin-development.md
```


# Reporting Issues

If you encounter a bug, please open a GitHub issue and include:

- Operating system (macOS, Linux, Windows)
- Terminal Info version
- Command that was executed
- Full output or error message

Example command:

```
tinfo weather now
```

# Code Style

Terminal Info follows standard Rust conventions.

Please ensure:

- Code is formatted with `cargo fmt`
- There are no `cargo clippy` warnings
- Changes are focused and minimal
- Unrelated refactoring is avoided in the same Pull Request

---

# Security

If you discover a security vulnerability in Terminal Info, please report it privately before opening a public issue.

Security-related issues may involve:

- plugin execution
- installation scripts
- update mechanisms
- external command execution

# Community Guidelines

Please be respectful and constructive when interacting with contributors.

Terminal Info follows the **Contributor Covenant Code of Conduct**.

# License

By contributing to Terminal Info, you agree that your contributions will be licensed under the same license as the project (Apache 2.0).
