# The One-Memory Cycle — fold, federate, seed, cut loose

**Status: proposal, awaiting the owner's blessing** — written 2026-08-14,
synthesizing three design conversations: the memory-federation brainstorm
(memory facets on any control), the jerry understanding-loop plan
(`understandingloop.md`), and the kb-churn decision (instance-owned kb
with extract-on-publish). Hard rules unchanged: branches always, writes
through platform commands, mutating tests on disposable instances,
nothing merges without the owner's express permission. **No store, code,
or repo changes accompany this document.**

The unifying theme, in the owner's words (still being workshopped):
*cultivating, refining, and growing an actionable self-referential
understanding over time.* Working shorthand in this doc: **understanding
that compounds** — perceive → claim → adjudicate → embody, each turn
making the next one cheaper.

---

## 0. The five decisions this cycle makes

1. **Fold jerry into newbound-agent** as a fifth library (`data/jerry` +
   `jerry/` FFI dylib crate). Folding repos does not fold libraries:
   agent stays cognition services, jerry stays the executive loop, each
   its own crate and dylib. An overlay can run agent without jerry.
2. **Hollis stays external as the reference sensor plugin** (camera to
   follow as the second). Its distribution path is proven
   (`dev.github.import` → activate → hot-load); its remaining cognition
   (discourse briefing, occupancy context, its LLM stack) is vestigial
   and migrates to jerry by strangler-fig — nothing is amputated before
   its replacement demonstrably works.
