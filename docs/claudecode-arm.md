# Claude inside the agent — the CLAUDECODE arm

**Status: verified live, 2026-08-17.** The agent's frontier arm can be
Claude itself, through the Claude Code CLI, drawing on a Pro/Max
subscription's OAuth login instead of metered API credits. The bridge
is `agent.llm.claude_code`; this page is its manual. (The runbook's
section 8 is the recap; this is the full story.)

## What one flip covers

```properties
# runtime/agent/botd.properties — both are LIVE keys: re-read on
# every call, no restart needed
LLM=CLAUDECODE
LLM_CTL=agent:llm:claude_code
```

`chat_llm` is the single door every frontier request goes through, so
this routes **all of it** at once:

- **the agent app's chat** (the chat tab at `/agent/index.html`),
- **the dev-session chat plugin** (the notebook's ask row — same
  `chat_llm` underneath),
- **salience escalations and epsilon audits** — `ask_llm` *is*
  `chat_llm` with a two-message conversation, so the executive's
  band-uncertain verdicts (0.35–0.65) and its 5% audits also land on
  Claude.

There is no per-surface split, deliberately (the no-seams rule). If
you want vLLM judging escalations and Claude only chatting, that is a
design change to ask for, not a setting.

## Prerequisites

- Claude Code installed on the box and **logged in** (`claude` on
  PATH; set `CLAUDE_CODE_BIN=/full/path/to/claude` if not).
- Nothing else. **Auth is the point**: the CLI authenticates from the
  OAuth login in `~/.claude`, so calls draw on the subscription. The
  bridge strips `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` from the
  child environment so an inherited key can never silently move
  billing onto metered credits (`CLAUDE_CODE_ALLOW_API_KEY=on` opts
  back in). Never pass `--bare` via `CLAUDE_CODE_ARGS` — it makes
  auth strictly API-key and defeats the purpose.

## The two postures

### 1. Cheap oracle (the default)

Out of the box: built-in tools OFF, system prompt **replaced** by
whatever the agent sends. Measured difference: Claude Code's own
system prompt plus CLAUDE.md discovery costs ~38k cache-creation
tokens per trivial call; replaced, ~200. This is the right posture
for salience escalations and plain question-answering.

### 2. Full agent — Claude living inside the chat

```properties
CLAUDE_CODE_SYSTEM_MODE=append
CLAUDE_CODE_MCP={"mcpServers":{"newbound":{"command":"./target/release/newbound","args":["mcp"]}}}
CLAUDE_CODE_CWD=/path/to/your/newbound/checkout
CLAUDE_CODE_PERMISSION_MODE=bypassPermissions
```

`append` keeps Claude Code's own system prompt (and pays for it) so
the delegate behaves like a real coding session; the MCP config hands
it **newbound's own MCP server** — every store command, not the
subset the tool loop happened to forward; `CWD` makes the relative
binary path resolve (and puts file tools in the right checkout);
`--strict-mcp-config` is added automatically so the delegate never
sees whatever other MCP servers the invoking user has configured.

A chat turn in the agent app is then a genuine Claude session that
can read controls, run store commands, and edit code — and it
returns when the work is done.

## What the delegate knows (the CLAUDE.md question)

A dev session working ON this codebase gets its environment knowledge
from CLAUDE.md and docs/interim-process.md. The inside delegate gets
the equivalent from FOUR store-resident layers, all of which already
ride every chat turn as the system prompt — nothing needs copying
into files:

1. **The platform curriculum** — the `agent.prompts` control's
   `prompt` facet (docs/prompting.md): the journaled platform
   knowledge both chat surfaces share. Teach the platform here.
2. **The venue shell + memory index** — each surface's own framing;
   the notebook injects the live memory index (domains, staleness
   marks, entries relevant to the open surface) automatically.
3. **The owner addendum** — the `agentprompt` control's js facet:
   YOUR standing instructions, appended to every ask. Extend the
   delegate's rules here, not in a file.
4. **The resident context** — injected by this bridge itself whenever
   `CLAUDE_CODE_MCP` is set (`CLAUDE_CODE_CONTEXT=off` suppresses
   it): where the delegate is (a LIVE instance), what its hands are
   (store commands as MCP tools, every param required), the write
   rules (journaled commands only, never `data/*` by hand,
   destructive experiments belong in a disposable), the memory ritual
   (orient via `agent-archivist-recall`, deposit only for the user's
   asks or durable lessons from requested work), and where the deep
   docs live. It also explicitly overrides the native tool-loop
   instructions (find_tools/call_command), which describe a harness
   this delegate does not have. Environment knowledge attaches at the
   layer that knows the environment.

