# Chat

`tinfo chat` starts a simple interactive AI chat in the terminal.

## Recommendation

For most users, `OpenRouter` is the best default choice.

Why:
- broader model catalog
- easier model switching
- one provider key for many backends

## Start

```bash
tinfo chat
```

Optional flags:

```bash
tinfo chat --provider openai
tinfo chat --provider claude
tinfo chat --provider openrouter
tinfo chat --model gpt-5.4
tinfo chat --system "You are concise and technical."
```

## In-Chat Commands

- `/provider` switch provider
- `/model` switch model
- `/new` start a new chat
- `/chats` open saved chats
- `exit` leave chat
- `quit` leave chat

## Provider Models

### OpenAI

- `gpt-5.4`
- `gpt-5.4-mini`
- `gpt-5.4-nano`
- `gpt-5.1`
- `gpt-5-mini`
- `gpt-5-nano`
- `gpt-5-pro`
- `gpt-5`
- `gpt-4.1`
- `o3-deep-research`

### Claude

- `claude-opus-4-6`
- `claude-sonnet-4-6`
- `claude-haiku-4-5`

### OpenRouter

- `z-ai/glm-5v-turbo`
- `stepfun/step-3.5-flash:free`
- `qwen/qwen3.6-plus-preview`
- `nvidia/nemotron-3-super:free`
- `anthropic/claude-4.6-sonnet`
- `anthropic/claude-4.6-opus`
- `openai/gpt-5.4-pro`
- `openai/gpt-5.3-codex`
- `google/gemini-3.1-pro-preview`
- `google/gemini-3.1-flash`
- `deepseek/deepseek-v3.2`
- `deepseek/deepseek-r1`
- `xiaomi/mimo-v2-pro`
- `minimax/minimax-m2.7`
- `x-ai/grok-4.20-multi-agent`
- `x-ai/grok-4.20`
- `meta/llama-4-400b-instruct`
- `mistralai/mistral-large-2603`
- `mistralai/devstral-2-123b`
- `z-ai/glm-5`
- `z-ai/glm-4.5-air`
- `openai/gpt-5.4-nano`
- `openai/gpt-5.4`
- `openai/gpt-oss-120b`
- `moonshotai/kimi-k2.5`
- `liquid/lfm-2.5-thinking`
- `google/gemma-4-31b-dense`

## Custom OpenRouter Models

When using `OpenRouter`, the model picker includes `Custom model...`.

Custom models must use the format:

```text
provider/model
```

Examples:

- `openai/gpt-5.4-pro`
- `anthropic/claude-4.6-sonnet`
- `google/gemini-3.1-flash`

Allowed characters:
- letters and numbers
- `/`
- `.`
- `-`
- `_`
- `:`

Invalid custom values are rejected immediately.

## History And Context

Config is shared through `~/.tinfo/config.toml`.

Relevant settings:

```toml
[ai.runtime]
chat_history = true
chat_context = true
```

- `chat_history = true` saves chats and enables `/chats`
- `chat_context = true` sends prior messages back to the provider
- `chat_context = false` keeps the active chat local, but only sends the latest message

## API Keys

Keys are stored in OS secure storage, not in plaintext config.

`tinfo chat` prompts for a key only after the provider is known.