3. **Memory federates** (the brainstorm's pure Idea A): any control can
   carry a memory facet; claims about a control live on that control;
   claims about a library live on its eponymous control. The read side
   (index scan, consolidate enumeration) generalizes from kb-only to
   store-wide. `remember` already supports this; it does not change.
4. **kb becomes instance-owned** — excluded from git on the scratch
   pattern — with a three-tier shape (§2): the **brain** (instance kb,
   living, private), the **primer** (a curated committed seed), and the
   **manuals** (federated library facets that ship with code).
5. **The perception contract gets written down** in this repo's docs —
   the typed-perception schema jerry consumes and sensors (hollis,
   camera) provide. Contracts live with the consumer; plugins implement
   them from outside.

Why these five travel together: jerry's continuous deposits make
kb-in-git untenable (machine-cadence appends cannot ride branches-always
review, and concurrent instances become a merge-conflict factory — the
2026-08-13 kb divergence between the camera and hollis sessions was the
mild preview). Federation is what makes instance-owned kb *survivable*:
it gives most durable knowledge a git home outside kb. The seed covers
the remainder that must outlive any single instance. And the fold puts
jerry inside the same trust and repo boundary as the memory it feeds —
the repo boundary, the git boundary, and the audience boundary become
the same line.

---

## 1. The boundary sentences

One sentence per component; anything in a component that belongs to
another sentence is future amputation.

| Component | Job |
|---|---|
| **hollis** (external plugin) | Perceive acoustically: emit typed perceptions; own its sensor state (voiceprints, calibration, geometry). |
| **camera** (external plugin, later) | Perceive visually: same contract. |
| **jerry** (this repo) | Maintain understanding: OODA loop; claims in, claims out; act only through platform commands. |
| **agent** (this repo) | Cognition services: LLM access, the archivist (remember / consolidate / promote), recall packs. |
| **kb** (this repo, instance-owned) | The one memory: claims with provenance, confidence, staleness. |
| **nanochat** (artifacts, never committed) | Embody the memory: the salience tier, trained from adjudicated claims, gated by held-out QA. |

Sensor-state vs. claim is the load-bearing distinction hollis already
demonstrates: the *voiceprint* is sensor-owned perceptual state; the
*binding* of that voiceprint to "Marc" is a claim in the one memory,
with provenance and confidence. Every future sensor follows this split.

---

## 2. The three memory tiers

### The brain — instance kb
The live `data/kb`, gitignored wholesale on the scratch pattern
(skeleton meta.json + controls index committed once, skip-worktree'd;
everything else untracked). Deposits — session, jerry, archivist — land
here freely at any cadence. `_patches` journals still accumulate, so
provenance survives locally; it simply stops being git's problem. Two
instances having divergent brains is not a bug to merge away; it is the
nature of separate experience. Sharing happens through promotion or,
eventually, P2P — never through git merges of raw memory.

Backup posture: the brain is runtime state, backed up like runtime
state. The seed-export command (§3) doubles as the backup tool.

### The primer — the seed
A curated, committed export of brain-essentials: doctrine (the owner's
voice), the working process, and whatever else a fresh instance must
know before its first act. Updated **only** by a deliberate export
command, reviewed like a docs change; it never churns because it only
changes on purpose. A fresh instance bootstraps its brain from the
primer (overlay.sh or an archivist bootstrap command) and diverges
freely. Audience: the primer inherits the repo's privacy — it lives in
newbound-agent precisely because doctrine never rides a public library.

### The manuals — federated library facets
Memory facets on the libraries the knowledge is about, filed by the
eponymous-control convention, shipped through every existing channel
(git, `dev.github.import`, P2P `install_lib`). A library arrives
carrying its own accumulated manual; the claims about `dev.code` version
in the same diff as `dev.code` itself, and a source-hash mismatch in
that diff is a self-flagging stale claim. This is where the bulk of
session learnings durably live — which is what makes the brain safe to
cut loose from git, including for ephemeral (CCR) sessions whose
containers are destroyed: what a session learns about a library survives
in that library's repo; only episodic residue dies with the container,
and episodic residue was never worth a merge conflict.

---

## 3. New and changed machinery

Small, and mostly already known:

- **Index scan** (`agent/src/agent/llm/tool_loop.rs:144`): enumerate all
  libraries → all controls → include any control with a memory
  attachment, labeled `lib.ctl`. kb's controls appear automatically,
  unchanged. The index becomes a literacy map of the whole store.
- **Consolidate** (`agent/src/agent/archivist/consolidate.rs:87,166`):
  same enumeration for the known-claims pack; filing passes the claim's
  subject lib instead of the `"kb"` literal. The pack gets per-domain
  caps / tag filtering **in this cycle, not later** — a continuously
  ticking Orient reading an unbounded store-wide pack is a cost
  explosion scheduled in advance.
- **Audience guard in `remember`** — also this cycle, not "later
  convention": refuse (or flag — owner's call, §7) deposits of
  private-class claims (doctrine-tagged, owner-voice) into any library
  with a non-empty/anonymous readers list. The moment jerry lands, the
  depositor is an autonomous loop; conventions that hold for a careful
  session do not hold at machine cadence.
- **`promote`** (new archivist command): sweep the brain for claims
  whose subject is a given library — procedural under federation
  addressing, the claim's `lib.ctl` domain *is* the extraction key —
  **union-merge by claim identity** into the library's shipped facet,
  mark the working copies promoted. The library's next commit carries
  them, reviewably. This union-by-identity merge is the same machinery
  the eventual `install_lib` clobber fix needs; build once, use twice.
  Promote also quietly answers "autonomous deposits meet
  branches-always": jerry writes freely into the brain, and nothing
  jerry produces ever touches git without a deliberate promote.
- **`seed_export` / `bootstrap`** (new archivist commands): export the
  curated primer from the brain (deliberate, reviewed); populate an
  absent brain from the primer on first run.
- **Session-end ritual changes** (CLAUDE.md): deposit → **promote** →
  commit manuals + any primer refresh. The brain stays local. The
  "commit data/ changes and push" instruction stops applying to kb.

Explicitly **not** in this cycle: P2P union-merge in `install_lib` and
memory-aware version bumps (noted, deferred — git remains the channel);
any jerry phase beyond 0 (this cycle builds the ground jerry's phases
stand on).

---

## 4. Phases

Each phase lands on a branch, is verified on a disposable instance, and
is useful even if the next never happens. Track A (memory) and Track B
(fold + contract) are independent until Phase A4.

### Track A — the memory work

**A1 — Federation MVP.** The two enumeration loops + the audience guard
+ consolidate's caps. *Verify (disposable):* seed a memory facet on a
platform control via `remember lib:"dev"`; the index lists `dev.<ctl>`;
consolidate files a dev-subject claim onto dev; a doctrine-tagged
deposit aimed at dev is refused. kb behavior unchanged throughout.

**A2 — Migration.** Disperse the brain's library-subject claims onto
their subject controls via `remember` (journaled, reviewable): the
platform-api claims → the newbound repo (branch commits — knowledge
churn moves into the platform repo, which is the point and the owner's
review load, §7); hollis claims → the hollis repo; crate-subject
(flowlang/ndata) and process/doctrine/episodic claims stay in the brain.
*Verify:* index shows the new homes; recall pack unchanged in content.

**A3 — promote + seed_export + bootstrap.** *Verify (disposable):*
deposit a lib-subject claim → promote → shipped facet gains it,
re-promote is a no-op (identity dedupe); wipe `data/kb` → bootstrap →
doctrine present, brain functional.

**A4 — Cut kb loose.** Gitignore `data/kb` wholesale, skip-worktree the
skeleton, commit the first primer, update CLAUDE.md's session ritual.
One-time: the repo's tracked kb content is removed from tracking in the
same commit that lands the primer, so nothing is ever only-in-history.
*Verify:* fresh clone + overlay + bootstrap on a disposable → working
brain, zero kb entries in `git status` after arbitrary deposits.

### Track B — the fold and the contract

**B1 — Perception contract doc** (`docs/perception-contract.md`). The
typed perception schema: `text_input`, `store_change`, `file_change`,
`peer_event`, and — widened from the current `{millis, sink}` STT shape
— `acoustic_event` carrying hollis's annotations (speaker entity,
location, ambience/state deltas). Hollis's `EventKind` enum is ~80% of
this already; its dead variants are the contract waiting for a
counterparty. Camera reviews against the same doc later.

