# The Interim Process — working on newbound until the agent takes over

**Status: reviewed — all §7 decisions made by the owner** (2026-08-09). This supersedes
the loop described in this repo's CLAUDE.md ("WHERE THE CODE LIVES",
2026-07-29). Facts marked **[verified]** were checked in source or on disk on
2026-08-09; everything else is proposal.

The goal it serves: the `agent` library eventually provides the LLM harness
*inside* newbound, enabling direct, complex edits from within the framework.
The interim process should converge on that — every session should leave the
in-newbound harness stronger, not run parallel to it.

---

## 0. What changed, in one paragraph

The old loop (hand-rolled Python JSON-RPC driver → canonical store → commit
`data/` → `export-bench.py` → second commit in a second repo for review →
disposable verify) was built around three constraints that no longer exist:

1. **MCP is a platform capability now.** `newbound mcp` dispatches to
   `flowlang::mcp` — the server ships in the flowlang crate (`flowmcp`),
   documented in its README, and the missing-param abort is fixed upstream as
   of flowlang 0.3.30 (wrappers validate declared params inside the panic
   guard). The Python glue and the local patch are both obsolete.
2. **The editing surface is on safe ground.** The write API is `dev.code`
   (43 commands), compiled **statically into `newbound_core`** — the
   "upserting onto the dylib the server is running from" hazard is gone for
   the editing commands themselves.
3. **The understanding-management layer is in-store.** `kb` (claims with
   provenance and source content hashes), the archivist
   (`log_turn`/`consolidate`/`queue_status`), and `dev.code.remember` exist.
   Knowledge no longer needs to accrete as markdown changelogs.

---

## 1. Repositories

### `newbound` (upstream, mraiser/newbound)
Canon for the platform: `newbound_core`, the static-rooted libraries
(`app`, `dev`, `peer`, `security`, `flow`), flowlang/ndata as crates.

