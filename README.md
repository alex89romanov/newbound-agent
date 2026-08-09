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

Check this repo out (github.com/mraiser/newbound-agent) next to a newbound checkout, then:

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

Then build. On a checkout whose generated initializer doesn't yet know
these FFI crates (upstream master — it has no agent blocks and no
hot-reload watcher until regenerated), the first build is a three-step:

```bash
tools/gen-cmd-crate.py .                       # only if cmd/ is absent
cargo build --release --features=serde_support # host, once
./target/release/newbound rebuild              # regenerate initializer
                                               #   (now sees agent/kb/scratch)
cargo build --release --features=serde_support # host again, with FFI blocks
(cd agent && cargo build --release --features=serde_support,python_runtime)
(cd kb && cargo build --release)               # dylibs hot-load from here on;
(cd scratch && cargo build --release)          #   no restart needed
```

Do not commit the regenerated initializer to the newbound repo — the
agent is optional there by design; the regeneration is local state, like
the build itself. On a checkout that already carries the FFI blocks (the
old mirror), the plain host + dylib builds suffice.

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
