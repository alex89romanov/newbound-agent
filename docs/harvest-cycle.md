# The harvest cycle — from environment to understanding to weights

**Status: plan of record for the next cycle — drafted 2026-08-17;
amended 2026-08-19 after the spectrum cycle landed (S1–S8,
`docs/spectrum-cycle.md`).** Builds strictly on the shipped framework
(perception contract, federated memory, the salience tier, the
flywheel, the split pointer, the CLAUDECODE arm — and now the
registry, the dataset manager, the backend seam, the delta trainer,
the bench, and the SFT gate). Nothing here replaces an existing
organ; every phase feeds one that already runs. The amendment in one
sentence: the banks this cycle fills are now REGISTERED DATASETS
(the spectrum's feed contract — no orphan banks), and the "eventual
chat-SFT refresh" this cycle only banked toward is no longer
eventual — `agent-model-sft_run` exists, gated and soaked, waiting
on the bank.

## Why this cycle

The charter: a system that maintains — grows, curates, infers — a
self-referential understanding of its environment (location, entities,
conversation, codebase, operating system, visual cues, environmental
sensors, hardware state) over time, with a local model that learns
from and eventually replaces the frontier model driving it.

Two observations from first live contact set this cycle's direction:

1. **A room teaches facts, not salience.** Listening yields a metric
   ton of information — who is present, what was discussed, the
   rhythms of the household — but very few *salience lessons*: most
   utterances are mid/low salience and the pairs are weak. The
   salience gate remains the flywheel's spine for the CODE domain
   (where store changes genuinely vary in import), but the acoustic
   domain's harvest must be **claims**, not verdicts. Today hollis's
   transcripts are judged and then... nothing. The understanding they
   carry evaporates.

2. **We pour trainable text on the floor.** Every frontier
   request/response — in-domain, answering about THIS system — is
   discarded after use. Every facet edit invites the question "why?"
   and its answer, and the store journals every one of them with
   labels and authors; nobody asks. The raw and synthetic capture
   channels are the cheapest high-value work available.

**A standing rule for every phase (owner, 2026-08-17): behavior NEVER
branches on which model the LLM arm points at.** Whatever serves —
vLLM, a hosted API, the Claude Code bridge, someday the local model
itself — every mechanism below treats it identically. The serving arm
appears in captured rows as a provenance tag (so training data can be
filtered by source later), and that is the only thing it is ever
allowed to be. (The spectrum cycle extended this law to the resident
itself — model identity and scale are never branch conditions — and
to synthetic-data generators, which are provenance tags under the
same rule: `docs/spectrum-cycle.md` standing rule 1 and ruling 10.)

Four goals, one loop: **acquire** understanding, **ruminate** on it,
**capture** the evidence as training data, and **assemble** it into
purpose-built contexts — which is both how the frontier gets smarter
about us today and how the local model needs less context tomorrow
(the syspack-shrinkage metric, finally made real).

## Current capture inventory (what already runs)

| Channel | What it yields | Status |
| :-- | :-- | :-- |
| Journal tailer → adjudication | claims about the codebase, hysteresis-guarded | live |
| Archivist chat sweep | claims distilled from chat sessions | live |
| Salience escalations + audits (incl. unparseable) | salience pairs → CPT curriculum | live |
| Session-end deposits + promotes | manuals on subject controls | live (agent-driven) |
| curriculum_export → stream datasets (`salience-pairs`, `memory`) + legacy ingest | CPT feedstock, registered and deduped | live (spectrum S2) |
| Persona corpus | voice (authored, not harvested) | live |
| hollis transcripts | stored, judged... then nothing | **the acoustic gap** |
| Frontier req/resp | discarded (LLM_CAPTURE_DIR covers ask_llm only, off) | **the raw gap** |
| Edit rationale ("why") | journaled labels nobody reads | **the synthetic gap** |
| OS/hardware state | not sensed at all | **the sensor gap** |
| Context assembly | ad-hoc per surface (query strings, memory:index fence) | **the assembly gap** |

## The phases

Ordered so the banking starts on day one (data lost while we build is
lost forever) and the assembler lands early (every later phase is a
customer of it). Each phase is a branch, a disposable battery, and a
merge word — the established rhythm.

### H1 — The message store, then bank the raw stream (day 1)

**First, the foundation: messages become records, referenced by ID.**
A conversation's turns ride every subsequent request in that
conversation, so naive capture stores message 1 once per turn —
quadratic duplication before the salience log, the archivist queue,
and the SFT bank each take their own copies. Instead: a new
**instance-specific library** (working name `msgs` — owner call), in
the kb/runtime posture (skeleton ships, nothing under its data ever
commits, skip-worktree + ignore). One record per individual message:
`{id, t, role, venue, content, entity?, provenance}` — and because
the store is content-addressed, identical text dedupes structurally.
Everything downstream then references **IDs, not text**:

- capture rows become `{t, venue, arm, model, msg_ids[], reply_id,
  tools?, cost_usd}` — a few dozen bytes per call
- a conversation is an ordered ID list; export renders by join
- hollis transcripts unify into the same space (an utterance IS a
  message from an entity, venue `room`) — one message universe for
  chat, frontier traffic, and the household's speech
- claims and training pairs cite message IDs as provenance, so every
  future lesson can be traced to the words that taught it
- the H2 assembler draws messages by ID and never pastes duplicates
  into one context

**Then capture**, now cheap: with `LLM_CAPTURE=on` (botd, live key),
every frontier call through `chat_llm` — all arms identically —
appends one ID-referencing row to
`runtime/agent/model/capture/YYYYMMDD.jsonl`. This supersedes the
ask_llm-only Q/A text files (fold them in, same switch).

Then teach `curriculum_export` a `chat` kind: captured turns rendered
as conversation rows (`{"messages": [...]}` — the backend-neutral
shape the persona, adapter, and SFT loaders already speak; the
serving dialect renders at the seam, per-resident), swept into a
REGISTERED STREAM DATASET (`chat-bank`, kind `sft`) through the
spectrum's feed contract — **separate from the CPT streams, and
never a loose directory** (no orphan banks; the original draft's
`runtime/agent/model/sft/` predates the dataset manager). The CPT
trainer does not eat it; it is `agent-model-sft_run`'s feedstock —
the chat-SFT refresh is no longer "eventual": the run, its
three-instrument gate, and the soak into the user pointer shipped as
spectrum S8 (`docs/spectrum-s8.md`), and the day this bank holds
8 train + 2 held-out conversations it is one command from a gated
candidate. The provenance tag still rides every row, so the run can
weigh sources as it sees fit.

*Owner call:* capture default (proposal: off in the repo, on in your
botd), and whether hollis-derived text is capture-eligible (proposal:
yes — it is already instance-local; capture adds no new exposure).

### H2 — The context assembler (day 2) — the crucial one

A first-class command: `agent.context.assemble(purpose, subject,
budget)` → an ordered, provenance-tagged context block drawn from
every knowledge source we have:

- federated claims (recall, with staleness marks honored)
- the subject's actual code (facets/command bodies via the claims'
  source pointers — the store IS the codebase)
