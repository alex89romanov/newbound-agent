# The harvest cycle — from environment to understanding to weights

**Status: plan of record for the next cycle — drafted 2026-08-17,
owner review pending.** Builds strictly on the shipped framework
(perception contract, federated memory, the salience tier, the
flywheel, the split pointer, the CLAUDECODE arm). Nothing here
replaces an existing organ; every phase feeds one that already runs.

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
allowed to be.

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
| curriculum_export → ingest → replay/heldout | CPT docs in the serving dialect | live |
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

### H1 — Bank the raw stream (day 1)

Extend capture into `chat_llm` itself, all arms: with `LLM_CAPTURE=on`
(botd, live key), every frontier call appends one structured JSONL row
— `{t, venue, arm, model, messages, tools, response, cost_usd}` — to
`runtime/agent/model/capture/YYYYMMDD.jsonl`. Instance-owned, never
committed, size-capped by daily rotation. This supersedes the
ask_llm-only Q/A text files (fold them in, same switch).

Then teach `curriculum_export` a `chat` kind: captured turns rendered
as nanochat chat-format conversations (`{"messages": [...]}`), banked
into `runtime/agent/model/sft/` — **separate from the CPT stream**.
The CPT trainer does not eat them yet; they accumulate as the
feedstock for the eventual chat-SFT refresh of the local model:
whatever frontier is serving, its in-domain answers become the local
model's future textbook, and the provenance tag lets that future
training run weigh sources as it sees fit.

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
  ("Q: why does bootstrap rewrite service.py? A: ...") into the SFT
  bank with provenance back to the patch.

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

`curriculum_export` v2: one command that sweeps every bank — salience
pairs (CPT, serving dialect), claims and curation traces (CPT),
captured chat + synthetic QA (SFT bank) — with per-kind counts
reported and a mind-tab card showing the week's harvest: claims by
domain and author, capture volume, SFT bank size, notions
pending audit. What gets MEASURED gets grown; this is the
gauge cluster for everything above.

## What this cycle does not do (deliberately)

- No SFT training run — the bank accumulates; the chat-SFT refresh of
  the local model is its own future phase with its own gate design
  (it will ride the 8a user pointer, protected by the same soak).
- No camera. H4's system sensor exercises the new-modality path
  cheaply first; camera follows the hollis template when hardware and
  appetite align.
- No autonomy expansion: every new act is drive-budgeted, journaled,
  and auditable; notions never self-promote to knowledge.

## Owner calls collected

1. Capture default and hollis-text eligibility (H1).
2. Context budgets per profile, and whether chat surfaces
   auto-assemble context per turn (H2 — a per-surface setting,
   arm-agnostic; costs tokens, buys groundedness).
3. New domains: `environment`, `notions` — names and audit posture (H4/H5).
4. Rumination drive split: how many acts/hour go to re-verify vs
   connect vs wonder (H5; proposal 2/1/1).
5. Retroactive why-harvest depth: full git history or last-N-days
   (H3; proposal full — it is one-time and the corpus is ours).
