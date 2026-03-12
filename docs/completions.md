# Shell Completions

`tinfo` can generate shell completions for:

- `bash`
- `zsh`
- `fish`

Commands:

```bash
tinfo completion bash
tinfo completion zsh
tinfo completion fish
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
