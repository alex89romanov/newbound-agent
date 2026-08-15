# The One-Memory Cycle — fold, federate, seed, cut loose

**Status: decisions made by the owner, 2026-08-14** — this revision
records his answers to the original proposal's open calls; §0 and §7 are
now decisions, not questions. Hard rules unchanged: branches always,
writes through platform commands, mutating tests on disposable
instances, nothing merges without the owner's express permission.

The unifying theme, in the owner's words (still being workshopped):
*cultivating, refining, and growing an actionable self-referential
understanding over time.* Working shorthand in this doc: **understanding
that compounds** — perceive → claim → adjudicate → embody, each turn
making the next one cheaper.

---

## 0. The five decisions (as made)

1. **Executive function blends into the agent library; jerry is parked
   as a donor.** The jerry repo stays a separate, unused library;
   relevant parts migrate into `agent` incrementally. The name "jerry"
   is dropped — cognition and executive function are both "agent."
   Two safeguards ride the decision:
   - The executive lands as **its own control** (`agent.executive`),
     not smeared across `agent.llm`/`agent.archivist`. Controls are the
     modules; amputation lines stay legible; the working archivist/LLM
     plumbing keeps its blast shield.
   - **The loop never autostarts.** Explicit start, killable,
     observable (the spawn/drive lesson, already paid for). Running
     cognition-services-only remains a runtime choice.
   The migration pattern is thus uniform: hollis's vestigial cortex
   strangler-figs *out* of the sensor, jerry's executive strangler-figs
   *in* to the agent, both toward the same library.
2. **Hollis stays external as the reference sensor plugin** (camera to
   follow as the second). Its distribution path is proven
   (`dev.github.import` → activate → hot-load); nothing is amputated
   before its replacement demonstrably works.