- recent acoustic reality (transcripts + occupancy from hollis's
  stores)
- system state (once H4's sensor exists) and service/trainer metrics
- recent session history (the archivist's turn queue, transient)

`purpose` selects a **profile** — `chat`, `escalation`, `rumination`,
`coding`, `briefing` — each with its own per-source budget weights;
`budget` is a token ceiling the assembler enforces by ranked
truncation. Every block carries provenance (`[claim kb.doctrine #123]`,
`[transcript 20:26]`) so downstream answers can cite and downstream
training pairs inherit traceability.

Consumers, wired in this phase: the salience **escalation prompt**
(today the frontier judges from a bare query string — it should see
the bound claims and code context; better escalation labels = better
curriculum for free) and a `context` command surface any consumer can
call — chat shells, tool-capable delegates over MCP, rumination acts,
all arms alike. The notebook's memory:index fence stays; this
augments.

*Metric:* assembled-context token counts land in metrics.jsonl per
purpose — the syspack-shrinkage baseline. The long game is watching
the budget needed for good answers FALL as the local model absorbs
the domain.

### H3 — The why-harvester (day 3)

The store already journals every mutation with patch labels and
authors; our own commits carry rich rationale. Two harvesters:

- **Procedural** (no model): pair each journaled patch with its label
  and author; pair each commit diff with its commit message. These
  are (change → stated-why) pairs, free, thousands available
  retroactively from git history and `_patches` journals.
- **Distilled** (frontier, budgeted): for significant changes (new
  command, changed behavior — significance from the salience verdict
  the store sensor already produces), assemble an H2 `coding` context
  and ask: *why was this change made, and what does it teach about
  how this system works?* → (a) a claim on the subject control,
  hysteresis-guarded like any adjudication; (b) a synthetic QA pair
  ("Q: why does bootstrap rewrite service.py? A: ...") into the
  `chat-bank` dataset with provenance back to the patch. This IS
  `dataset_derive`'s model-driven mode (spectrum ruling 10): the
  generator is a provenance tag, the spend is deliberate or
  drive-budgeted, never ambient, and the derived rows carry lineage.

This is the code-domain harvest matching the charter's "exceptional
at code first": the system explaining its own becoming, in trainable
form.

### H4 — The room yields claims; the box becomes a sense (day 4)

**Acoustic consolidation** — executive-side, NOT hollis-side (sensors
stay procedural): a drive-budgeted act that runs when conversation
subsides (cadence/occupancy signals hollis already emits). It takes
the recent transcript window + entity records, assembles an H2
context, and asks the frontier for CLAIMS: what happened, who was
involved, what was decided, what patterns recur ("Marc works
evenings", "the household discusses X on Sundays"). Deposits go to a
new `environment` domain (or per-entity homes) with `inferred`
confidence, subject to the same hysteresis and owner audit as
everything else. Entity naming closes the loop: when a voiceprint
entity gains a name in conversation ("I'm Marc"), that becomes a
claim binding voiceprint → person — the binding hollis's contract
always promised.

**The system sensor** — a new procedural built-in (contract §4
addition, kind `system_state`): disk, memory, load, GPU
utilization/temperature, service liveness. Threshold-crossing
emissions only (never polling spam — coalescing is the sensor's first
responsibility). Payloads are observations; "disk will fill by
Friday" is a claim the executive may infer. Cheap, immediately useful
(the agent noticing its own GPU is busy is self-model), and it
exercises the zero-executive-change test again.

**"Being a good sensor" — the design exploration.** Hollis currently
emits only transcripts, but its lower levels produce a stream of
determinations we leave on the table: transient classifications
(label, confidence, dB), continuous-source labels, ambience shifts,
state changes, cadence, DOA locations, voiceprint match scores,
calibration health. The contract's mapping table already reserves
rows for all of these; this workstream decides — deliberately, in
writing — which ones an agent-grade sensor should emit and how:

- **Vocabulary**: wire the remaining `acoustic_event` variants
  (transient, continuous, ambience_shift, state_change, cadence)
  through the existing emit path, each with a coalescing rule (one
  perception per state SHIFT, never per frame — a dishwasher starting
  is one event; a dishwasher running is zero).
- **Space**: locations make the home legible. Recurring located
  sources become claims ("the dishwasher lives at [x,y,z]", "the
  front door is the transient source at ...") — the acoustic map the
  entity tracker already half-owns, promoted from sensor state to
  shared understanding where it earns it.
- **Self-knowledge**: the sensor reports its own condition —
  calibration scores, muted mics, discovery changes — as
  perceptions, so the agent knows when its ears degrade (and can say
  so, or a rumination act can file a wondering about it).
- **The rubric**: the exploration's written deliverable is a
  perception-contract amendment — §6 gains sensor-quality criteria
  (coalescing discipline, salience_hint honesty, payload =
  observation never conclusion, self-reporting) that hollis is
  measured against and camera will be built against.

The salience lesson from first contact applies here in reverse: the
low levels are where the acoustic domain's *real* information lives —
the goal is emitting it at the granularity of meaning, not volume.

### H5 — Rumination: the idle mind works its garden (day 5)

Phase 4 gave us drive-budgeted epistemic acts; this phase gives the
drive a real repertoire, each act journaled as a curation trace
(which is already a curriculum kind — rumination trains the model
too):

- **Re-verify**: pick claims marked stale, re-read their referents,
  confirm/amend/retire. (The curator the memory system has waited for.)
- **Connect**: pick a claim neighborhood (recall), ask for an
  inference that FOLLOWS from them but is stated nowhere → candidate
  claim in a `notions` domain, low confidence, tagged `inferred`,
  never auto-promoted — the owner's memory-tab audit (bless/edit/
  forget) is the gate from notion to knowledge.
- **Wonder**: generate open questions from gaps ("the camera sensor
  is chartered but unbuilt; what would its binding be?") → surfaced
  on the mind tab as a small "wonderings" list — idea generation the
  owner can pick from, prune, or ignore.
- **Consolidate acoustics** (H4's act, same budget).

All acts run under the existing drive budget and frontier cooldown;
all products are auditable memory, never silent mutation. Rumination
respects `SALIENCE` being off (it is memory work, not judgment work) —
but its frontier spend rides the same accounting.

### H6 — One export to rule them (close of cycle)

`curriculum_export` v2 — half of it landed with spectrum S2: the
sweep already feeds the `salience-pairs` and `memory` stream
datasets, deduped and self-registering, with per-kind counts
reported. What remains for this cycle is extending the SAME feed
contract to the new banks — the `chat` kind into `chat-bank` (H1)
and the why-harvest's synthetic QA (H3) — plus the mind-tab card
showing the week's harvest: claims by domain and author, capture
volume, the banks' row counts (`agent-model-dataset_list` already
carries them), notions pending audit, and the bench's reports
beside them. What gets MEASURED gets grown; the gauge cluster is
mostly wired — this cycle points it at the harvest.

## What this cycle does not do (deliberately)

- No SFT training run fired by this cycle's phases — the bank
  accumulates. (The run itself is no longer future work: spectrum S8
  shipped it, gate design written first as this page demanded, and
  it rides the 8a user pointer protected by the same soak — exactly
  as predicted here. Filling the bank is this cycle's half of the
  handshake; firing `sft_run` on it stays a deliberate owner act.)
- No camera. H4's system sensor exercises the new-modality path
  cheaply first; camera follows the hollis template when hardware and
  appetite align.
- No autonomy expansion: every new act is drive-budgeted, journaled,
  and auditable; notions never self-promote to knowledge.

## Owner calls collected

0. The message library's name (`msgs`?) and whether hollis dual-writes
   transcripts (its own store + a message record) or the executive
   records them on perceive (H1; proposal: executive-side on perceive,
   keeping the sensor decoupled — hollis's store stays its own).
1. Capture default and hollis-text eligibility (H1).
2. Context budgets per profile, and whether chat surfaces
   auto-assemble context per turn (H2 — a per-surface setting,
   arm-agnostic; costs tokens, buys groundedness).
3. New domains: `environment`, `notions` — names and audit posture (H4/H5).
4. Rumination drive split: how many acts/hour go to re-verify vs
   connect vs wonder (H5; proposal 2/1/1).
5. Retroactive why-harvest depth: full git history or last-N-days
   (H3; proposal full — it is one-time and the corpus is ours).
6. Which low-level acoustic events emit by default once wired (H4;
   proposal: ambience_shift, state_change, and located transients on
   by default; per-frame anything, never).
