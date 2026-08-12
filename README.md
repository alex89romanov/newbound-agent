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
`newbound rebuild` → host build with the FFI blocks → the three dylibs
→ agent-app staging), finishing with the git hygiene below and the
overlay probe as proof. On an already-set-up checkout every step
short-circuits and the whole run takes seconds. Afterwards
`./target/release/newbound` serves the web UI (the agent app at
`/agent/index.html`, port per `config.properties`) and
`./target/release/newbound mcp` serves the store to a coding harness.

The app staging is the piece of the platform's `install_lib` step the
overlay doesn't cover: the server serves only apps listed in
`config.properties`, and the app shell must exist under `runtime/agent`.
Setup copies `data/agent/_APPS/agent` there, excludes it via the
per-clone `.git/info/exclude` (the platform's tracked `.gitignore`
doesn't know the agent), and ensures `agent` is in the `apps` list —
creating `config.properties` from the example when absent, appending to
the list when present. It never touches other keys; note that a bare
`newbound mcp` run auto-creates the file with `http_port=0`, which setup
flags but deliberately leaves alone.

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
native attachment for its lifetime even after building.

Getting the binary built *before* session start: the reliable way on
Claude Code on the web is the **environment's setup script** — point it
at `tools/setup.sh`. This repo also carries a SessionStart hook
(`.claude/hooks/session-start.sh`), but repo-level hooks only load when
this repo is the session's project directory: in a two-repo session
(newbound + newbound-agent side by side — the normal layout) the
project directory is their parent and the hook does **not** fire
(verified 2026-08-12).

For a session where attachment already failed, `tools/nb-call.py`
drives the same tool surface over stdin JSON-RPC:

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
