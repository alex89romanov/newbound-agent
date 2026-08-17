# The understanding loop — design & plan

**Status: design of record, amended by the owner 2026-08-15.** Originally
written 2026-08-12 as the jerry proposal; entering this repo with the
owner's corrections folded in. Naming note: "jerry" below is historical —
per `one-memory-cycle.md` §0 the executive function lands as the
**`agent.executive`** control inside the agent library, and the jerry
repo is a parked donor. The two owner corrections of 2026-08-15 are
marked **[AMENDED]**: the training architecture (commitment 5 and Phase
6 — the base *grows*; the doc's original frozen-base-plus-knowledge-LoRA
inverted the owner's vision) and the sensor paradigm (commitment 2 and
Phase 2 — the built-in sensor is the resident codebase; the paradigm
must expand to all environmental input). Hard rules unchanged: branches
always, writes through platform commands, mutating tests on disposable
instances, nothing merges without the owner's express permission.

## The goal (owner's words, paraphrased)

Build and continuously update a contextual representation — an
*understanding* — of the full local environment, and use learnings in
real time to build and update the local model (nanochat), which
eventually becomes THE local model. Drive both with an OODA loop that
uses that context.

## Where the donor stands (verified by boot test, 2026-08-12)

- Boots clean: init → bootstrap check → fresh state → four threads
  (narrative processor, CPT curator, speech, executive OODA). The loop
  idles quietly when queues are empty.
- Inputs: `jerry.language.hear` (text — new; previously voice was the
  *only* way in) and `listen`, delegated to whatever `STT_CTL` names
  (no provider installed yet; hollis is the intended one).
- LLM: delegated through `LLM_CTL` → `agent.llm.ask_llm` by default;
  round-trip proven live.
- Memory: a private, in-process stack — thought stream, embedding graph
  (fastembed), narrative fact extraction — persisted once to
  `jerry_state.json` and never re-saved (`TODO - Save jerry state`).
  This parallels what kb + archivist already do in the store.
- Initiative: `spawner_task` / `drive_task` disabled. When enabled,
  debugging got chaotic fast — spontaneous acts had no attribution.
  They are crucial (the executive only wakes for inputs or thoughts;
  nothing currently generates thoughts) but must come back domesticated.
- Bootstrap hazard: with no model artifacts, `bootstrap_llm` clones
  nanochat and runs full training *synchronously inside init* — and
  init discards its result either way.

## The five commitments

1. **One memory.** Understanding lives in the store as claims (the kb
   shape: provenance, confidence, staleness hashes), not in a side JSON
   file. The narrative engine becomes a *producer of claims*; the
   `_patches` journals provide history and provenance for free. One
   brain — the executive is a mouth that talks to it continuously
   instead of at session boundaries. (Since amended by
   `one-memory-cycle.md`: the brain is instance-owned; library-subject
   knowledge federates onto the subject libraries as manuals; nothing
   the executive deposits touches git without a deliberate promote.)

