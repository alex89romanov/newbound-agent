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

## Setup (symlink overlay)

Check this repo out next to a newbound checkout, then:

```bash
cd path/to/newbound
path/to/newbound-agent/tools/overlay.sh path/to/newbound-agent
```

The script symlinks `data/agent`, `data/kb`, `data/scratch`, `agent/`,
`kb/`, `scratch/` into the checkout and marks the tracked scratch
skeleton files skip-worktree in this repo. Library discovery follows
symlinks (`read_dir` + `Path::is_dir()` — verified), so the platform,
`flowb`, the hot-reload watcher, and `newbound mcp` see ordinary
directories.

Then build as usual:

```bash
cargo build --release --features=serde_support        # host
(cd agent && cargo build --release)                   # dylibs hot-load;
(cd kb && cargo build --release)                      #   no restart needed
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