Verified live: asked to look up a command and state its rules, the
delegate called the store through MCP, quoted the command's real
desc, and answered "writes must never touch data/* files by hand and
must go through the journaled dev.code commands, with mutating
experiments confined to a disposable checkout" — the rules arrived.

## Why this is not a drop-in model provider

chat_llm's other arms answer ONE model turn and hand tool calls back
for newbound's `tool_loop` to execute. Claude Code is an agent
harness: it runs its OWN loop with its OWN tools and returns only
when finished. So this arm always answers `kind:"text"` — never
`kind:"tool_calls"` — and `tool_loop` terminates on it. That is the
honest mapping: the turn was delegated wholesale, and the text IS the
finished result. The `tools` newbound passes are deliberately ignored
(they name commands only the server process can run); the MCP wiring
above is how the delegate gets real hands.

## Every knob

All in `runtime/agent/botd.properties`, all live (read per call).
They also appear in the mind tab's config card.

| Key | Default | What it does |
| :-- | :-- | :-- |
| `CLAUDE_CODE_BIN` | `claude` | Path to the CLI. |
| `CLAUDE_CODE_MODEL` | *(CLI default)* | `--model` for the delegate. |
| `CLAUDE_CODE_EFFORT` | *(CLI default)* | `--effort` (low/medium/high…). |
| `CLAUDE_CODE_SYSTEM_MODE` | `replace` | `replace` = agent's prompt only (~200 tokens); `append` = keep Claude Code's prompt (full agentic behavior, full price). |
| `CLAUDE_CODE_MCP` | *(none)* | Path or literal JSON for `--mcp-config`; `--strict-mcp-config` rides along automatically. |
| `CLAUDE_CODE_PERMISSION_MODE` | *(none)* | `--permission-mode`, e.g. `bypassPermissions` for an unattended delegate. |
| `CLAUDE_CODE_TOOLS` | *(empty = built-ins OFF)* | `default` restores the built-in toolset; or name them (`Bash,Read`). MCP tools are unaffected. |
| `CLAUDE_CODE_CWD` | *(server's cwd)* | Working directory for the delegate. |
| `CLAUDE_CODE_TIMEOUT` | `600` | Seconds before the whole process tree is killed and the failure reported. |
| `CLAUDE_CODE_ARGS` | *(none)* | Extra whitespace-split argv (no spaces inside a value). |
| `CLAUDE_CODE_ALLOW_API_KEY` | `off` | `on` lets an inherited `ANTHROPIC_API_KEY` through (bills credits). |

Every answer carries `cost_usd`, `num_turns`, and `session_id` from
the CLI's JSON result — on a subscription the cost is charged to the
plan's allowance rather than billed, but it still measures what was
spent. Calls are stateless (`--no-session-persistence`): machine
traffic never litters the `/resume` picker.

## The escalation-traffic trade

With this arm on, every band escalation (rate-capped at one per 5s)
spawns a CLI call. In replace mode each is cheap, but if the
escalation log runs hot you are spending plan allowance on judgment
calls a local vLLM box could make for free. Watch the mind tab's
frontier-calls stat; the trade is yours to pick per box — the arm
flips back to `LLM=VLLM` just as live.

## Verification record

2026-08-17, on a disposable instance with real calls: `chat_llm`
answered through the arm (`kind:text`, `cost_usd` attached);
`ask_llm` answered through the same path. All CLI flags the bridge
builds exist in CLI 2.1.233. One memorable moment: asked to echo the
test string "ESCALATION-OK", the delegate **refused** — the phrase
pattern-matched an authorization-token probe. The refusal itself
proved the path end-to-end; benign prompts pass normally. A frontier
arm with judgment is, after all, what the escalation lane is for.

## Troubleshooting

- **"could not run `claude`"** — the CLI is not on the server
  process's PATH; set `CLAUDE_CODE_BIN` to the full path.
- **"--dangerously-skip-permissions cannot be used with root/sudo"**
  — `bypassPermissions` is refused when the server runs as root. Run
  the instance as a normal user (the right answer), or grant just the
  store tools instead: `CLAUDE_CODE_PERMISSION_MODE=` (empty) plus
  `CLAUDE_CODE_ARGS=--allowedTools mcp__newbound`.
- **"returned no JSON"** — usually a login prompt or usage-limit
  notice; the error includes stderr's tail, which says which. Run
  `claude` interactively once on the box to log in.
- **Timeouts on real agentic turns** — raise `CLAUDE_CODE_TIMEOUT`;
  a full-agent turn editing code legitimately takes minutes. The
  timeout path kills the whole process group, so no orphans
  accumulate either way.
- **Answers feel expensive** — check `CLAUDE_CODE_SYSTEM_MODE`; you
  are probably paying for the full Claude Code prompt on calls that
  only needed the oracle posture.