2. **Sensors are procedural.** Never spend model cycles on the
   derivable. Sensors emit typed perceptions into the executive's input
   queue, pre-joined to the claims they touch. The LLM's job starts
   where derivation ends: adjudicating what a change *means*.

   **[AMENDED 2026-08-15 — the sensor paradigm.]** The first sensor —
   included with the agent library, not a plugin — is for the
   **resident codebase**: everything observable procedurally from
   inside the process boundary, in growth order store journals (the
   change feed of the entire object graph — the reference
   implementation), then filesystem, builds, and peer events. Because
   the executive acts only through platform commands, which are
   journaled, its own actions arrive back through this sensor as
   perceptions: the agent's first experience is itself. **Hollis is the
   first official plugin sensor** (ears; the reference plugin), with
   camera scheduled close behind (eyes). The paradigm must be
   *exceptional at code first* and expand to all non-code environmental
   input — situational awareness is bounded by the sensor contract, not
   by the codebase. Contract clauses that enforce the expansion:
   - The envelope is modality-agnostic; only payloads are modal:
     `{kind, timestamp, sensor, payload, touched-claims}`, with
     `store_change` / `file_change` / `peer_event` / `text_input` as
     built-in kinds and `acoustic_event` / `visual_event` as the first
     plugin kinds.
   - **Zero-executive-change test**: adding a modality requires a new
     payload kind and a new binding function, and nothing else. If the
     envelope cannot express code perceptions crisply, fix the
     envelope; never special-case a sensor.
   - **Each sensor owns its binding function.** Perceptions arrive
     already attached to the beliefs they touch: for code the binding
     is exact (staleness hashes join a changed record to the claims
     citing it); for hollis it is voiceprint→entity-claim resolution;
     for camera, its visual-signature mapping. Sensor-state vs. claim
     is a contract clause: the sensor keeps its perceptual state
     (journal cursors, voiceprints, embeddings); the store keeps the
     meaning.

3. **Initiative from epistemic pressure.** Spawn and drive return, but
   drawing from claim state instead of randomness: stale claims to
   re-verify, contradictions to adjudicate, gaps to explore. Every
   spontaneous act is attributable to the claim that provoked it —
   chaos was un-attributable initiative; this is debuggable by
   construction. Drive becomes a dial (how much idle capacity goes to
   epistemic work), never a content source.

4. **Tiered cognition behind seams.** Reflexes are procedural; a fast
   salience judgment ("does this matter?") is nanochat's job; full
   deliberation escalates to the frontier through the agent library.
   All three behind runtime-meta seams like the ones already landed
   (`LLM_CTL` pattern). Every escalation where the frontier disagrees
   with nanochat's salience call is a labeled training pair.

