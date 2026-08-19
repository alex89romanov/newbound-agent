# FFI dynamic loading — drop libloading/notify, load crates at hot-swap time

**Status:** in development — Phases 1–2 landed on the flow repo's
`claude/ffi-dynamic-loading-design-vfwdi8` branch (`flowlang::hotswap` in
`0659bca`, builder emission in `306d138`); Phases 3–5 open. Follow the
phases in order.
**Origin:** 2026-08-19 session (owner request).
**Scope:** `mraiser/flow` (flowlang — most of the work), `mraiser/newbound`
(dependency removal + `dev.code` command changes), `mraiser/newbound-agent`
(setup.sh, process docs, kb deposits).
**Rules that apply throughout:** branches always, owner merges; platform
command bodies (`newbound_core` src) are store-authored via `dev.code`
commands, never hand-edited; mutating experiments run against a disposable
instance (`tools/scratch-instance.md`).

## 1. Goals

1. **Remove `libloading`, `notify`, and `notify-debouncer-full` entirely.**
   All three are declared in `newbound/Cargo.toml:31-33` and referenced by
   exactly one artifact: the *generated* `src/generated_initializer.rs`
   (emitted by flowlang's builder when FFI crates exist). No committed
   `.rs` file in any of the three repos uses them. Their whole surface is
   `Library::new` + one `Symbol` lookup, and one debounced directory watch —
   trivially replaceable with `libc::dlopen`/`dlsym` (libc is already a
   flowlang dependency) and a std-only mtime poller.
2. **Load FFI library crates dynamically from store metadata**, so a brand
   new FFI library becomes live in a running instance — no initializer
   regeneration, no host rebuild, no restart. This flips the last "yes" row
   of the restart matrix in `docs/interim-process.md` to "no".
3. **Make the existing hot-reload path deterministic.** Today `dev.code
   compile` returns OK and the new code goes live up to ~2s *later*, when
   the debounced watcher fires. After this work, a successful compile of an
   FFI-rooted library reloads synchronously: OK means live.

Non-goals: changing how static (`newbound_core`-rooted) libraries load,
changing the `RUST_COMMANDS` fn-pointer registry, changing the
ndata mirror contract, actually *unloading* libraries (see §4).

## 2. Current mechanism (verified against source, 2026-08-19)

The load unit is the **crate root**: a library's `data/<lib>/meta.json`
declares `root` (crate directory) and `cargo.ffi` (bool) —
`flow/src/builder/util.rs:17` (`get_crate_info`). New libraries default to
FFI: `newbound_core/src/app/app/newlib.rs:64` writes
`{"crate_types":["dylib"],"dependencies":{},"ffi":true}`.

`newbound rebuild` → `flowlang::builder::build_all()`
(`flow/src/builder/mod.rs:23`) scans `data/`, scaffolds crates, and calls
`generate_main_initializer` (`flow/src/builder/initializer.rs:15`), which
writes the host's `src/generated_initializer.rs` containing, **when any FFI
crate exists**:

- a `#[repr(C)] Initializer { ndata_config, cmds: Vec<(String, Transform,
  String)> }` struct (textually duplicated into every FFI crate's `lib.rs`
  by `flow/src/builder/scaffolding.rs:31-60`);
- `FlowLangLibrary` — copies the dylib from
  `<root>/target/<profile>/lib<root>.so` to a timestamped temp path (to
  defeat dlopen's path/inode caching), `libloading::Library::new`s it,
  resolves `mirror_<root>`, calls it with the `Initializer`, and writes the
  returned `(id, Transform, io)` tuples into the global `RUST_COMMANDS`
  object as raw fn pointers cast to i64 (`flow/src/rustcmd.rs:22`);
- a `LIBRARYHEAP: GlobalSharedMutex<HashMap<String, FlowLangLibrary>>`
  registry and a `reload_library(lib)` entry point;
- a `hot_reloader` module with the FFI crate names **hardcoded as a vec
  literal** (`initializer.rs:172`), watching each
  `<root>/target/<profile>` via `notify` + `notify-debouncer-full` (2s
  debounce) and calling `reload_library` on change;
- one init block per crate: static crates call `<crate>::cmdinit`, FFI
  crates load via the above.

On the dylib side, `mirror_<root>` calls
`flowlang::mirror(("data", config))` under a `Once` — `DataStore::mirror`
(`flow/src/datastore.rs:40`) makes the dylib share the host's ndata heaps —
then runs the crate's `cmdinit` into the `Initializer`.

The runtime build loop: `dev.code compile`
(`newbound_core/src/dev/dev/compile.rs`) regenerates src from the store and
runs `cargo build` in the crate root; for FFI crates it does **not** reload
— it relies on the watcher noticing the new dylib. `dev.dev.activate_lib`
(FFI branch, `activate_lib.rs:104-114`) builds the crate **and the host**
and returns `"RESTART: <lib> roots a hot-reload crate - restart Newbound
once to activate it"`.

### Why a new library needs a restart

The FFI crate list is baked into `generated_initializer.rs` at host build
time — both the init blocks and the watcher's vec literal. A crate added
after that is invisible until regenerate + host rebuild + restart
(`docs/interim-process.md`, restart matrix row 4).

### Defects of record in the current design

1. **Unload UB window.** `FlowLangLibrary::reload` does
   `*self = Self::load(...)` — the old `Library` drops → `dlclose` — while
   `RUST_COMMANDS` still holds raw pointers into the old mapping until the
   rewrite completes, and other threads may be *executing* old transforms.
   Any command deleted from the new build keeps a permanently dangling
   pointer. This is the observed **crash on hot-swap of a library with
   code running in a thread**: dlclose unmaps the code segment under the
   running thread's instruction pointer, and the next fetch faults.
2. **Compile/reload race.** The 2s debounce means "compile OK" precedes
   "new code live"; an exec issued immediately after a compile can run the
   old code.
3. **Restart for new crates** (the headline problem).
4. **Duplicated ABI struct.** `Initializer` is generated textually into the
   host and into every crate's `lib.rs`; nothing enforces they stay in
   sync.
5. **Dead/vestigial code.** `flow/src/builder/loader.rs` is an unused,
   Linux-only dlopen seed with a latent bug (`CStr::from_bytes_with_nul` on
   a non-nul-terminated path always errors — it needs `CString::new`).
   `newbound/src/main copy.rs` + the optional `hot-lib-reloader` dependency
   and `reload` feature are leftovers of an abandoned approach.

## 3. Target design

### 3.1 New hand-written flowlang module: `flowlang::hotswap` (flow repo)

All FFI mechanics move out of generated code into a real module that ships
with flowlang. Nothing here is generated; it is ordinary reviewed source.

```rust
// flow/src/hotswap.rs — public surface (sketch)

/// The one canonical FFI handshake struct. Host and (newly generated)
/// crates both name this type; field order is ABI — never reorder.
#[repr(C)]
#[derive(Debug)]
pub struct Initializer {
    pub ndata_config: ndata::NDataConfig,
    pub cmds: Vec<(String, Transform, String)>,
}

/// Called once from the generated initializer. Stores the config,
/// loads every FFI crate the store declares, then starts the poller.
pub fn start(magic: (&'static str, ndata::NDataConfig));

/// Load (or first-load) one crate root. Copy dylib to a unique temp
/// path, dlopen, resolve mirror_<root>, register commands. Explicit
/// entry point for platform commands (activate_lib, compile).
pub fn load(root: &str) -> Result<(), String>;

/// Load a new generation of an already-loaded root; overwrite its
/// command registrations; deregister ids that vanished. The old
/// generation is intentionally never dlclosed (§4).
pub fn reload(root: &str) -> Result<(), String>;

/// Re-read data/*/meta.json and load any FFI root not yet loaded.
/// Called by the poller each tick and callable from platform code.
pub fn rescan();
```

Internals:

- **`DynLib` — the libloading replacement.** ~60 lines. Unix:
  `libc::dlopen(path, RTLD_NOW)` / `libc::dlsym` — libc is already in
  `flow/Cargo.toml:37`, so this adds **zero** dependencies. Windows:
  hand-declared `extern "system"` bindings to `LoadLibraryW` /
  `GetProcAddress` (no winapi crate). Start from
  `flow/src/builder/loader.rs` but: fix the `CString` bug, widen
  `cfg(target_os = "linux")` to `cfg(unix)` (macOS uses the same POSIX
  API), and **delete the `Drop` impl** — `DynLib` has no dlclose path at
  all (§4). Delete `builder/loader.rs` itself; the module supersedes it.
- **Registry.** `LIBRARYHEAP: GlobalSharedMutex<HashMap<String,
  LoadedLib>>` moves here from generated code (same ndata primitive it
  uses today). `LoadedLib { dynlib: DynLib, root: String, generation: u64,
  stat: (mtime, size), registered_ids: Vec<String> }`. The stored
  `NDataConfig` lives in a `std::sync::OnceLock`, set by `start`.
- **Store scan.** A library is FFI-rooted iff `meta.json` has a non-`.`
  `root` and `cargo.ffi == true`. Promote that reading to one shared place
  — e.g. `DataStore::lib_crate_info(&lib) -> (String, bool)` in
  `flow/src/datastore.rs` — and make `builder/util.rs::get_crate_info`
  delegate to it, so builder and loader can never disagree. Multiple
  libraries may share one root; the **root** is the load unit (dedupe,
  exactly as `build_all`'s `crates_to_wire` map does).
- **Temp-copy discipline (kept from today).** dlopen caches by path;
  loading a fresh copy per generation is what makes reload real. New:
  on unix, **unlink the temp file immediately after a successful dlopen**
  (the mapping holds its own reference), so nothing accumulates in
  temp_dir. On Windows a loaded DLL can't be deleted — name the copies
  predictably (`nb_ffi_<root>_<n>.dll`) and sweep stale ones in `start`.
- **The poller — the notify/debouncer replacement.** One std-only thread,
  ~2s tick (env-tunable, e.g. `NEWBOUND_FFI_POLL_MS`, `0` = disabled):
  each tick, `rescan()` the store (a `read_dir` of `data/` plus a
  `meta.json` read per library — single-digit files, negligible), then
  `stat` each known root's dylib at `<root>/target/<profile>/` with
  today's prefix/extension rules. Reload when the (mtime, size) pair
  differs from the loaded generation's **and** is unchanged since the
  previous tick — the two-tick quiesce replaces the 2s debounce and skips
  files cargo is still writing. A dylib appearing for a root that isn't
  loaded yet is a first load. This keeps the out-of-band path working:
  a bare `cargo build` in `agent/` (what setup.sh step 5 and terminal
  sessions do) still hot-loads into a running server, no notify crates
  involved.

### 3.2 Builder changes (flow repo)

`generate_main_initializer` (`flow/src/builder/initializer.rs`) shrinks to:

```rust
// This file is auto-generated by the flowlang build script. Do not edit.
use flowlang::rustcmd::{RustCmd, Transform};
use flowlang::datastore::DataStore;
use ndata::NDataConfig;
use newbound_core;
use cmd;

pub fn initialize_all_commands(magic: (&'static str, NDataConfig)) {
    // ... RUST_COMMANDS setup + one cmdinit block per STATIC crate,
    //     exactly as today ...
    flowlang::hotswap::start(magic);
}
```

- No `libloading`/`notify` imports, no `Initializer`/`FlowLangLibrary`/
  `LIBRARYHEAP`/`hot_reloader` emission, no per-FFI-crate init blocks, no
  hardcoded crate list. **The generated initializer becomes FFI-agnostic**:
  it changes only when the *static* crate set changes. The `hotswap::start`
  call is emitted unconditionally (it's a no-op scan when the store
  declares no FFI roots), so even "no sub-crates" projects like the flow
  repo itself stay uniform.
- Scaffolding (`flow/src/builder/scaffolding.rs`): new crates' `lib.rs`
  template uses `flowlang::hotswap::Initializer` instead of declaring a
  local copy. The `mirror_<root>` body is otherwise unchanged. Existing
  crates (`agent`, `kb`, `scratch`, …) keep their committed local struct —
  it is layout-identical, and `lib.rs` is only generated when absent — so
  no forced migration; regenerate opportunistically.
- `update_main_cargo_workspace_exclude` and the dependency wiring stay
  as-is (the exclude list is still required so cargo doesn't build FFI
  crates as workspace members).

### 3.3 Platform changes (newbound repo)

- **`Cargo.toml`:** delete `libloading`, `notify`, `notify-debouncer-full`;
  bump `flowlang` to the release carrying `hotswap` (0.3.34, §5).
  Optional cleanup rider (owner's call, separable): delete
  `src/main copy.rs`, the `hot-lib-reloader` optional dependency, and the
  `reload` feature — note `build_compile_command`
  (`compile.rs:295`) forwards `--features=reload` and generated crate
  manifests carry an empty `reload = []`, so removing the feature touches
  the `dev.code compile` command too; keeping it is harmless.
- **Committed `src/generated_initializer.rs`:** regenerated to the slim
  form above (static crates + `hotswap::start`). `src/main.rs` unchanged.
- **`dev.code` command changes** — these are `newbound_core`-rooted, so
  they are authored through the store (`upsert_command` → `compile`) and
  shipped with one final host rebuild + restart (the last restart of its
  kind):
  - `dev.dev.compile` (`compile.rs`): after a build that succeeded *and*
    advanced an artifact for an FFI-rooted library, call
    `flowlang::hotswap::reload(root)` (or `load` if not yet loaded)
    synchronously before returning OK. The poller remains the backstop for
    out-of-band builds; this call is what makes "OK means live" true.
  - `dev.dev.activate_lib` (`activate_lib.rs`): FFI branch keeps
    `build_all()` + `rebuild_rust_api()` + the manifest mending + the crate
    build, then calls `hotswap::load(root)` and returns OK/live. **Delete
    the host rebuild (lines 110-112) and the `RESTART:` sentinel** — that
    is the feature landing. (Its callers/UI that special-case the RESTART
    string must be swept — search the store for consumers.)
  - `app.app.newlib` (`newlib.rs`): structurally unchanged. A fresh
    library has no dylib until its first command compiles; the compile
    path (above) or the poller brings it live. Optionally call
    `hotswap::rescan()` at the end for tidiness.

### 3.4 Agent repo (newbound-agent)

- **`tools/setup.sh`:** step 4's markers (`grep 'Initialize crate: agent'`
  etc.) vanish — the new initializer names no FFI crates. The
  rebuild-then-rebuild-host dance is still needed **once per fresh
  clone**, but for different reasons (regenerating `api.rs`, the workspace
  exclude, crate scaffolds); re-key the check on something that still
  exists (e.g. `hotswap::start` present in the initializer *and*
  `agent/src/api.rs` regenerated). Step 5 (dylib builds) is unchanged and
  now hot-loads via the poller.
- **`docs/interim-process.md`:** restart matrix row 4 ("A new FFI crate")
  flips to **no** — and the verified-hot-path paragraph is rewritten
  around explicit reload + poller instead of the notify watcher.
- **kb deposits:** at completion, `agent-archivist-remember` the durable
  claims (new hotswap surface, the never-dlclose contract, the flipped
  restart matrix) with `subject` extras for `kb.platform-api`, and promote
  before pushing, per the session-end rules.

## 4. ABI and safety contract (read before coding)

- **Never dlclose. Ever.** `RUST_COMMANDS` holds raw fn pointers into
  loaded mappings indefinitely, and any thread may be mid-transform during
  a reload. Unloading is the one source of UB we cannot fence, so the
  design removes it: every loaded generation stays mapped for the life of
  the process. Cost: one dylib mapping (a few MB) leaked per reload,
  dev-time only, bounded by reload count. This *fixes* defect #1 — today's
  code dlcloses live libraries — including the known crash when a library
  is hot-swapped while one of its threads is running: the thread now
  finishes on the old, still-mapped code. Calibrate expectations, though:
  the surviving thread is **safe, not upgraded**. A thread spawned by an
  old generation keeps executing that generation's code and its
  crate-local statics until it exits (shared *data* is fine — the mirror
  handshake shares the host's ndata heaps across generations). A library
  that keeps a persistent worker thread needs a cooperative handoff to
  adopt new code: the old thread observes a generation bump and exits,
  and the new generation respawns it.
- **Stale-id sweep.** On reload, ids registered by the previous generation
  but absent from the new one are removed from `RUST_COMMANDS`, so a
  deleted command fails cleanly ("No such command") instead of silently
  running leaked old code. Ids still present are overwritten in place —
  in-flight calls through old pointers stay valid because nothing unmaps.
- **The handshake is not a C ABI.** `Initializer` carries `Vec`/`String`/
  fn pointers; the real contract is *same rustc, same flowlang and ndata
  versions, same profile and feature set on both sides*. That already
  holds (crates pin `flowlang = "=0.3.33"`, `ndata = "=0.3.17"`), but make
  it checkable: scaffold an additional
  `#[no_mangle] pub extern "C" fn nb_ffi_contract_<root>() -> *const c_char`
  returning `flowlang::hotswap::contract_ptr()` — each side's own flowlang
  copy computes its string, so they only agree when the copies agree. As
  implemented, the string carries the flowlang version, the profile, and
  the **layout sizes** of `Initializer` and `NDataConfig`
  (`flowlang=…;profile=…;ptr=…;init_size=…;cfg_size=…`): dependency
  versions aren't visible to `env!`, and comparing the handshake struct's
  actual layout is the stronger check anyway. The loader compares before
  calling `mirror_<root>`; a missing symbol is a legacy crate → warn and
  proceed; a mismatch → refuse the load with a clear error. Advisory in
  v1 (existing crates lack the symbol), strict once the overlay crates
  regenerate.
- **`Initializer` field order is ABI.** It now has one definition
  (`flowlang::hotswap`); document "never reorder/retype fields" on the
  struct. Existing crates' local copies must match it byte-for-byte until
  regenerated.
- The per-load `Once` around `flowlang::mirror` in each dylib is per
  *copy*, so mirror runs once per generation — identical to today's
  behavior, no change needed.

## 5. Rollout and version choreography

newbound consumes flowlang from crates.io (`flowlang = "0.3.33"`), and the
overlay crates pin `=0.3.33`. So:

1. Develop and test flowlang changes in the flow checkout.
2. To run the full stack before a release exists, point the newbound
   checkout at the local flow via `[patch.crates-io] flowlang = { path =
   "../flow" }` — this is builder-adjacent local state exactly like the
   workspace exclude; the checkout's `Cargo.toml` is already
   skip-worktree'd by setup.sh, and the patch line must never be
   committed. The overlay crates need the same patch in their manifests
   while testing (also never committed — revert before pushing, as
   setup.sh's hygiene step does for api.rs churn).
3. Owner publishes **flowlang 0.3.34**.
4. Bump: `newbound/Cargo.toml` → `flowlang = "0.3.34"`; `agent`, `kb`,
   `scratch` crate manifests → `=0.3.34`. New-crate scaffolds inherit
   automatically — the builder derives dependency lines from the root
   `Cargo.toml` (`builder/cargo.rs::get_core_dependency_lines`).

## 6. Work plan

**Phase 1 — flowlang runtime (flow repo).** ✅ **Landed** (flow branch
`claude/ffi-dynamic-loading-design-vfwdi8`, commit `0659bca`).
`src/hotswap.rs`: `DynLib` (unix + windows), registry, `Initializer`,
`start`/`load`/`reload`/`rescan`, a `loaded()` introspection helper,
poller (with failed stats not retried until they change), contract
check; `DataStore::lib_crate_info` + `datastore::crate_info_from_meta`
with `builder::util::get_crate_info` delegating; `builder/loader.rs`
deleted. Tests as specified, with the fixtures compiled by plain rustc
against the test build's own ndata rlib (same compiler, same ndata,
offline) instead of cargo-in-a-tempdir: unit coverage for quiesce/
contract/meta-parsing/register-sweep, a two-generation `DynLib`
never-unload test, and `tests/hotswap_e2e.rs` driving the full mirror
handshake (store discovery, execution through the shared heap,
deterministic reload with changed behavior, stale-id sweep, a
generation-1 transform pointer surviving a reload, contract refusal).
*Accepted:* `cargo test` green in flow (13 tests, 0 failures, no new
warnings); zero dependency changes in `flow/Cargo.toml`.

**Phase 2 — builder emission (flow repo).** ✅ **Landed** (flow branch,
commit `306d138`). `generate_main_initializer` emits static-crate blocks
plus one `hotswap::start(magic)` call — no loader/watcher plumbing, no
FFI crate names — factored into a pure, unit-tested
`render_main_initializer` (deterministic: static crates sorted). The
empty-project case emits the same shape, and flow's own committed
initializer is regenerated to it. The FFI scaffold imports the canonical
`flowlang::hotswap::Initializer` and exports `nb_ffi_contract_<crate>`
via `contract_ptr()`. *Accepted against a disposable overlaid instance
patched to local flowlang:* the regenerated initializer carried no
`libloading`/`notify` tokens and no FFI crate names; the host rebuilt;
all three overlay dylibs loaded at startup via store discovery (agent:
65 commands, legacy warn-and-proceed path); the poller reloaded a
touched dylib in a live server; a brand-new FFI library created via
`app-app-newlib` + rebuild + cargo build loaded with a contract **match**
— including first-appearing in a *running* server, no restart; and
`agent archivist queue_status` executed end-to-end via `exec`.

**Phase 3 — platform (newbound repo).** Dependency removal + flowlang
bump; regenerate the committed initializer; store-authored `dev.code`
changes (`compile` explicit reload, `activate_lib` live-load, optional
`newlib` rescan) with regenerated crate src committed together, per the
process. One host rebuild + restart ships it. *Accept:* §7 end-to-end
passes; `grep -rE 'libloading|notify' Cargo.toml Cargo.lock src/` finds
nothing (`Cargo.lock` proves the transitive cut). Expect the regenerated
initializer to **drop the `cmd` block**: no library in the store roots
there, and the generator (old and new alike) only emits crates that
store libraries declare — the manifest's `cmd` path dependency stays.

**Phase 4 — agent repo.** setup.sh re-keying, interim-process.md matrix
flip, kb deposits + promote. *Accept:* fresh-clone setup.sh run is green
end to end; docs match observed behavior.

**Phase 5 — release.** Owner publishes flowlang 0.3.34; pin bumps land in
newbound and the overlay crates; patch lines are gone from every manifest.

Phases 1–2 can proceed in one flow-repo branch; Phase 3 waits on them;
Phase 4 waits on 3. Nothing merges without the owner.

## 7. End-to-end acceptance test (disposable instance)

Scripted against a scratch instance (`tools/scratch-instance.md`), driven
over MCP / `tools/nb-call.py`:

1. Start the instance once. It stays up for the whole test.
2. `app-app-newlib lib:hotdemo` (FFI by default) — verify
   `data/hotdemo/meta.json` has `root: hotdemo`, `ffi: true`.
3. `dev-code-upsert_command` a rust command returning a marker string;
   `dev-code-compile` it.
4. Execute it (`exec` / MCP `tools/call`) **in the same process** —
   succeeds. *This is the feature: no restart happened.*
5. Edit the command body to a new marker; compile; execute — new marker,
   immediately (no sleep between compile-OK and exec: the synchronous
   reload guarantee).
6. Delete the command from the store; compile; execute — clean "no such
   command" error, not the old marker (stale-id sweep).
7. **Thread-survival (the historical crash).** Add a command that spawns
   a thread which loops for ~30s writing a heartbeat into the store, and
   invoke it. While the thread runs, edit and recompile the library
   (twice, for good measure). Verify: the process does not crash, the
   heartbeats continue uninterrupted (old code, still mapped), and a
   fresh invocation after the reload runs the new generation.
8. Out-of-band: run `cargo build --release` directly in `hotdemo/`
   after touching a src file; within ~2 poll ticks the reload log line
   appears (poller backstop).
9. Regression: rows 1–3 of the restart matrix unchanged — facet writes
   live-serve; an `agent`-lib command edit hot-reloads; a `dev.code`
   command edit still requires host rebuild + restart. Run the smoke
   battery (`tools/smoke`).

## 8. Risks and open questions

- **Windows loader path is best-effort.** No Windows CI exists; the
  `LoadLibraryW` path lands `cfg`-complete but untested. Current notify
  code was nominally cross-platform, so parity is kept, not improved.
- **Poller latency vs. watcher.** Worst-case pickup for *out-of-band*
  builds is ~2 ticks (~4s) vs. today's ~2s debounce — but the path that
  matters (`dev.code compile`) becomes synchronous, i.e. strictly better.
- **Leaked generations.** Deliberate (§4). If a long-lived production
  instance ever hot-reloads thousands of times, revisit with a
  quiescence-counted unload — out of scope now.
- **Struct drift before regeneration.** Until `agent`/`kb`/`scratch`
  regenerate their `lib.rs`, their local `Initializer` copies must stay
  layout-identical to the canonical one. The contract symbol turns a
  future drift into a load-time refusal instead of UB — prioritize
  regenerating the overlay crates soon after Phase 3.
- **`RESTART:` string consumers.** `activate_lib`'s sentinel may be
  pattern-matched by UI/flows; sweep the store for consumers before
  changing the return shape.
- **crates.io cadence.** Phase 5 needs the owner; until then every
  checkout runs on uncommitted `[patch.crates-io]` lines — the hygiene
  steps must keep them out of commits.
