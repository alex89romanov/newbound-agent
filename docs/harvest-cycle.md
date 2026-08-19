# The harvest cycle — from environment to understanding to weights

**Status: plan of record for the next cycle — drafted 2026-08-17;
amended 2026-08-19 after the spectrum cycle landed (S1–S8,
`docs/spectrum-cycle.md`); amended again 2026-08-19 after owner
review of that amendment (rulings folded in below, and four places
where the page described code that does not exist or coupling the
layering rules forbid).** Builds strictly on the shipped framework
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

**A second standing rule (owner, 2026-08-19): where user data
lives.** JSON that must never be checked in to git belongs in the
**runtime library** (`./data/runtime`); user **files** belong in the
runtime crate (`./runtime/<library_name>`). A dedicated library is
reserved for the rare case where a table scan over the JSON genuinely
earns the isolation. The existing user-data libraries are grandfathered
— they cost a step at every level and confuse the user base, but
refactoring them is not this cycle's work.

**A third standing rule (owner, 2026-08-19): sensor layering.**
Sensors plug in to agent, so a **sensor knows agent** — and the agent
library may never know about an individual sensor. The exception is
the built-in family (the codebase sensors, which ship with agent); a
hardware sensor may reasonably join it and be agent-native. Agent is
itself a plugin to `newbound_core`, so agent knows core — but
**`newbound_core` may not house an agent-requiring sensor**. The
distinction that makes this workable: *a probe is not a sensor*. A
probe gathers facts procedurally and depends on nothing; a sensor
binds and emits perceptions, which means calling `perceive`, which
means knowing agent. Core may house a probe. Only agent may house the
sensor that publishes it.

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
| OS/hardware state | probed by the resource map (spectrum S1) — read as data by the solver, never emitted as perceptions | **the sensor gap** (narrowed: a probe exists, a sensor does not) |
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
and the SFT bank each take their own copies. Instead: **one record per
message in the runtime library** — no new library (the user-data
standing rule above settles the draft's owner call in the other
direction). Messages are fetched by ID and never scanned, so they take
the general rule, not the table-scan exception.

**Shape.** `{id, t, role, venue, content, entity?, provenance}`, one
record *per message* — not one record holding a list. The
`salience_log` idiom (a single runtime record with a `rows` array) is
capped at 1000 for exactly this reason, and messages are the
provenance substrate every later claim and training row cites forever.

**Dedup is a choice, not a freebie.** The draft claimed identical text
would dedupe structurally "because the store is content-addressed".
Not by default: `set_data` writes to a path sharded from the **id**
(flowlang `datastore.rs`, `get_data_file` → `id[0:4]/…/id`), and ids
are system-minted unless the caller supplies one. Content addressing
IS available — supply the id, minted as a hash of the content (owner,
2026-08-19) — with one downstream effect to design around: hash the
whole record and no two occurrences ever collide, so nothing dedupes.
Real dedup wants the split — an immutable **content record** whose id
is the hash of its text, plus a thin **occurrence record** (`t`,
`role`, `venue`, `entity`, `provenance`) pointing at it. Then citing
"a message" means citing an occurrence and citing "the words" means
citing the content. Decide this at H1's start: retrofitting the split
once IDs are embedded in claims and training rows is expensive.

Everything downstream then references **IDs, not text**:

- capture rows become `{t, venue, arm, model, msg_ids[], reply_id,
  tools?, cost_usd}` — a few dozen bytes per call
- a conversation is an ordered ID list; export renders by join
- acoustic transcripts unify into the same space, by kind rather
  than by sensor (an utterance IS a message from an entity, venue
  `room`) — one message universe for chat, frontier traffic, and the
  household's speech, and a second acoustic sensor needs no new code
- claims and training pairs cite message IDs as provenance, so every
  future lesson can be traced to the words that taught it
- the H2 assembler draws messages by ID and never pastes duplicates
  into one context

**Then capture**, now cheap: with `LLM_CAPTURE=on` (botd, live key),
every frontier call through `chat_llm` — all arms identically —
appends one ID-referencing row to
`runtime/agent/model/capture/YYYYMMDD.jsonl`. This is a user file
under the runtime crate, per the standing rule — and it is not an
orphan bank, because it holds no trainable text: it is an index of
IDs, and the bank it feeds (`chat-bank`) is a registered dataset.
Folding in the ask_llm-only Q/A text files means *importing* them —
their text becomes message records, their calls become capture rows —
not letting loose text survive beside a managed dataset.

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
- recent acoustic reality — drawn from perceptions **by kind**
  (`acoustic_event`), and after H1 from the message records they
  become, never from a named sensor's own store: the sensor-layering
  rule forbids the agent library knowing about hollis specifically,
  and by-kind consumption is what makes the assembler work unchanged
  the day a second acoustic sensor or the camera exists
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
  ("Q: why does bootstrap rewrite service.py? A: ...") into
  **`code-why`'s sft slice** — the dataset the spectrum's own feed
  contract assigns this channel, not `chat-bank`. Keeping captured
  frontier chat and synthetic QA in separate datasets is what makes
  "did the win come from the raw transcripts or the synthetic
  expansion?" an ordinary one-brick bench question; mixing them
  forecloses it.

**What H3 must build, stated plainly.** The previous amendment said
this "IS `dataset_derive`'s model-driven mode". That mode does not
exist: `dataset_derive` ships the procedural transform only and
refuses everything else (`this branch ships one transform:
render_dialect`), its own header noting that "model-driven generation
arrives with its governed spender later". H3 lands it — for the whole
subsystem, not just for itself:

- **A generator axis.** Today `transform` is one string dispatched
  procedurally inside the service. Ruling 10 admits three generator
  classes; they do not share an address — procedural transforms and
  the resident run in the service, the frontier arm runs in the agent
  library behind `chat_llm`. Dispatch therefore forks on *where the
  generator executes*, which is not a model-identity branch (the
  standing rule holds), but is a second execution path to design.
- **The governed spender**, and a sequencing consequence: ruling 10
  wants deliberate-command spend always and drive-budgeted spend once
  H5 lands — but H3 runs on day 3 and H5 on day 5. So **H3 ships
  deliberate-command-only** with an explicit row/spend cap, and H5's
  rumination act calls it. That keeps the order and hands H5 a second
  act for free.
- **Kind and shape.** `dataset_derive` hardcodes `kind: "cpt"` on its
  output; distilled QA is conversation-shaped and lands in an `sft`
  slice, so derive must learn to emit both.
- **Snapshot pinning.** Ruling 10 pins snapshots for bench and SFT
  runs; a derivation off a live stream must pin its source at derive
  time or its lineage points at a moving target.

Sized honestly, H3 is a phase on the order of H1 — a procedural
harvester (free, retroactive) plus a real extension to a shipped
subsystem command.

This is the code-domain harvest matching the charter's "exceptional
at code first": the system explaining its own becoming, in trainable
form.

### H4 — The room yields claims; the box becomes a sense (day 4)

**Acoustic consolidation** — executive-side, NOT hollis-side (sensors
stay procedural): a drive-budgeted act that runs when conversation
subsides (cadence/occupancy signals the acoustic sensor already
emits). It takes the recent transcript window — perceptions of kind
`acoustic_event`, and the message records they become — plus the
entity claims the agent itself holds, never a reach into the sensor's
own library (the layering rule), assembles an H2
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
(the agent noticing its own GPU is busy is self-model).

**It reuses the probe it already has.** Spectrum S1 shipped the
resource map — `agent-model-resources` already shells `nvidia-smi`
for GPUs and reads free disk, and the posture solver consumes it. The
sensor must not stand up a second probe: **one probe, two consumers**
— the solver reads it as data, the sensor emits its
threshold-crossings as perceptions. The probe grows RAM, load,
temperature and service liveness once, and both consumers gain them.
Two probes reporting one box eventually disagree, and reuse is the
standing priority.

Housing follows the layering rule: the sensor is **agent-native**,
joining the built-in family, which is legal precisely because it
ships with agent. Should the probe ever gain a non-agent consumer it
may descend into `newbound_core` as pure sysinfo — legal because
nothing that calls `perceive` moves with it — but there is no second
consumer today, and the move costs a host rebuild and a restart.

*One claim narrowed:* a built-in sensor exercises the envelope, the
kind registry and binding — it does **not** exercise the plugin path
(discovery, `Command::lookup` delegation, the missing-agent counted
skip). Only hollis tests that. Keeping this sensor built-in is right
— it buys the solver's direct access — but the page should not claim
it re-proves the plugin route.

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
and the why-harvest's into `code-why` (H3) — and here the word
"contract" flatters the code: today it is an inlined loop over a
literal `[("salience-pairs", true), ("memory", false)]` with a
*boolean* discriminator, plus some fifty lines of registration, dedup
and registry re-render that `dataset_add` carries its own copy of.
The bool does not generalize past two streams, and copies three and
four are what H1 and H3 would write. **So H6's factoring comes
first** — one feeder owning row counts, holdout policy, dedup and
registration, called by every channel — or the banks' semantics drift
apart. (Reducing the codebase is the standing priority; this cycle
grows it, so every growth pays for itself in reuse.) Plus the
mind-tab card
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

0. **RULED (owner, 2026-08-19) — no message library.** Messages are
   user JSON: records in the runtime library, per the user-data
   standing rule; `msgs` survives at most as a record-id namespace.
   The dual-write half is settled by the layering rule rather than
   separately: the **executive records on perceive**, because that
   keeps the message store sensor-agnostic — any sensor's utterances
   become messages by kind, with no hollis-specific code in agent —
   and hollis's own store stays its own. What remains open is
   internal to H1: whether ids are content hashes, and whether the
   content/occurrence split is taken up front (see H1).
1. Capture default and hollis-text eligibility (H1).
2. Context budgets per profile, and whether chat surfaces
   auto-assemble context per turn (H2 — a per-surface setting,
   arm-agnostic; costs tokens, buys groundedness).
3. New domains: `environment`, `notions` — names and audit posture (H4/H5).
4. Rumination drive split: how many acts/hour go to re-verify vs
   connect vs wonder (H5; proposal 2/1/1).
5. Retroactive why-harvest depth: full git history or last-N-days
   (H3; proposal full — it is one-time and the corpus is ours).
   Note H3's scope grew: it now lands `dataset_derive`'s model-driven
   branch and its spender, deliberate-command-only, with the
   drive-budgeted half arriving in H5.
6. Which low-level acoustic events emit by default once wired (H4;
   proposal: ambience_shift, state_change, and located transients on
   by default; per-frame anything, never).

**Also ruled 2026-08-19, and folded in above rather than left as
calls:** synthetic QA lands in `code-why`, not `chat-bank` (H3); the
feed contract is factored before it is copied again (H6); the system
sensor reuses the resource map's probe and stays agent-native, and
`newbound_core` may house a probe but never an agent-requiring sensor
(H4, standing rule 3).