5. **[AMENDED 2026-08-15 — the flywheel: the base grows; the skin is
   re-derived.]** The original text here ("base stays frozen; LoRA
   skills accumulate and version") inverted the owner's vision and is
   superseded. The corrected architecture assigns each mechanism what
   it is actually good at — knowledge lives in weights via continued
   pretraining; personality and chat skill live in a low-rank adapter:

   - **The base trains continuously in the background.** A resident
     online-learning service (trainer and server in one process behind
     the `model` control, speaking HTTP on localhost; **[AMENDED
     2026-08-16: no dispatch seam — the subsystem is ON or OFF via
     `SALIENCE=on` in settings, and the agent bootstraps its own
     server when it's on and nothing is answering]**) steps all day: new curated tokens enter an ingest buffer the
     moment the narrator/archivist produces them, and **every step
     samples a mixed mini-batch** — fresh tokens + a reservoir of older
     synthetic data + standard nanochat data at a set replay ratio.
     Arrival-order per-token training is explicitly rejected (correlated
     gradients, recency capture, catastrophic forgetting); the
     replay-buffered stream is its observably-identical convergent
     twin. Open-ended LR schedule (constant-with-decay, no cosine-to-
     zero: there is no end of training). The sampler's ratios and epoch
     caps are the overfit guard — a day of dev activity is 10⁴–10⁶
     tokens against ~10⁹/day of trainer appetite.
   - **The curriculum**: adjudicated claims and escalation-disagreement
     pairs, never raw logs — plus a frontier **narrator** pass that
     watches development in real time (journals, sessions, diffs,
     build results) and expands the distilled facts into CPT-grade
     text: explanations, Q&A, counterfactuals. Code events lead the
     curriculum because their ground truth is mechanically checkable;
     non-code modalities join through the same adjudication gate later.
   - **Serving is double-buffered with split pointers.** Weights are
     never served from the tensors the optimizer writes; the serving
     copy swaps between steps (milliseconds), so the updated base is
     available to the very next request. The **salience tier serves the
     live pointer** (its failure mode is self-correcting: bad salience
     produces bad escalation calls, which the epsilon-audit of
     "not-salient" verdicts catches); **user-facing surfaces serve the
     last gated checkpoint**.
   - **The gate moves behind the weights**: the eval suites — domain QA
     generated from staleness-checked claims, a general-capability
     slice (the replay mix's report card), and a personality/chat
     regression probe — run continuously against the live weights, and
     a regression **auto-rolls the serving pointer back** to the last
     good checkpoint while training is repaired. Checkpoint ring every
     N minutes plus a daily durable one bounds any bad stretch.
     Promotion of the gated pointer is always a *pair*: base checkpoint
     + its adapter.
   - **Personality and chat skill are a LoRA over the moving base**,
     re-derived when the continuously-running personality probe slips —
     metric-driven, not calendar-driven. Adapters degrade gracefully
     under base drift (a low-rank nudge on activations), which is what
     makes immediate serving under the standing skin sound.
   - **Epsilon-audit the salience gate's silence**: a small random
     fraction of "not salient" verdicts gets frontier review anyway —
     the training signal is otherwise one-sided (false-negatives never
     escalate, so they would never be labeled).
   - **Privacy** (owner's call, 2026-08-15 — the claims precedent at
     the data layer): model artifacts never ride a repo, and a base
     that has trained on brain/world data *is* private-class in its
     entirety — weights mix irreversibly, so there is no "promote" for
     parameters and **no public weights, ever**. Instead, every
     curriculum sample is **class-stamped at birth** from its
     provenance (a sample inherits the strictest class of any source;
     the narrator never blends classes in one sample — non-homogeneous
     provenance means private, no judgment calls). The public artifact
     is the **model seed**: `curriculum_export` filters the append-only
     class-tagged training log to public, emits a versioned dataset
     (audited before release: frontier-sampled slice review, string
     scans, and canary tokens planted in private data that the export
     must never contain), and a fresh install trains its OWN model from
     stock nanochat on standard data + the seed. kb-seed bootstraps a
     fresh brain; the model seed bootstraps a fresh mind; both diverge
     under their instance's own experience thereafter. The seed ships
     fetched-at-install (the hollis models pattern), with the mix-ratio
     recipe, never in-repo. **[2026-08-16: the mixed-mini-batch trainer
     is LIVE in the resident service (Phase 6) - candidate-copy CPT,
     fresh/replay/standard mix (MODEL_MIX), a held-out gate (no
     standard-data regression + curriculum parity, MODEL_GATE) before
     any promotion through the double-buffered pointer, and
     consecutive-failure reset. Verified end-to-end on a fabricated
     tiny nanochat base: promote, hold, and reset paths all
     exercised.]** **A fresh install treats seed data and
     standard nanochat data identically** — one ingest, one sampler,
     one mix; no install-time state machine and no grace-period
     mechanism (owner's correction, 2026-08-15). **[AMENDED
     2026-08-16: salience defaults OFF (`SALIENCE` unset); turning it
     on is the deliberate decision.]** When on, the resident model
     judges at tick rate and the frontier stays the auditor — band
     verdicts escalate, disagreements become curriculum. Moving any
     *further* workload onto the local model is a decision made
     someday, when it is demonstrably good enough — informed by the continuously-running
     eval suites and the syspack-shrinkage metric, which measure
     readiness without ever deciding it. The local model is an
     optimization every instance grows into, never a dependency of any
     day.
   - **The metric survives unchanged** and finally becomes reachable:
     how much of the recall pack can be deleted because nanochat knows
     it cold. Shrink the prompt, keep the accuracy. Scale note:
     train-state + serving copy co-reside comfortably at d20 (~561M);
     above that (d26/d32), offload optimizer state to CPU.

## The OODA loop, phase by phase

- **Observe** — perceptions arrive pre-joined to beliefs ("this
  changed, and it touches these three claims"). No LLM spent.
- **Orient** — retrieval, not reasoning: pull relevant claims with
  confidence and staleness; flag contradiction ("input disagrees with a
  high-confidence belief" is the most salient signal there is). Same
  assembly as `chat_llm`'s recall pack and the coding session's brief —
  one brain, third mouth. Orient calls `agent:archivist:recall`
  directly — no seam (owner's call, 2026-08-15: it's all agent now;
  seams are reserved for boundaries where the filler crosses an
  ownership or delivery line, and command dispatch is already
  store-late-bound). The executive still does not own retrieval: the
  recall RESULT SHAPE is the contract it depends on.
- **Decide** — rank against goals *plus* the epistemic work queue.
  Existing goal fields (priority, focus, boredom threshold) survive;
  drive modulates the pull rate.
- **Act** — through platform commands, so acts are journaled, so the
  executive's own actions flow back in through the code sensor like
  everything else. Self-model for free.

Two scars turned into rules: **hysteresis** (a flaky sensor moves a
claim's confidence, never toggles the claim — no belief-thrash), and
**observability before autonomy** (dashboard shows current OODA phase
and the claims behind the last decision; the loop is killable —
`FIXME - Should be killable` is architecture now, not a nit).

## Phases

Each phase lands on a branch, is verified on a disposable instance,
and is useful even if the next phase never happens. (The one-memory
cycle's Track A — federation, migration, promote/seed/bootstrap, and
the instance-owned brain — landed first and is the ground these phases
stand on.)

### Phase 0 — Clean the launchpad
Strip init's true dead weight (the `// WTF?!` call, commented
`populate_system_knowledge`, `stream_new` duplicate, orphaned imports —
the bulk of the 79 warnings); keep the Once + `start`-flag guard.
Make the executive loop killable and expose current phase in state.
Guard bootstrap: init must not train synchronously — check artifacts,
report, move on (surface "model missing" as a claim instead).
Applied on entry as the executive migrates into `agent.executive`
(one-memory-cycle Track B2).
*Verify:* boot test — same clean boot, warning count collapses, loop
stops on command. *Unblocks:* safe iteration on everything below.

### Phase 1 — Orient reads claims **[AMENDED: seam squashed]**
`agent-archivist-recall`: claims by topic across the whole federation
(the brain and every library's manuals through one door), scored by
match, with staleness computed from the source hashes `remember`
stamps — stale means the referent facet drifted or vanished, not
merely aged. The executive's loop orients each drained perception
through a direct call to it (owner's call, 2026-08-15: no
`CONTEXT_CTL` — it's all agent now, and command dispatch is already
store-late-bound); the recall RESULT SHAPE ({claim, detail, tags,
confidence, home, time, age_days, stale, stale_checked, promoted,
score}) is the contract, and a broken or missing recall degrades to an
empty context rather than killing the loop. The orientation lands in
`status.last_context` — observability first, prompts later.
*Verify:* disposable; seed a claim with a facet-pointer source; recall
ranks it first fresh, then stale after the referent is patched;
perceive an envelope touching it and `status.last_context` names it.
*Unblocks:* everything — once orientation reads claims, sane
initiative, the salience tier, and the curriculum have a place to
stand.

*Executed 2026-08-15 on claude/phase1-recall: recall + the orient step
in the executive loop, verified end-to-end on a disposable (153 claims
federated; conversational query ranks the seeded claim first with 7
matched after stopword filtering; staleness flips on referent drift;
perceive→orient→status round-trip under 2s).*

### Phase 2 — First sensor: the resident codebase **[AMENDED]**
The built-in sensor family, journal tailer first: a task emits
`store_change` perceptions, each joined to the claims whose source
hashes cover the changed records — the reference implementation of the
perception contract. Filesystem, build, and peer perceptions extend the
same family behind the same envelope. `hear` stays the text sensor;
hollis arrives later via the existing `STT_CTL` seam untouched (widened
to `acoustic_event` per the contract, so its annotations — speaker
entity, location, ambience deltas — survive the boundary).
*Verify:* on a disposable, edit a store record via `dev.code`; watch
the perception appear in the input queue with the right claims
attached. *Unblocks:* the executive subscribes to reality, starting
with itself.

*Executed 2026-08-15 on claude/phase2-sensor: `agent.sensor`, the
journal tailer — mtime-gated sweep every ~2s, one `store_change`
envelope per new journal entry, claims bound by source pointer with
staleness by hash compare, delivered through `perceive` like any
external sensor. Verified on a disposable: a `dev.code` facet patch
arrived in `status.last_context` with the source-pointed claim bound
and stale within one sweep — and the self-model held: a `remember`
deposit (a journaled act) returned through the sensor as the next
perception. Cursor is runtime state (resets to now on start, no
history replay); the persisted cursor arrives with sensor-state
records later.*

*Phase 9 — the first PLUGIN sensor — executed 2026-08-17 on
claude/phase9-hollis (agent repo) + the hollis repo's matching branch.
Hollis's cortex now proposes every resolved transcript as an
`acoustic_event` envelope through `agent.executive.perceive`
(`Command::lookup`, fire-and-forget on a thread), with binding done
through the agent's own recall — the claims naming the resolved
speaker ride the envelope. The dependency arrow is one-way by the
owner's rule: hollis knows agent; agent's only new code is
sensor-AGNOSTIC per-sensor accounting (rows keyed by the envelope's
`sensor` field) surfacing in executive status and the mind tab.
`hollis.audio` gained `inject` (the no-microphone test surface,
home of the shared emit path), `status`, and `transcripts`; the
`emit=off` botd key silences the sensor without stopping the ears; a
missing agent library is a counted skip (verified agent-less). The
hollis app UI — previously empty facets — was created in the
agent-app idiom: chips, cortex toggle, inject form, counters,
transcripts. Battery on a disposable: 3 marker injects steered
fast/normal/deep, per-sensor counts rendered in both UIs, gate
off/on cycled, named-entity inject arrived with `bound: 1` and the
seeded claim as `bound_top`.*

### Phase 3 — Narrative engine deposits into kb
Fact extraction produces claims (provenance, confidence) instead of
private graph nodes; adjudication updates/supersedes claims under the
hysteresis rule. `jerry_state.json` shrinks to executive runtime state
only — understanding no longer lives in a file that's saved once.
Embeddings are computed at query time (owner's call, one-memory-cycle
§7): no fastembed in the hot path, no derived indexes in the canonical
store.
*Verify:* a conversation on a disposable ends with new claims in the
store, visible in the brain (instance-owned — nothing rides git; the
subject-bearing ones await promote). *Unblocks:* the curator has
adjudicated material; the store is the single source.

*Executed 2026-08-15 on claude/phase3-adjudication:
`agent-archivist-adjudicate`, the write-side counterpart to recall,
with the donor's Curator shape (match → decide → apply → trace) under
the hysteresis rule made mechanical: relationship is decided
procedurally where certain (exact restatement, no candidates) and by
the LLM where judgment is needed (parse failure = the donor's
default-INSERT), but the EFFECT is never the LLM's to choose —
corroboration moves confidence one step up (settled-and-writes-NOTHING
at high: the convergence fixed point), contradiction one step down,
and only a contradiction at the low floor supersedes, with the
successor entering at low to earn its way up. Three consecutive
contradictions to flip a high belief; no thrash. Superseded claims
stay in the facet as history but never answer recall. Curation traces
(capped 200) land on the domain's `traces` facet — audit today,
curriculum later. `consolidate` files through adjudicate now, so
restatement is evidence instead of a dedupe error. Verified on a
disposable with the LLM stub: both ladders walk correctly, supersede
fires only at the floor, recall hides the retired belief and returns
the successor, a logged conversation ends as a recallable claim
(swept 2, filed 1), and the fixed point holds under repeat
stimulation. jerry_state.json needed no shrinking — the fold kept
understanding in the store from birth; embeddings stayed query-time
(token match), no fastembed anywhere.*

### Phase 4 — Initiative, domesticated
An epistemic-work command derives the queue from claim state (stale /
contradicted / gapped — plus unpromoted-claim pressure). Decide pulls
from it; drive returns as a budget dial; spawner's random injection is
deleted for good. Dashboard attribution ("acting because: claim X,
state Y") ships *in the same phase* — observability before autonomy,
in one unit.
*Verify:* seed a stale claim on a disposable; the executive notices and
acts on it unprompted, and the dashboard says why. *Unblocks:* an agent
that does things on its own without becoming undebuggable.

*Executed 2026-08-15 on claude/phase4-initiative:
`agent-archivist-epistemic_work` derives the queue from claim state —
stale (source hash drifted, unacknowledged), review (confidence at the
low floor, or drift acknowledged), unpromoted (subject-bearing brain
claims; promotion pressure surfaced, never auto-promoted — that
channel stays curated). No LLM, no randomness; the donor spawner's
random injection was deleted for good by never being built. The
executive's idle branch decides at the drive budget's pace
(`set_drive`, acts/hour, 0 = off; perceptions always preempt) and the
ONLY autonomous write is `decay`: an evidence-based confidence step
down the same hysteresis ladder, at most one step per distinct drift
state (the observed referent hash is recorded; repeats are no-ops, the
floor writes only its acknowledgment, once) — initiative is convergent
by construction, closing the loop-gain question. Attribution ships in
`status.last_act` (kind, claim, home, why, action, before/after).
Verified on a disposable: the queue found real pre-existing work in
the copied brain unprompted; a staged drift decayed high→medium within
one budget tick with the dashboard saying why; the same drift never
decayed twice; a second drift walked the claim to low; perceptions
preempted; drive 0 froze acts; stop stopped. "Gapped" detection needs
semantic judgment and waits for the salience tier.*

### Phase 5 — The salience tier **[AMENDED: serving; 2026-08-16: no
seam]** Orient's "does this matter?" judgment goes to nanochat — ON
or OFF via `SALIENCE=on` in botd.properties, nothing pluggable (owner:
"no one is swapping nanochat out for something else"). The executive
calls the `agent.model.salience` client directly; it answers from the
resident service's **live pointer** (double-buffered weights, standing
personality adapter), and if the service isn't running the agent
builds and launches it itself (`agent.model.bootstrap`, fired once per
start). Escalation disagreements are logged as
training pairs; the epsilon-audit samples non-escalated verdicts.
*Verify:* tick-rate orientation runs without frontier calls;
escalation log fills with (input, nanochat-call, frontier-call) rows.
*Unblocks:* the loop runs hot without cost explosion; the curriculum
writes itself. **[2026-08-17: STEERING is live (Phase 7, owner
go-ahead) - the verdict is computed first (bound-claims-only context,
matching the agreement gate's conditions) and gates orientation depth:
fast path below SALIENCE_BANDS low= (no recall; epsilon audit
retained), deep recall above high=, unchanged between.
SALIENCE_STEER=off reverts live. The founding restraint - verdicts
recorded, not acted on - is hereby lifted for orientation; acts remain
drive-budgeted and decay-only.]**

*Executed (5a, the architecture half) 2026-08-15 on
claude/phase5a-salience. `SALIENCE_CTL` is a REAL seam under the
squashed-seam doctrine — its filler crosses a delivery boundary (a
stub today, the frontier if the owner points it there, the resident
nanochat live pointer when the serving half lands) and swapping
fillers must change nothing in the executive: the zero-executive-
change test is the 5b acceptance criterion. The orient step asks the
seam for a verdict {salient: 0.0-1.0, reasoning} per perception and
RECORDS it in last_context — verdicts are not yet acted on
(observability before autonomy, the Phase 4 move repeated).
Owner-blessed defaults: uncertainty band 0.35..=0.65 escalates to the
frontier (via ask_llm, a direct in-library call); a deterministic 5%
epsilon-audit (hash of query + perception time — reproducible, no rand
dependency) samples confident verdicts; disagreement = |local −
frontier| > 0.25; the escalation/audit log lives in the runtime
library (instance-owned, unjournaled so the store sensor never
perceives the audit trail, capped 1000); at most one frontier call per
5s, band escalations dropped in between are counted. Verified on a
disposable with salience + frontier stubs: verdicts recorded; a
mid-band perception escalated and logged its disagreement (0.5 vs
0.95); the cooldown dropped and counted a second escalation; a
33-perception confident burst ran tick-rate with zero additional
frontier calls; a crafted epsilon hit produced an audit row with
agreement correctly classified. The serving half (5b) — the resident
online-learning service's live pointer behind this same seam — waits
for the owner's hardware.*

### Phase 6 — The flywheel, continuous and gated-behind **[AMENDED]**
The resident online-learning service of commitment 5, in full: ingest
queue fed by the curator (adjudicated claims + escalation pairs + the
frontier narrator's expansions), replay-buffered mixed-batch stepping,
double-buffered split-pointer serving, the continuous eval harness
(domain QA from staleness-checked kb, general-capability slice,
personality probe) with auto-rollback, checkpoint ring, and
probe-triggered skin re-derivation. Track the syspack-shrinkage metric
from day one.
*Verify:* end-to-end on the owner's hardware (GPU) — the service runs
for a sustained stretch while development happens; the live pointer
moves, the gated pointer advances only through passing checkpoints, a
deliberately-poisoned batch triggers auto-rollback for stated reasons.
*Unblocks:* the goal itself: learnings landing in weights, safely, as
they happen.

## Sequencing rationale

1 before 4: initiative needs claims to stand on. 3 before 6: the
curriculum needs adjudicated claims, not logs. 0 and the observability
half of 4 before any autonomy: the spawn/drive lesson, paid for once
already. 5 before 6 in spirit (the escalation log is the richest
training source), but 6's curator work can start from claims alone.
Track A of the one-memory cycle precedes Phase 3's deposits (landed
2026-08-14: the brain is instance-owned, so continuous deposits never
meet the branches-always rule).

## Open calls for the owner

- ~~Claim home~~ **Answered** (one-memory-cycle §7): shared kb, filed
  by *subject*; the executive is the author, not the address.
- ~~Embeddings~~ **Answered**: query-time; revisit only if scale
  demands.
- ~~nanochat serving~~ **Effectively answered by the continuous
  design**: trainer and server share weight buffers, so one resident
  process behind the `model` control (PyO3), exposed through
  `SALIENCE_CTL`; an OpenAI-compatible endpoint remains available as an
  additional face if anything external ever needs it.
- ~~hollis timing~~ **Answered**: ears arrive any time via `STT_CTL`,
  widened to the `acoustic_event` perception kind so hollis's
  annotations survive the boundary.
- ~~Privacy classes for the base~~ **Answered** (owner, 2026-08-15):
  single private live model, everything mixed; no public weights at
  all. The publishable artifact is the class-filtered curriculum — the
  **model seed** — from which a fresh install derives its own model
  (commitment 5's privacy clause has the full mechanism: class-at-
  birth, single-class samples, curriculum_export, audit + canaries;
  seed and standard data are one undifferentiated mix at install, and
  the frontier drives indefinitely until the owner decides otherwise).
- **Replay ratio and gate thresholds** (new): the sampler's
  fresh/reservoir/standard mix and the three suites' pass bars are
  dials that need first values; proposals will come with Phase 6's
  design, tuned on the first sustained run.
