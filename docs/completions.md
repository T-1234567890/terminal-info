# Shell Completions

`tinfo` can generate shell completions for:

- `bash`
- `zsh`
- `fish`
- `powershell`

Commands:

```bash
tinfo completion bash
tinfo completion zsh
tinfo completion fish
tinfo completion powershell
tinfo completion install
tinfo completion uninstall
tinfo completion status
```

## Quick Usage

Print completions to stdout:

```bash
tinfo completion zsh
```

Save them manually:

```bash
tinfo completion bash > tinfo.bash
tinfo completion zsh > _tinfo
tinfo completion fish > tinfo.fish
```

Install automatically for the current shell:

```bash
tinfo completion install
```

Remove the installed file for the current shell:

```bash
tinfo completion uninstall
```

Check the current install status:

```bash
tinfo completion status
```

## Examples

Temporary use in a shell session:

```bash
source <(tinfo completion bash)
```

For `zsh`:

```bash
tinfo completion zsh > "${fpath[1]}/_tinfo"
```

For `fish`:

```bash
tinfo completion fish > ~/.config/fish/completions/tinfo.fish
```

For PowerShell:

```powershell
tinfo completion powershell > ~/Documents/PowerShell/Completions/tinfo.ps1
```

## Covered Commands

Generated completions include built-in command groups such as:

- `weather`
- `diagnostic`
- `config`
- `profile`
- `plugin`
- `completion`

They also include global flags like:

- `--plain`
- `--compact`
- `--color`
- `--json`