**B2 — Jerry enters the repo.** `data/jerry` + `jerry/` crate, overlay.sh
extended, jerry Phase 0 (launchpad hygiene: kill the synchronous
bootstrap-training in init, killable loop, warning purge) applied on the
way in — the folded-in jerry is already the cleaned one. Heavy-tail
discipline: query-time embeddings (no fastembed in the hot path), model
artifacts fetched-at-install and gitignored like hollis's models. A LoRA
trained on brain claims inherits the brain's privacy class — training
outputs never ride a repo push.
*Verify:* overlay + build on a clean disposable; jerry boots, idles,
stops on command; no new heavy deps in the default build.

### Sequencing rationale

A1 before A2 (migration needs the addressing); A2 before A4 (nothing
loses its git home before its new home exists — **federate first, then
seed, then cut loose**); A4 before jerry Phase 3 ever runs (the
machine-cadence churn problem should never exist for even a day). B1
anytime; B2 after A1 if convenient (jerry's later phases read the
federated index, but Phase 0 touches none of it).

---

## 5. What falls out for free

- The repo boundary **is** the audience boundary: private brain repo
  (agent, kb, jerry) vs. public plugin repos (hollis, camera) carrying
  only their own manuals.
- Any peer installing a library inherits its manual — the P2P knowledge
  commons — with `author` + the unreviewed tag as trust hooks.
- Review effort concentrates where it means something: promote diffs and
  primer refreshes, not raw deposit streams.
- Jerry's epistemic-pressure queue (its Phase 4) gets a natural extra
  source later: unpromoted-claim pressure ("this has been sitting in the
  brain for a month; promote or discard?").

---

## 6. Migration inventory (current kb, 2026-08-14)

- `platform-api` (37+ claims) → subject controls in the newbound repo
  (A2), except crate-subject claims (flowlang/ndata) which stay in the
  brain until/unless a natural home exists (§7).
- `workflow` → primer (process-grade) or brain (session-specific);
  triaged at A2, curated at A4's first primer export.
- `doctrine` → primer, verbatim.
- `m2026-07` and episodic buckets → brain only; they stay with whichever
  instance made them.
- hollis-subject claims → the hollis repo's facets via promote (A3+).

---

## 7. Open calls for the owner

1. **Bless the five decisions in §0** (individually — they're
   separable, though §0 argues they travel best together).
2. **Eponymous-control convention** for library-level claims (vs. a
   sidecar knowledge control per library). This doc assumes eponymous.
3. **Crate-subject claims** (flowlang, ndata): brain-resident forever,
   or filed onto `flow`'s primary control as nearest kin?
4. **Primer format and location**: kb-shaped directory the bootstrap
   copies, or a single exported JSON the bootstrap imports? (Proposal:
   single JSON under `docs/` or `data/kb-seed/`; import via bootstrap —
   the store stays the only kb-shaped thing.)
5. **Audience guard behavior**: hard refuse vs. warn-and-file-to-brain.
   (Proposal: hard refuse; an autonomous depositor should hit walls,
   not advisories.)
6. **Promote trigger**: explicit command only (proposal), or also
   hooked into publish/`lib_archive`?
7. **A2's review load**: the platform-api migration lands as newbound
   repo branch commits — dozens of claims arriving as store diffs for
   your review. One batch, or trickled per-library?
8. **Naming**: "jerry" as the library/control name, and
   brain/primer/manuals as the tier vocabulary (used throughout this
   doc; happy to rename).

Nothing moves until these are answered. On blessing, the natural first
branch is A1 — the two enumeration loops, the guard, and the caps.
