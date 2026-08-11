# TODO: buildrust mod-file generation should regenerate, not append

**Status:** open — needs a session with `mraiser/flowlang` attached.
**Origin:** 2026-08-11 session (nebula nav-convention discussion surfaced it).
**When picked up:** deposit the durable claims below into `kb.platform-api`
via `agent-archivist-remember`; this file is the interim record because the
originating session had no newbound MCP attachment.

## The problem

`flowlang::builder` (verified against flowlang 0.3.31 source,
`src/builder/util.rs::update_mod_file_content`) maintains all generated
mod files **additively**: it inserts `pub mod X;` / `X::cmdinit(cmds);`
lines only if absent, and never removes anything. This applies to every
level of the chain, because all four use the same function:

1. control-level `mod.rs` (per-command `pub mod` + `cmds.push(...)`)
2. library-level `mod.rs` (per-control `pub mod` + `cmdinit` call)
3. crate `lib.rs` (per-library `pub mod`)
4. crate `cmdinit.rs` (per-library `use` + `cmdinit` call)

So deleting a command, control, or library from the store leaves its
generated Rust wired into the compile chain forever. Additionally,
`build_all` iterates libraries **from the store**, so a removed library's
crate source is never even visited again — nothing can prune it.

By contrast, `builder/initializer.rs::generate_main_initializer` already
regenerates the top-level initializer wholesale each build. That is the
correct pattern; the mod-file chain should match it.

## Evidence (2026-08-11 audit of mraiser/newbound)

- `data/flow` was deleted in `c9381d2`, but `newbound_core/src/flow/`
  (15 empty scaffolding stubs), `lib.rs`'s `pub mod flow;`, and
  `cmdinit.rs`'s `flow::cmdinit(cmds);` survived — committed, compiled,
  called at startup. Benign only because every flow stub was empty.
  Cleaned up manually on branch `claude/nebula-nav-ui-convention-xpucyk`
  (commit `9fb8347`).
- Full audit of registered commands: 122 `cmds.push` ids across
  app/dev/peer/security, **0 stale** (all have live store records). So
  no functional damage yet — but only because no rust-backed command has
  been deleted since generation. The failure mode is latent, not
  hypothetical: any store-side deletion of a rust command will keep the
  dead command compiled and registered in `RUST_COMMANDS`.
- Empty scaffolding dirs also accumulate: `ensure_control_scaffolding`
  creates a dir + stub `mod.rs` for **every** control (UI-only ones too),
  and undeclared stubs pile up on disk (27 orphan dirs in `src/app/`,
  27 in `src/dev/`, 3 in `src/peer/` as of this audit — all harmless
  empty stubs, never compiled, but noise that masks real problems).

## Proposed fix (in flowlang, not newbound)

Rebuild each mod file from the store's current state every build:

- control `mod.rs`: emit exactly the `pub mod` + `cmds.push` lines for
  commands that exist now, preserving nothing from the previous file.
- library `mod.rs`: same, from the current controls list; only scaffold
  dirs for controls that actually have rust commands.
- `lib.rs` / `cmdinit.rs`: regenerate the library list from the store's
  current library set (respecting each crate's root, since FFI libs
  live in their own crates). Hand-authored content in `lib.rs`
  (Initializer struct, `mirror`, API static) must be preserved or moved
  to a non-generated file first — this is the delicate part.
- optionally: delete (or at least warn about) source dirs under the
  generated tree that no longer correspond to any store record.

Constraint: newbound pins `flowlang = "0.3.31"` from crates.io, so the
fix lands as a flowlang release + version bump in newbound, and in the
external library crates (nebula, agent, kb, scratch) which pin the same.

---

## Also pending kb deposit (same reason: no MCP this session)

For `kb.doctrine` — **the nb-head navigation convention** (adopted
2026-08-11, implemented on `claude/nebula-nav-ui-convention-xpucyk` in
newbound + newbound_nebula): app titlebars open with a tiny home glyph
(`&#8962;`) linking to `../app/index.html` with `title` and `aria-label`
"All apps", followed by the app name as plain unlinked text
(`.nb-home` / `.nb-title`). Canonical markup: `app.ui_reference`;
shared roles: `app.ui` css; reference external consumer:
`nebula.nebula` (externals conform by copy, never by reference —
core references nothing outside newbound_core). The dev frame keeps
its own structure, aligned on href/tooltip only.