**The push rule, as amended by the owner (2026-08-09):** on the owner's
own sessions and repos, work lands as **branches** — and **nothing merges
to master without his express permission**. (The old absolute
never-push-to-`mraiser/*` rule remains in force for Alex's sessions.)
A brand-new repo's *initial* push is the one exception: there is no
master to protect yet.

### `newbound-agent` (mraiser/newbound-agent — the durable home of the harness)
Contains everything that is the agent's, not the platform's:

- `data/agent/`, `data/kb/` — the store libraries (journals travel inside
  them automatically).
- `agent/`, `kb/` — the generated FFI crates, **tracked** so the Rust is
  reviewable as ordinary source (see §3).
- `data/scratch/` + `scratch/` — **skeletal only, on the `cmd` pattern**
  (below).
- `docs/` — the agent/process docs (this doc, agent design material).
  The platform-feature docs (scene-facet, flow3d, flowlang-format) are
  **offered upstream** into newbound's `docs/` via a mirror branch —
  split by subject (decided).
- `tools/` — the smoke battery, `scratch-instance.md`, the validator.
  The smokes stay here **for now**; revisit whether any move upstream
  once the new workflow has proven out (decided).

**The scratch pattern (decided, per the `cmd` precedent):** the skeleton is
committed once — `data/scratch/meta.json`, an empty controls index, and the
crate scaffold `flowb` would generate (`scratch/Cargo.toml`, `lib.rs`,
empty `cmdinit.rs`/`api.rs`) — so a fresh clone builds and hot-reloads with
no generation step. Then `data/scratch/`, `scratch/`, and `runtime/scratch/`
are gitignored **wholesale**, so normal use — transient code, skills,
experiments — never lands in a commit and can't poison upstream. One git
mechanic completes the owner's intent that changes to the *checked-in*
files also go untracked: ignore rules only hide untracked files, so the
skeleton files are additionally marked
`git update-index --skip-worktree` after the initial commit — after that,
even scratch usage rewriting the controls index stays invisible to status
and diff.

**Overlay mechanics — symlinks** (the owner's call, 2026-08-09; his
long-standing practice). The agent repo is checked out side by side, and
the newbound checkout carries symlinks into it: `data/agent`, `data/kb`,
`data/scratch` → `../newbound-agent/data/…`, plus `agent/`, `kb/`,
`scratch/` for the crates. Safe by construction: library discovery is
`read_dir` + `Path::is_dir()` (flowlang appserver), and `is_dir()` stats
*through* symlinks **[verified]**. Gitignore shenanigans are needed only in
the newbound repo, and even then only when editing from a working install
rather than a clean one.

### `alex89romanov/newbound` (the mirror) — de-forked, and shrinking further
With agent/kb/scratch moved out, the mirror stops being a long-lived fork.
Under the amended push rule its staging role shrinks again: sessions the
owner drives push platform-side branches **directly to upstream** for him
to merge. The mirror remains only for Alex-driven sessions (where the old
never-push rule still applies) and retires when it has no remaining job.

### `newbound-bench` — retired
Its three residual jobs are all covered: the review mirror is unnecessary
(§3), docs and smokes move to `newbound-agent`. Archive it as the
historical record it already mostly is. `export-bench.py`,
`install-bench.py`'s HTTP era, and `nbtransport.py` retire with it
(`install --mcp` survives only if seeding a fresh instance from repo files
is ever needed again; store-first makes that rare).

---

## 2. The session loop

**Setup** (once per environment):

1. Check out newbound (upstream or mirror branch) + overlay `newbound-agent`.
2. `cargo build --release --features=serde_support` at the root; build the
   `agent`/`kb` dylibs (`cargo build --release` in each; add
   `python_runtime` where needed).
3. Register the store as a native MCP server — `.mcp.json` in the checkout:

   ```json
   {
     "mcpServers": {
       "newbound": {
         "command": "./target/release/newbound",
         "args": ["mcp"]
       }
     }
   }
   ```

   Every store command is then a first-class tool call — schemas from
   `tools/list`, permission-gated by the coding harness, no driver scripts.
   This is the load-bearing alignment: the outer harness (Claude Code
   today) and the in-newbound notebook agent use the **same tool surface**,
   so every improvement to `dev.code`'s commands, descs, and errors
   improves both at once.

   **Tool exposure (decided):** the agent works from a **starter subset**
   of commands — the read family plus the journaled workhorses — and
   discovers the rest on demand (`search_commands` and describe-style
   lookup). Every command stays reachable; none but the starters are
   preloaded. This mirrors the tool model the bench notebook already
   established (default set + discovery meta-tools), so the two harnesses
   converge here too.

**Authoring** — writes go through platform commands, never through the
store's files (unchanged rule). Where the edit lands decides the rhythm:

| Target | Mechanism | Restart? **[verified]** |
|---|---|---|
| Facets (js/html/css/scene/flow/data) in any library | `dev.code` patch/write commands | never — served from the store |
| Rust commands in `agent`, `kb`, `scratch` | `upsert_command` → `compile` | **no** — the watcher hot-reloads the dylib |
| Rust commands in `newbound_core`-rooted libs (`app`, `dev`, `peer`, `security`, `flow`), incl. `dev.code` itself | upsert → compile → host rebuild | **yes** — static rlib, restart the process |
| A **new** FFI crate (new root) | `newbound rebuild` regenerates the initializer | **yes** — watcher list + init blocks are baked in at rebuild time |

The hot path is verified end to end: `initialize_all_commands` arms the
watcher before the `mcp` subcommand dispatches; the watcher observes
`{agent,kb,scratch}/target/release`, reloads a changed dylib from a fresh
temp copy (first appearance handled too), and rewrites `RUST_COMMANDS`;
flowlang's `tools/list` walks the store fresh per request and `tools/call`
does `Command::lookup` per call. New commands appear and new code runs
inside one long-running `newbound mcp` process.

Disciplines that ride along: fill `desc` at creation (`tools/list` omits
blank-desc commands — desc *is* discovery, and it is also the curation
lever for what the model sees); mutating work runs against disposable
instances only; no production code in scratch.

**Verification** — unchanged and proven: disposable instance from a tar
copy of the checkout, the smoke battery, the validator. Runs in-sandbox
without the owner's time; his hardware stays necessary only for live
promotion, real peers, and GPU measurements.

**Deliverable** — one commit per unit of work, carrying `data/` changes
**and** the regenerated crate src together (the compile step produces both;
committing them together is the drift guard). Commit messages name the
touched `lib.ctl` pairs. The owner pulls `newbound-agent` (agent-side work)
or a short-lived mirror branch (platform-side work).

---

## 3. Review without export

The export mirror is dead. Review happens directly on the repo, because the
store was never opaque in *content*, only in *addressing*:

- **Rust** is generated in full under each crate's `src/`
  (`agent/src/agent/<ctl>/<cmd>.rs`) — ordinary reviewable source, tracked.
- **Facets** are plain sibling files with real extensions on disk
  (`<id>.js`, `<id>.html`, `<id>.css`, `<id>.flow`, `<id>.memory`,
  `_patches`) **[verified]** — `git diff data/` already shows full readable
  content.
- **Records** are single-line JSON — one `.gitattributes` textconv entry
  (pretty-print) makes those diffs readable.
- **id → name**: a shard path doesn't say "dev.shelf's js facet". A tiny
  lookup helper (or simply naming touched controls in the commit message)
  closes the last gap. If it ever deserves more, the `_patches` journals
  already record old/new for every write — a `changes_since` command
  rendering a digest from them is the platform-native version, per "never
  waste LLM cycles on something derivable procedurally".

---

## 4. Context and memory

- **Thin primer.** CLAUDE.md shrinks to a page: hard rules, this loop,
  pointers. The 60KB session chronicle it replaced is history, not primer;
  what in it still matters gets distilled into `kb` claims (with sources
  and hashes, so staleness is detectable) and monthly rollups
  (`kb.m2026-07` is already the shape).
- **Procedural brief.** A `kb` command assembles doctrine + workflow +
  staleness-checked claims into one markdown pack. The recall-layer work
  already injects a pack into `chat_llm`'s system message; the *same* pack
  is what a coding session reads at start. One brain, two mouths.
- **Sessions deposit back.** `remember` and `log_turn` are ordinary MCP
  tools now. Findings and doctrine-grade corrections land in `kb` *during*
  the session; turns worth keeping go through `log_turn` so the archivist
  consolidates them regardless of which harness drove. This is the step
  that makes the interim *accumulate toward* the in-newbound agent instead
  of running parallel to it: when the notebook agent takes over more work,
  it inherits the memory the interim built.

---

## 5. The ramp

Framed as capabilities, the seam is further along than the old docs
suggest:

| Capability | Where it lives |
|---|---|
| Tool surface (`dev.code`, MCP) | **in-platform** ✔ |
| Context assembly (kb / syspack) | **in-platform** ✔ |
| Memory formation (archivist, `remember`) | **in-platform** ✔ |
| Loop driver (planning, multi-step, judgment) | outer harness, for now |
| Execution environment (cargo, browsers, disposables) | outer harness, for now |
| Model quality | chat_llm → local model; frontier via the outer harness |

Interim division of labor: the outer harness drives complex multi-step work
*through* the platform's own tools and deposits into the platform's own
memory; the notebook agent handles in-app asks and grows one loop
capability at a time (the archivist's queue is a natural seed for
task-shaped work). When stronger models are wanted inside the notebook,
`chat_llm`'s provider boundary is the place — an OpenAI-compatible endpoint
or a thin bridge both fit behind it without changing the harness around it.

---

## 6. Retirements and syncs (housekeeping, not yet done)

- Retire: `export-bench.py`, `nbtransport.py`, the HTTP era of
  `install-bench.py`, `patches/flowlang-missing-param-abort.patch` (fixed
  upstream 0.3.30+), the bench CLAUDE.md chronicle (distill → kb).
- Sync: the mirror to upstream HEAD; crates to flowlang 0.3.31 /
  ndata 0.3.17 (0.3.17 fixes the JSON parser rejecting negative numbers —
  flow diagrams with nodes at negative coordinates depend on it). **Not
  cosmetic [verified]**: on the mirror's 0.3.28, a missing declared param
  on a static (newbound_core) command still panics uncaught and kills the
  `newbound mcp` process — the overlay probe demonstrated it live; the
  0.3.30+ builder's wrapper guard is the fix, which upstream's rebuild
  already carries.

**Executed and verified 2026-08-09** (this session): the agent repo was
carved (agent + kb + skeletal scratch + docs + tools, 178 files), the
mirror-side removal staged on `claude/newbound-workflow-review-eotm2p`
with `.mcp.json` added, and the symlink overlay proven end to end —
`tools/overlay.sh` linked six paths, host + all three dylibs built through
the links, and `tools/overlay-probe.py` passed 7/7 against `newbound mcp`:
libraries discovered through symlinks (129 tools incl. all 43 `dev-code-*`),
a static command executed a real store read, and the agent FFI dylib
dispatched through the overlay. (kb lists no tools by design — it is a
data-only library.)

---

## 7. Decisions (all made by the owner, 2026-08-09)

1. **Overlay: symlinks** — side-by-side agent repo checkout, symlinked
   into the newbound tree; enumeration verified symlink-safe (§1).
2. **Scratch skeleton**: meta.json + empty controls index + crate
   scaffold committed once, then `data/scratch/`, `scratch/`, and
   `runtime/scratch/` gitignored wholesale, with skip-worktree on the
   tracked files so even their runtime mutations go untracked (§1).
3. **Docs: split by subject** — platform-feature docs offered upstream
   via a mirror branch; agent/process docs live in the agent repo (§1).
4. **Smokes: stay in the agent repo for now** — revisit once the new
   workflow is established and proven (§1).
5. **MCP gating: status quo for the interim** — no server-side
   allowlist; curation is blank-desc omission plus the harness's
   per-call permissioning plus the disposable-instance rule. The agent
   works from a starter subset of commands with search/discovery of the
   rest (§2).
