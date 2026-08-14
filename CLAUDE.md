# newbound-agent — Session Primer

The Newbound agent as a library set: `agent` (the LLM plugin), `kb` (the
knowledge base), skeletal `scratch`, docs, and harness tools. It overlays
a `mraiser/newbound` checkout via symlinks. The full process is
**`docs/interim-process.md`** — read it once; this page is the recap.

## Hard rules

- **Branches always; nothing merges to master without the owner's
  express permission** (his rule, 2026-08-09). Applies to this repo and
  the platform repo alike.
- **Writes go through platform commands, never through the store's
  files.** `data/*` is a content-addressed store; edit it via the
  `dev.code` commands over MCP. Reading files is fine.
- **Every declared param must be passed** on every command call — there
  are no optional parameters.
- **Mutating experiments run against a disposable copy**
  (`tools/scratch-instance.md`), never a live instance.
- **No production code in `scratch`**, and nothing under `data/scratch/`,
  `scratch/`, or `runtime/` is ever committed (skeleton excepted,
  already handled by overlay.sh's skip-worktree).
- **The brain is instance-owned: nothing under `data/kb/` is ever
  committed** (docs/one-memory-cycle.md A4 — tracked files are frozen,
  overlay.sh skip-worktrees them, .gitignore hides the rest). Memory
  reaches git only through the two curated channels: the subject
  libraries' shipped manuals (`agent-archivist-promote`) and the primer
  (`docs/kb-seed.json`, refreshed only by a deliberate
  `agent-archivist-seed_export`).

## Session start

1. Overlay + build + app staging: `tools/setup.sh` — one idempotent
   command (see README for what it does; repo-level SessionStart hooks
   don't fire in two-repo web sessions, so run it if the binary is
   missing). After the first build, dylibs hot-reload and only
   `newbound_core`-rooted Rust needs a host rebuild + restart.
2. The platform checkout's `.mcp.json` attaches `newbound mcp` — every
   store command is a native tool, named `lib-control-command`. If the
   binary didn't exist when the session started, that attachment is
   lost for the session — `tools/nb-call.py` drives the same surface.
3. Orient from the store, not from chronicles: memory is federated
   (docs/one-memory-cycle.md). The brain (`kb`) carries doctrine
   (`kb.doctrine`) and the working process (`kb.workflow`); every
   library's controls carry their own manuals as memory facets, listed
   lib.ctl in the memory index. On a fresh clone the brain starts from
   the frozen snapshot — top it up with
   `agent-archivist-bootstrap path:docs/kb-seed.json` (idempotent;
   setup.sh does this when it can).
4. Work from the starter command subset; discover the rest with
   `dev-code-search_commands`. `desc` is discovery — fill it on
   everything you author.

## Session end

- Deposit what you learned: `agent-archivist-remember` for durable
  claims. Library-subject claims go straight onto the subject control
  (`lib:<subject>`), or into the brain with a `subject` extra
  (e.g. `"subject": "dev.code"`) for later promotion. A lesson left
  only in chat is a lesson lost.
- **Promote before you push**: `agent-archivist-promote lib:<lib>` moves
  the brain's subject-bearing claims onto the shipped manuals —
  publishing warns about anything you leave behind, but never promotes
  on its own.
- Commit **manuals + regenerated crate src together**, on a branch, and
  push — in whichever repos the touched libraries live. The brain
  (`data/kb/`) never rides a commit; refresh `docs/kb-seed.json` via
  `seed_export` only when doctrine/process-grade material changed, and
  review that diff like a docs change.