3. **Memory federates, and the manual surface belongs to the
   platform.** Any control can carry a memory facet — its *manual* —
   filed by the eponymous-control convention (claims about a control on
   that control; claims about a library on its same-named primary
   control). The machinery splits:
   - **flowlang owns the read surface** (its own release cadence, not
     gating this cycle): "memory" as a first-class documented
     attachment; manual presence/counts in `lib_info` and the generated
     API reference; flowmcp exposing manuals as MCP *resources*, so any
     MCP client — not just our agent — can read a control's accumulated
     manual. A facet on a platform control is thereby platform
     documentation, not agent residue: desc (what it is), params (how
     to call it), manual (what we've learned about it).
   - **Agent owns the write path**: `remember` validation, provenance
     stamping, `promote`, consolidation. Readers are universal; writers
     are governed.
4. **kb becomes instance-owned** — excluded from git on the scratch
   pattern — with the three-tier shape of §2: brain / primer / manuals.
5. **The perception contract ships with agent** (this repo's docs now;
   eventually it is simply part of the agent library's own manual).
   The **first sensor is built in**: the resident codebase (store
   journals, then filesystem/builds/peers — the agent's first
   experience is itself). **Hollis is the first official plugin**
   (ears, the reference plugin), camera close behind (eyes). The
   paradigm must be exceptional at code first and expand to all
   non-code environmental input — situational awareness is bounded by
   the sensor contract, not the codebase. (Owner's clarification,
   2026-08-15; contract clauses in `understandingloop.md` commitment 2.)

Why these travel together: continuous executive deposits make kb-in-git
untenable (machine-cadence appends cannot ride branches-always review,
and concurrent instances become a merge-conflict factory — the
2026-08-13 kb divergence between the camera and hollis sessions was the
mild preview). Federation is what makes instance-owned kb *survivable*:
it gives most durable knowledge a git home outside kb. The seed covers
the remainder that must outlive any single instance. The repo boundary,
the git boundary, and the audience boundary become the same line.

---

## 1. The boundary sentences

One sentence per component; anything in a component that belongs to
another sentence is future amputation.

| Component | Job |
|---|---|
| **hollis** (external plugin) | Perceive acoustically: emit typed perceptions; own its sensor state (voiceprints, calibration, geometry). |
| **camera** (external plugin, later) | Perceive visually: same contract. |
| **agent** (this repo) | Cognition and executive function: LLM access, the archivist (remember / consolidate / promote), recall packs, the OODA loop as `agent.executive` — claims in, claims out, acts only through platform commands — and the built-in codebase sensor family (store journals first). |
| **kb** (this repo, instance-owned) | The one memory: claims with provenance, confidence, staleness. |
| **flowlang** (platform crate) | The manual surface: memory facets as first-class control documentation, indexed and MCP-exposed. |
| **nanochat** (artifacts, never committed) | Embody the memory: the salience tier, trained from adjudicated claims, gated by held-out QA. |
| **jerry** (parked donor repo) | Nothing runs from here; code migrates into `agent.executive` piecemeal and dies here. |

Sensor-state vs. claim is the load-bearing distinction hollis already
demonstrates: the *voiceprint* is sensor-owned perceptual state; the
*binding* of that voiceprint to "Marc" is a claim in the one memory,
with provenance and confidence. Every future sensor follows this split.

---

## 2. The three memory tiers

### The brain — instance kb
The live `data/kb`, gitignored wholesale on the scratch pattern
(skeleton meta.json + controls index committed once, skip-worktree'd;
everything else untracked). Deposits — session, executive, archivist —
land here freely at any cadence. `_patches` journals still accumulate,
so provenance survives locally; it simply stops being git's problem.
Two instances having divergent brains is not a bug to merge away; it is
the nature of separate experience. Sharing happens through promotion
or, eventually, P2P — never through git merges of raw memory.

Backup posture: the brain is runtime state, backed up like runtime
state. The seed-export command (§3) doubles as the backup tool.

### The primer — the seed
A single curated JSON under `docs/` (owner's call): doctrine (the
owner's voice), the working process, and whatever else a fresh instance
must know before its first act. Updated **only** by a deliberate export
command, reviewed like a docs change; it never churns because it only
changes on purpose. A fresh instance bootstraps its brain from the
primer (overlay.sh or an archivist bootstrap command) and diverges
freely. Audience: the primer inherits the repo's privacy — it lives in
newbound-agent precisely because doctrine never rides a public library.

### The manuals — federated library facets
Memory facets on the libraries the knowledge is about, filed by the
eponymous-control convention, shipped through every existing channel
(git, `dev.github.import`, P2P `install_lib`). A library arrives
carrying its own accumulated manual; the claims about `dev.code`
version in the same diff as `dev.code` itself, and a source-hash
mismatch in that diff is a self-flagging stale claim. This is where the
bulk of session learnings durably live — which is what makes the brain
safe to cut loose from git, including for ephemeral (CCR) sessions
whose containers are destroyed: what a session learns about a library
survives in that library's repo; only episodic residue dies with the
container, and episodic residue was never worth a merge conflict.

---

## 3. New and changed machinery

Small, and mostly already known:

- **Index scan** (`agent/src/agent/llm/tool_loop.rs:144`): enumerate all
  libraries → all controls → include any control with a memory
  attachment, labeled `lib.ctl`. kb's controls appear automatically,
  unchanged. Built agent-side first (no crate-release dependency); the
  flowlang-native surface (decision §0.3) subsumes it in a later
  flowlang release, at which point the agent's scan simplifies to a
  platform query.
- **Consolidate** (`agent/src/agent/archivist/consolidate.rs:87,166`):
  same enumeration for the known-claims pack; filing passes the claim's
  subject lib instead of the `"kb"` literal. The pack gets per-domain
  caps / tag filtering **in this cycle, not later** — a continuously
  ticking Orient reading an unbounded store-wide pack is a cost
  explosion scheduled in advance.
- **Audience guard in `remember`** — **hard refuse** (owner's call):
  deposits of private-class claims (doctrine-tagged, owner-voice) into
  any library with a non-empty/anonymous readers list are rejected, not
  warned. An autonomous depositor should hit walls, not advisories.
- **`promote`** (new archivist command): sweep the brain for claims
  whose subject is a given library — procedural under federation
  addressing, the claim's `lib.ctl` domain *is* the extraction key —
  **union-merge by claim identity** into the library's shipped facet,
  mark the working copies promoted. The library's next commit carries
  them, reviewably. Trigger is **explicit, with a warn-if-unpromoted
  check on publish** (owner's call): publish never promotes on its own,
  but tells you what you're leaving behind — `git status`'s
  relationship to `git push`. The union-by-identity merge is the same
  machinery the eventual `install_lib` clobber fix needs; build once,
  use twice. Promote also answers "autonomous deposits meet
  branches-always": the executive writes freely into the brain, and
  nothing it produces ever touches git without a deliberate promote.
- **`seed_export` / `bootstrap`** (new archivist commands): export the
  curated primer from the brain (deliberate, reviewed); populate an
  absent brain from the primer on first run.
- **Session-end ritual changes** (CLAUDE.md): deposit → **promote** →
  commit manuals + any primer refresh. The brain stays local. The
  "commit data/ changes and push" instruction stops applying to kb.

Explicitly **not** in this cycle: P2P union-merge in `install_lib` and
memory-aware version bumps (noted, deferred — git remains the channel);
the flowlang read surface (its own release, follows the cycle); any
executive migration beyond the Phase-0-grade skeleton (this cycle
builds the ground the executive stands on).

---

## 4. Phases

Each phase lands on a branch, is verified on a disposable instance, and
is useful even if the next never happens. Track A (memory) and Track B
(executive + contract) are independent until Phase A4.

### Track A — the memory work

**A1 — Federation MVP.** The two enumeration loops + the hard-refuse
audience guard + consolidate's caps. *Verify (disposable):* seed a
memory facet on a platform control via `remember lib:"dev"`; the index
lists `dev.<ctl>`; consolidate files a dev-subject claim onto dev; a
doctrine-tagged deposit aimed at dev is refused. kb behavior unchanged

*[REVERSED 2026-08-15 (owner): the audience guard is removed —
claude/sovereign-instances. Local instances are sovereign: anyone can
edit their copy's code, so anyone can edit their copy's beliefs,
doctrine tags included, and publish their fork to their own peers.
Gating local writes on the instance's readers ACL policed the wrong
layer ("are we the local instance police? No thank you"). Curation
happens at the credentialed exits — repo push rights, crates.io
tokens, the owner's branch-diff review of promote/seed/package
changes — never at the local store. The tags remain as classification
for those reviews; nothing refuses them at write time.]*
throughout.

**A2 — Migration.** Disperse the brain's library-subject claims onto
their subject controls via `remember` (journaled, reviewable): the
platform-api claims → the newbound repo in **one batch** (owner's
call); hollis claims → the hollis repo; crate-subject claims (flowlang,
ndata) → **flow's primary control** as nearest kin (owner's call #3
direction — flow-resident documentation); process/doctrine/episodic
claims stay in the brain. *Verify:* index shows the new homes; recall
pack unchanged in content.

**A3 — promote + seed_export + bootstrap**, including the
warn-on-publish check. *Verify (disposable):* deposit a lib-subject
claim → promote → shipped facet gains it, re-promote is a no-op
(identity dedupe); publish with unpromoted changes warns; wipe
`data/kb` → bootstrap → doctrine present, brain functional.

**A4 — Cut kb loose.** Gitignore `data/kb` wholesale, skip-worktree the
tracked kb files (overlay.sh, per clone), commit the first primer
(`docs/kb-seed.json`), update CLAUDE.md's session ritual and setup.sh
(idempotent brain top-up from the primer). *Verify:* fresh clone →
frozen brain snapshot present, new deposits invisible to `git status`;
bootstrap idempotence proven at A3.

*Executed 2026-08-14 with one deliberate deviation: freeze, don't
delete.* The tracked kb files stay tracked at their A4 state forever
instead of being removed from tracking — a committed deletion would
delete (or modify/delete-conflict) every existing clone's live brain on
pull. Freezing changes nothing on pull, hands fresh clones a working
snapshot, and the ignore + skip-worktree pair still guarantees no
deposit ever rides a commit. The snapshot ages harmlessly: the primer
and the manuals are the living channels, and bootstrap tops a stale
snapshot up. One known cost: git checkout across branches whose frozen
kb states differ refuses while the live brain has drifted — use a
worktree for cross-branch work in this repo.

### Track B — the executive and the contract

**B1 — Perception contract doc** (`docs/perception-contract.md`, shipped
with agent). The typed perception schema: a modality-agnostic envelope
`{kind, timestamp, sensor, payload, touched-claims}` with `text_input`,
`store_change`, `file_change`, `peer_event` as built-in kinds and —
widened from the donor repo's `{millis, sink}` STT shape —
`acoustic_event` carrying hollis's annotations (speaker entity,
location, ambience/state deltas), `visual_event` reserved for camera.
Two acceptance criteria (owner's sensor clarification): the
**zero-executive-change test** (a new modality is a payload kind plus a
binding function, nothing else) and **per-sensor binding** (perceptions
arrive attached to the claims they touch: staleness-hash join for code,
voiceprint resolution for hollis). The built-in codebase sensor's
journal tailer is the reference implementation; hollis is the reference
plugin — its `EventKind` enum is ~80% of the contract already; its dead
variants are the contract waiting for a counterparty.

*Executed 2026-08-15:* `docs/perception-contract.md` v1 — envelope,
per-sensor binding, delivery/coalescing rules, the kind registry
(four built-in kinds, `acoustic_event` with the full hollis
`EventKind` mapping, `visual_event` reserved), reference
implementations, and the three acceptance criteria.

**B2 — `agent.executive` control is born.** The donor repo's Phase-0
hygiene applied on entry: no synchronous bootstrap-training anywhere
near init, killable loop, explicit start, current-phase observable in
state. Heavy-tail discipline: query-time embeddings (no fastembed in
the hot path), model artifacts fetched-at-install and gitignored like
hollis's models. Training architecture is the amended
`understandingloop.md` commitment 5 (owner's correction, 2026-08-15):
the **base grows** — a resident online-learning service trains it
continuously (replay-buffered mixed batches: fresh curated tokens +
reservoir + standard nanochat data), serving double-buffered with split
pointers (salience live, user-facing gated) and a gate-behind eval
harness with auto-rollback — while **personality/chat skill is a LoRA**
re-derived over the moving base when its regression probe slips. A
model trained on brain/world data inherits the brain's privacy class in
its entirety — training outputs never ride a repo push.
*Verify:* overlay + build on a clean disposable; the executive starts
on command, idles, stops on command; no new heavy deps in the default
build; `agent.llm`/`agent.archivist` behavior untouched.

*Executed 2026-08-15* (claude/one-memory-b2-executive): the
`agent.executive` control with start (explicit, killable, never
autostarts) / stop / status (phase, queue depth, counters) / perceive
(the contract's sink - loud on shape, tolerant on vocabulary).
Skeleton only: drains and accounts the queue; no LLM, no acts, no new
deps. 14/14 on a disposable. The donor's machinery migrates into this
control phase by phase from here.

### Sequencing rationale

A1 before A2 (migration needs the addressing); A2 before A4 (nothing
loses its git home before its new home exists — **federate first, then
seed, then cut loose**); A4 before the executive's deposit loop ever
runs (the machine-cadence churn problem should never exist for even a
day). B1 anytime; B2 after A1 if convenient (later executive phases
read the federated index, but the skeleton touches none of it).

---

## 5. What falls out for free

- The repo boundary **is** the audience boundary: private brain repo
  (agent, kb) vs. public plugin repos (hollis, camera) carrying only
  their own manuals.
- Any peer installing a library inherits its manual — the P2P knowledge
  commons — with `author` + the unreviewed tag as trust hooks.
- Any MCP client inherits the manuals too, once flowmcp exposes them as
  resources: the store documents itself to whoever connects.
- Review effort concentrates where it means something: promote diffs
  and primer refreshes, not raw deposit streams.
- The executive's epistemic-pressure queue (donor Phase 4) gets a
  natural extra source later: unpromoted-claim pressure ("this has been
  sitting in the brain for a month; promote or discard?").

---

## 6. Migration inventory (current kb, 2026-08-14)

- `platform-api` (37+ claims) → subject controls in the newbound repo,
  one batch (A2).
- Crate-subject claims (flowlang, ndata) → flow's primary control (A2).
- `workflow` → primer (process-grade) or brain (session-specific);
  triaged at A2, curated at A4's first primer export.
- `doctrine` → primer, verbatim.
- `m2026-07` and episodic buckets → brain only; they stay with whichever
  instance made them.
- hollis-subject claims → the hollis repo's facets via promote (A3+).

---

## 7. The owner's calls (recorded 2026-08-14)

1. Jerry stays a separate unused donor library; executive function
   migrates into `agent` as we go. Blend cognition + executive under
   the name "agent"; drop "jerry."
2. Hollis external as reference sensor plugin — agreed.
3. Eponymous-control convention confirmed; manuals move "all the way
   into flowlang" as the read surface (control metadata → API reference
   → MCP already live there; memory facets add flow-resident
   documentation). Crate-subject claims file onto flow's primary
   control.
4. kb instance-owned — agreed. Primer: single JSON under `docs/`.
5. Perception contract ships with agent.
6. Audience guard: hard refuse.
7. Promote: explicit command, plus warn-if-unpromoted on publish.
8. Newbound-repo claim migration: one batch.

First branch on blessing of this revision: **A1** — the two enumeration
loops, the guard, and the caps.
