# newbound-agent

The Newbound agent, as a library set you drop into a newbound checkout:

- `data/agent/` — the LLM plugin: providers and tool loop (`agent.llm`),
  the notebook and chat shells, the archivist's memory formation.
- `data/kb/` — the knowledge base: claims with provenance and source
  content hashes (staleness is detectable), doctrine, monthly rollups.
- `data/scratch/` — the agent's hot-swappable scratchpad (skeletal in
  git; see below).
- `agent/`, `kb/`, `scratch/` — the generated FFI dylib crates, tracked
  so the Rust is reviewable as ordinary source.
- `docs/` — the agent/process docs. **Read `docs/interim-process.md`
  first** — it is the working process this repo exists to serve.
- `tools/` — the smoke battery, disposable-instance recipe, validator,
  and pure check suites.

The agent is a plugin, optional by design. This repo is the durable home
of the harness; the platform lives in the newbound repo and knows nothing
about this one.

## Setup — one command

Check this repo out (github.com/mraiser/newbound-agent) next to a
newbound checkout, then:

```bash
path/to/newbound-agent/tools/setup.sh    # finds the sibling checkout;
                                         #   or pass its path explicitly
```

Idempotent: on a fresh clone it is the complete first-time sequence
(symlink overlay → `cmd/` scaffold if absent → host build →
`newbound rebuild` → host build with the FFI blocks → the three dylibs),
finishing with the git hygiene below and the overlay probe as proof. On
an already-set-up checkout every step short-circuits and the whole run
takes seconds. In a Claude Code on the web session the committed
SessionStart hook (`.claude/hooks/session-start.sh`) runs it
automatically at container start.

The overlay symlinks `data/agent`, `data/kb`, `data/scratch`, `agent/`,
`kb/`, `scratch/` into the checkout and marks the tracked scratch
skeleton files skip-worktree in this repo. Library discovery follows
symlinks (`read_dir` + `Path::is_dir()` — verified), so the platform,
`flowb`, the hot-reload watcher, and `newbound mcp` see ordinary
directories.

The git hygiene handles the two kinds of builder-written local state so
neither can land in an accidental commit:

- **newbound repo**: `newbound rebuild` rewrites `Cargo.toml` (the
  workspace exclude), `src/generated_initializer.rs`, and
  `newbound_core/src/api.rs`. Never committed there — the agent is
  optional by design — so setup marks them skip-worktree
  (undo: `git update-index --no-skip-worktree <file>`).
- **this repo**: the rebuild regenerates each FFI crate's `src/api.rs`
  against the libraries present in *that* checkout, deleting stubs for
  libraries installed elsewhere (`camera`, `hollis`, …). Setup reverts
  that environment-induced churn unless the file already carried edits
  before it ran; intentional regeneration gets committed together with
  its `data/` changes as usual.

### MCP attachment and the fallback driver

The checkout's `.mcp.json` attaches `./target/release/newbound mcp` to
the coding harness natively — but only if the binary exists when the
session starts; a session that begins on an unbuilt container loses the
native attachment for its lifetime even after building. The SessionStart
hook (or a CCR environment setup script running `tools/setup.sh`)
closes that gap for subsequent sessions on the same container. For a
session where attachment already failed, `tools/nb-call.py` drives the
same tool surface over stdin JSON-RPC:

```bash
tools/nb-call.py --list dev-code-              # discover
tools/nb-call.py dev-code-search_commands '{"lib":"","ctl":"","query":"memory"}'
```

## The scratch pattern

`data/scratch/` and `scratch/` are committed as a skeleton (meta.json,
empty controls index, the crate scaffold) and then gitignored wholesale.
Changes to the *tracked* skeleton files are silenced per clone with
`git update-index --skip-worktree` (overlay.sh does this). Scratch is
where the agent's transient evals and self-authored skills live — no
production code, and nothing it writes can poison a commit.

## Provenance

Carved from `alex89romanov/newbound` @ `796a598` (2026-08-09). File-level
git history stays in that repo; the store's own `_patches` journals inside
`data/agent`/`data/kb` carry the authoring history and travel with this
repo.
