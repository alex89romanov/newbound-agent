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

## Session start

1. Overlay + build: see README (fresh checkout = the three-step first
   build; after that, dylibs hot-reload and only `newbound_core`-rooted
   Rust needs a host rebuild + restart).
2. The platform checkout's `.mcp.json` attaches `newbound mcp` — every
   store command is a native tool, named `lib-control-command`.
3. Orient from the store, not from chronicles: the kb library's memory
   facets carry doctrine (`kb.doctrine`), the working process
   (`kb.workflow`), and platform facts (`kb.platform-api`) — read them
   early; they are claims with provenance and staleness hashes.
4. Work from the starter command subset; discover the rest with
   `dev-code-search_commands`. `desc` is discovery — fill it on
   everything you author.

## Session end

- Deposit what you learned: `agent-archivist-remember` for durable claims
  (domain = the kb control it belongs to). A lesson left only in chat is
  a lesson lost.
- Commit `data/` changes and regenerated crate src **together**, on a
  branch, and push. The `_patches` journals inside `data/` are the
  authoring history and travel with the commit.
