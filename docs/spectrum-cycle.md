# The spectrum cycle — one substrate from d10 to the frontier

**Status: plan of record for the model-subsystem cycle — drafted
2026-08-18, revised same day after owner direction; all ten owner
calls ruled in session, 2026-08-18/19.**
Companion to `docs/harvest-cycle.md`, not its successor: the harvest
cycle grows the feedstock; this cycle grows the organism that eats it.
They interleave — S2 below is H6's landing zone, and S8 is the SFT
phase the harvest charter deliberately deferred. Builds strictly on
the shipped framework (the scorer seam, the CPT trainer and its gates,
the ring, the split pointer, the persona adapter, curriculum ingest).
Nothing here replaces an organ; every phase widens one.

## Why this cycle

The resident model subsystem is welded to one point on the scale axis:
a nanochat checkpoint small enough to hold twice on one card. The weld
is thin — the gates, the double buffer, the pointers, the soak, the
persona probe are model-agnostic already — but it is real, and it
shows in three ways:

1. **Only nanochat-format weights load.** `MODEL_CHECKPOINT` names a
   nanochat base dir; the scorer constructs nanochat's own GPT class
   and pickled tokenizer. Community nanochat checkpoints drop in
   today; a general open release (Llama, Qwen, Kimi, whatever ships
   next month) fails at load, and no converter can fix an architecture
   mismatch. The route is a second backend, not a translator.
2. **The trainer assumes the model fits twice.** The candidate is a
   deepcopy of the live model with fp32 AdamW moments beside it
   (`service.py` `trainer_real`). For a d20 that is under 12GB all-in
   — why today's design serves and trains on one consumer card. The
   assumption is nowhere written down, and it silently defines the
   subsystem's ceiling.
3. **Data is plumbing, not a subsystem.** Three hardcoded pool names,
   a replay file, `standard.txt`, and — critically — the forgetting
   guard anchored to the base dir's own pretraining shards, a luxury
   only a model born here has.

**The correction this cycle makes (owner, 2026-08-18): scale is a
spectrum, not a set of size classes.** The spectrum runs from a
nanochat d10 to the latest open frontier release; the hardware runs
from one 3090 to a cluster of B200s. The system must scale elegantly
across the whole range — same commands, same gates, same mind tab —
and no design decision may assume any particular box. Hardware
requirements vary drastically; *mechanisms* must not.

**And the aim, sharpened (owner, same day): the subsystem is also the
lab.** What we are building toward is a super-easy way to mix and
match datasets in and out; to build profiles and try them at
different sizes and across different architectures; to snap the parts
together like bricks — and, when a variant wins, to say *why*: was it
the training data or the architecture? Deriving purpose-built LoRAs
on demand is a first-class product of the subsystem, not a
persona-only trick. And the agent is its own richest supplier
(owner, same day): doing its agent thing, it throws off trainable
data continuously — and could throw off far more — including
material whose best use is seeding synthetic generation; all of it
must have an easy path into the same dataset interface (the feed
contract, S2). And above everything else: every function the
subsystem serves today keeps serving, and every phase pulls in the
same direction as the standing proposals.

## Standing rules for every phase

1. **Scale is a measured quantity, never a branch.** No mechanism may
   test model family, name, or a size class. One component — the
   posture solver (S5) — reads the resource map and the model's
   footprint and publishes its arithmetic; only its *output* varies.
   This extends the harvest cycle's standing rule to the resident
   itself: identity is provenance, scale is measurement, and neither
   is ever a branch condition.
2. **The gates are the constitution.** Promote, hold, reset, soak,
   rollback, the agreement measure, the forgetting guard, the persona
   gate — identical in *meaning* at every point on the spectrum.
   Scale may change how long a gate takes or what physically moves at
   promotion; it may never change what the gate measures or who is
   allowed to pass. The bench (S6) measures with the same instruments
   the gates enforce with — one vocabulary of quality, everywhere.
3. **Models, datasets, recipes, and adapters are products of the
   system — user data, never shipped** (owner ruling, 2026-08-18).
   The app ships mechanism and compiled-in defaults only, and
   bootstraps its own model; users import their own weights and grow
   their own datasets. User data lives where user data already lives:
   records in the runtime library (user settings and state, beside
   `salience_log`), files under the runtime folder. Nothing in the
   agent library, nothing in git. Imports are deliberate commands;
   nothing acquires data or weights on its own, and every imported
   artifact carries provenance (source, revision, hash) from the
   moment it enters.
4. **Every posture is published.** The solver's arithmetic, the chosen
   training posture, the placement, the anchor in use — all land in
   `service_status`, `metrics.jsonl`, and the mind tab. An override
   exists (`MODEL_POSTURE=`) and is published as loudly as a choice.
   Silent adaptation reads as magic until it reads as a bug.
5. **Compatibility is the first gate of every phase.** The shipped
   loop — stub → nanochat serve → CPT → gates → split pointer →
   persona — runs unchanged through every merge, on the same runbook
   steps. A phase that improves the lab by breaking the organism has
   failed its own gate.

## The shape of the answer

Two seams, one unification, and one unit of composition carry the
whole spectrum — and the last one is the lab:

- **The backend seam** (S3): everything the subsystem asks of a model
  — load, render the dialect, one loss step, generate, name adapter
  targets, save/load — becomes one interface with two
  implementations, nanochat and HF. Everything above the seam
  (gates, pointers, soak, persona, ingest) does not know which is
  serving. The zero-executive-change test, one level down.
- **The engine seam** (S7): token generation is in-process today and
  stays so wherever the model fits; where it doesn't, the same scorer
  fronts an external engine (vLLM-class, localhost). Pointer
  semantics — flip, soak, rollback — are identical; only the latency
  of a flip differs (an in-memory swap vs an engine reload).
- **The candidate is always base + delta** (S5): full CPT is the
  degenerate delta (the whole state dict — today's ring, unchanged on
  disk); a LoRA is a small one; frozen is the empty one. Ring
  checkpoints become `{base_ref, delta}`, promotion is apply-delta,
  rollback is a pointer move. One code path from d10 to the frontier;
  the solver only picks the delta type.
- **The recipe is the unit of composition** (S6): a named, declarative
  record — base (architecture × size), named datasets with mix
  weights, posture, hyperparameters, adapters, eval set. Everything
  the subsystem does becomes a recipe run: birth is the shipped
  nanochat speedrun recipe, the live CPT loop is a standing recipe,
  an experiment is a control recipe plus a variant differing in
  exactly one brick. Attribution falls out by construction — vary the
  data with the architecture held, or the architecture with the data
  held, and the bench scores both with the same measures on the same
  eval sets.

The fidelity ladder the solver walks, top rung that fits with declared
headroom (serving reservation + margin), never a model-name branch:

| Posture | Candidate | Fits when (roughly) |
| :-- | :-- | :-- |
| `full` | deepcopy + fp32 AdamW | params ×~20 bytes ≤ free VRAM (d10–d26 on a 3090) |
| `full-sharded` | FSDP across GPUs/nodes, optional CPU-offloaded optimizer | the same arithmetic over the whole map |
| `lora(r)` | frozen bf16 base + low-rank delta | base + serving headroom fit; rank from what remains |
| `qlora(r)` | 4-bit frozen base + low-rank delta | an 8B on a 3090 |
| `frozen` | none — serve and bank | everything else |

`frozen` is a posture, not a failure: the curriculum still banks, the
watchdog still measures, and the split pointer means the two lanes sit
at *different* points on the spectrum — a d20 salience judge training
`full` beside an imported 8B user pointer serving `frozen` on the same
card is the intended shape, not an edge case.

## The weld inventory (what S3 formalizes)

Every nanochat-specific touchpoint, all in `service.py` +
`bootstrap.rs`, ~250 lines of 1,648:

| Touchpoint | Today | Spectrum form |
| :-- | :-- | :-- |
| load/serve | `NanochatScorer` (`service.py:257`), `from_ring` (`:301`) | `Backend.load` behind `make_scorer` |
| dialect render | `render_for_completion` in `score()`, `gen_agreement`, `render_sample` | `Backend.render` — the one-dialect rule intact, unparseable-escalates unchanged |
| loss step | `model(x, y)` scalar forward (`:1181`) | `Backend.loss` |
| ring save/load | nanochat `save_checkpoint` (`:1238`) | delta save/load `{base_ref, delta}` |
| adapter targets | `lora_target_paths` (`:759`), hook-LoRA | `Backend.adapter` (PEFT on HF) |
| persona masking | `render_conversation` + `-1` ignore-index (`:944`) | `Backend.render_masked` (chat templates without assistant marks get a fallback) |
| context length | `meta["model_config"]["sequence_len"]` (`:1038`) | `Backend.context_len` |
| standard docs | base-dir parquet shards (`:998`) | the anchor dataset, via S2 |
| pools | three literals (`:1162`) | named pools from S2 |
| env build | nanochat pyproject venv (`bootstrap.rs:103`) | per-backend env recipe, same donefile idiom |
| distribution | `NANOCHAT_DIST` / `train.sh` | S7 placement |

## The phases

Ordered so the records land first (cheap, and today's d20 loop is
their first customer), the killer feature lands early (the persona
machinery already proves the adapter pattern), and the physics lands
last, on settled foundations. Each phase is a branch, a disposable
battery, and a merge word — the established rhythm.

### S1 — The model record and the resource map

Models become records: backend, source and lineage (born here vs
imported; `nanochat:cpt-…` vs `qwen3-8b:cpt-…` — the pointer-name
lineage story, generalized), tokenizer/template facts, footprint
(params, dtype, bytes), anchor dataset ref, provenance. The records
live in the runtime library — user settings and state, beside
`salience_log` — never in the agent library, never in git; the
commands that manage them are the shipped code.
`agent-model-import` is the deliberate door: bring an HF
snapshot or a nanochat base dir under management, hash it, demand an
anchor (S2). `MODEL_CHECKPOINT` survives as the degenerate alias — one
unregistered nanochat dir, exactly today's behavior.

Beside it, the **resource map**: a procedural probe (GPUs, free VRAM
each, interconnect, nodes — `nvidia-smi` facts plus placement config)
published like any sensor. First customers: the solver (S5) — and the
birth path, whose `NANOCHAT_TRAIN_ARGS` defaults were hand-sized for
one card and can now be sized from the map.

### S2 — The dataset manager

Named datasets as records in the runtime library: kind (`cpt` / `sft`
/ `eval` / `persona` / `anchor`), provenance, hash, row counts,
held-out policy, mix weight. Bytes under
`runtime/agent/model/datasets/` — the runtime folder, user files.
Commands in the `curriculum_export` idiom: `dataset_add` (local file
or HF hub — deliberate, never automatic), `dataset_list`,
`dataset_inspect`. The trainer's three hardcoded pools generalize to
named pools with weights (`parse_kv` already accepts arbitrary keys;
only the pools dict is a literal) — **mixing a dataset in or out of
the live loop becomes editing one weight**, a settings change, not a
code change. Multi-node shard placement follows the existing
`NANOCHAT_DIST` NFS guidance and becomes the manager's problem, not
the runbook's.

**The anchor rule** rides here: every model record names the dataset
its forgetting guard measures against, resolved by lineage (owner
ruling, 2026-08-18). A model born here — nanochat today, any
recipe-trained model tomorrow — anchors to a held-out slice of its
own training datasets, automatically and exactly: the recipe records
what trained it, so the anchor problem exists only for imports. An
import arrives without its data, so the import door MINTS one:
sample the model itself at import time (a few hundred generations
from varied prompts, frozen), measuring drift from its imported self
— distribution-free, offline, right for any model on the spectrum,
and frozen at import: what we teach it afterward is the standing
gates' business. An explicit anchor always overrides (an open-data
model whose corpus is public; the fineweb-edu sample ships as an
optional dataset recipe, not a default). A model without an anchor
cannot pass a gate; refusing to measure is not passing.

**The feed contract — the agent feeds its own lab.** Doing its agent
thing, the agent already throws off trainable residue on every
channel the harvest cycle inventories. Each channel, live or
chartered, terminates in a named dataset — and a channel that does
not exist yet is chartered to land there the day it does:

| Channel (harvest ref) | Dataset (kind) |
| :-- | :-- |
| salience escalations + audits (live) | `salience-pairs` (cpt) |
| claims + curation traces, incl. rumination's (live, H5) | `memory` (cpt) |
| persona corpus (live) | `persona` (persona) |
| captured frontier traffic (H1) | `chat-bank` (sft) |
| why-harvest, procedural + distilled (H3) | `code-why` (cpt + sft) |
| hollis transcripts + acoustic claims (H4) | `room` (cpt) |

**No orphan banks**: once this phase lands, a channel writing
trainable text anywhere but a managed dataset is a bug. H6's
export-v2 IS the sweep into datasets; the trainer drains datasets
rather than loose ingest files; H1's SFT bank is a dataset like any
other, waiting for S8.

**Streams and snapshots**: a live channel feeds a *stream* dataset —
append-only, rolling counts. The standing CPT loop may ride streams
(today's behavior, unchanged); an experiment or an SFT run pins a
*snapshot* — a frozen, hashed cut — so the bench compares like
against like and a result stays reproducible after the stream has
moved on.

**Derivation — synthetic data is a dataset operation.**
`dataset_derive` produces a new dataset from managed sources through
a declared generator: a procedural transform (the serving-dialect
rendering `render_sample` does today is exactly one), or a model —
the frontier arm or the resident itself — prompted over source rows:
distilled QA from why-harvest raw pairs (H3's distilled channel IS a
derivation), claim-grounded question synthesis, format augmentation.
A derived dataset records its lineage — source datasets and
revisions, the generator as a provenance tag (the harvest standing
rule: a tag, never a branch), the transform recipe — so "did the win
come from the raw data or the synthetic expansion?" is an ordinary
one-brick bench question between a dataset and its derived sibling.
Model-driven derivation spends tokens: it runs by deliberate command
or under a drive budget like rumination, never ambiently — and
governance is uniform across generators (ruled, 2026-08-19): frontier
arm, resident, or procedural transform obey the same rule, the
generator staying a provenance tag, never a branch.

### S3 — The backend seam

The weld inventory above, formalized: one backend interface, two
implementations. `HFScorer` beside `NanochatScorer` behind
`make_scorer`; serving, the user pointer, and the persona adapter
(PEFT) come up first — the user lane gets capable open bases early,
before the trainer generalizes. Acceptance is the same test 5b used:
zero executive change, and now also zero gate change — the same
`service_status`, the same mind tab, a different resident. The
dialect discipline is preserved verbatim: training, serving, and the
agreement gate speak one prompt dialect per model record, and an
unparseable verdict still escalates.

### S4 — Adapters on demand

The killer feature, and it is mostly already written:
`derive_adapter` (`service.py:870`) is the whole pattern — corpus →
LoRA → gate (must improve its subject by `min_gain`, must not
lobotomize the base past `guard`) → apply — welded to one corpus and
one lane. Promote it to a general organ: **adapters become named
records** (purpose, dataset ref, base ref, targets, rank, gate
report — in the runtime library, like every registry record),
derived by one command against any managed base —

    agent-model-adapter_derive name:<n> dataset:<d> base:<model> ...

— and applied to or removed from a pointer deliberately. Persona
becomes the first instance of the general mechanism rather than a
special case: its card, its probe, its re-derive loop unchanged. A
purpose-built skin — a coding adapter from H3's why-harvest, a
salience-dialect adapter, a per-entity voice — is derived in minutes
on the box that serves it, gated exactly as persona is today, and its
gate report travels with it. Stacking is ruled (owner, 2026-08-18):
allowed, and gated as a unit — merged additive deltas commute, so
there is no order; any change to the stack (add, remove, base
movement underneath) re-runs every member's subject probe with the
full stack applied, plus one standard-loss guard for the whole
stack, on the watchdog cadence persona already proves. A stack of
one is today's persona behavior, unchanged.

### S5 — The delta trainer and the posture solver

The heart. The candidate becomes base + delta; the solver walks the
fidelity ladder against the resource map and publishes its choice and
its arithmetic. "Fits" is defined (owner ruling, 2026-08-18): free
VRAM minus the computed serving reservation (weights + KV at serving
context + activation room) minus a 15% margin (a setting);
`MODEL_POSTURE=` forces a rung, published, and a forced posture that
does not fit refuses at launch with the arithmetic shown. Gates keep their meaning: the agreement measure runs
on generated verdicts whatever the delta type; the forgetting guard
reads the anchor; promote applies the delta (in-memory flip where the
model lives in-process); hold and reset discard it; the ring stores
deltas with base refs and prunes by a byte budget rather than a count
(five full d20 deltas are cheap; five full 8B deltas are 80GB;
adapters are noise), with two floors that hold regardless of budget:
the protected user-pointer set never prunes, and neither does the
newest entry — the ring is the restart-recovery path and never
empties. Today's ring layout is the `full` posture's disk
form, unchanged — a live instance upgrades in place.

**Two residents, ruled (owner, 2026-08-18): the lanes may diverge.**
Today the fast lane is the slow lane's canary because both pointers
draw from one lineage. Configuring a second model record
(`USER_MODEL=`) splits the lanes — a fresh install has one nanochat,
so one resident stays the default by construction — and the
generalization is that *soak is serving without authority*: the
user-lane resident earns its promotion by shadow-serving verdicts on
live perceptions (recorded, never steering, sampled) until it has
the same soak evidence the fast lane provides for free when lineages
match. The RAM price is paid on a ladder, not up front: **residency**
is solver-arbitrated per lane just as posture is —
`resident → quantized → offload → on-demand → external (S7)` — with
the fast lane holding priority (always-on, latency-sensitive,
continuously training) and the user lane degrading first (bursty,
human-paced: load per conversation, idle-unload, the judge keeps the
card). Nothing is refused for RAM; it degrades, visibly.

### S6 — The recipe and the bench

The lab. A **recipe** is a named, declarative record snapping the
bricks together: base (architecture × size), datasets with mix
weights, posture (or `auto` for the solver), hyperparameters,
adapters, eval set. `recipe_author` / `recipe_clone` make variants
one-edit cheap; **`agent-model-experiment`** runs a recipe — or a
control + variant pair — to a stated budget (steps or hours) and
scores every result with the instruments the gates already enforce
with: agreement on held-out pairs, anchor loss, per-dataset held-out
loss, persona loss where an adapter rides — on named `eval` datasets
that never train and are frozen per experiment for comparability. A
recipe may add eval datasets; it may never remove the standing
yardstick (ruled, 2026-08-18).
Reports are records; the mind tab gains a bench card: runs, the brick
that varied, the deltas per measure.

**The one-brick discipline** is how "was it the data or the
architecture?" gets answered by construction: an experiment names its
control and its single varied axis; the command warns — records
honestly, never refuses — when more than one brick moved. Size sweeps
are the same mechanism, not a feature: one recipe, a list of depths,
the solver sizing each run to the map — a d10/d20/d26 sweep is one
command on any box that fits, and an architecture comparison is the
same sweep with the base brick swapped. A raw-vs-synthetic
comparison is the same discipline again, applied to a dataset and
its derived sibling (S2).

The birth path folds in rather than duplicating: today's speedrun IS
the shipped recipe (`NANOCHAT_TRAIN_ARGS` becomes its knobs), and
bootstrap's train-if-empty behavior is running it. GPU governance on
one card: an experiment borrows the standing trainer's time-share —
candidate steps pause, serving never does, and the borrow is
published in `service_status`; with placement (S7), the bench takes
spare GPUs instead. Anything mutating shared state runs against a
disposable copy — the existing rule, unchanged.

### S7 — Placement and the cluster

Serve and train become placeable roles over the resource map. The
degenerate placement is today's: one process, one GPU, time-shared.
Then: same box, serve on GPU 0, train on 1..n; then multi-node
training (torchrun/FSDP — `NANOCHAT_DIST` generalizes into placement
config); then the engine seam's far end, where serving itself is an
external engine — installed only when configured, its own
donefile-guarded stage, never in the default environment — and a
promotion is an adapter hot-swap behind an unchanged pointer (what
the posture ladder trains at that rung is exactly what the engine
can swap live). Dialect fidelity through the wire is part of the
engine adapter's acceptance; unparseable-escalates applies to remote
verdicts unchanged. The service keeps its single HTTP front door and
its command surface at every placement; `/status` reports the map and
who is placed where. At the very top of the spectrum — a
frontier-scale MoE on a B200 cluster — serving is external and
training touches adapters only; nothing in the mechanism knows it is
big, and that is the point.

### S8 — SFT joins the loop

The phase the harvest charter deferred, chartered here: the banked
chat/SFT dataset (H1) trains the user lane — on an imported capable
base, likely the largest single quality jump the user pointer will
ever take. An SFT refresh is a recipe run whose product faces the
user gate: it rides the soak and the pointer machinery unchanged, and
its gate design gets its own written page before code, per the
harvest cycle's own deferral logic. Not scheduled ahead of S5 and S6
— it needs the delta trainer, the anchor rule, and the bench to
exist, because the first question about an SFT'd candidate is a bench
question.

## What elegant means, testably

Three reference boxes and one attribution demo, one acceptance bar —
same commands, same gates, same mind tab, and the only difference
between boxes is what the solver and placement publish:

- **R1 — one 3090, nanochat d20.** Today's behavior reproduced
  through the new machinery: solver lands `full`, ring bit-compatible
  with the live instance, no runbook step changes. (Standing rule 5:
  this is also every phase's first gate, not just the cycle's.)
- **R2 — one 3090, an imported 8B instruct.** Solver lands `qlora`;
  every gate fires with real numbers against the given anchor; user
  pointer promotes through soak; persona derives via PEFT.
- **R3 — a multi-GPU (then multi-node) box.** Placement separates
  serve and train; the same `service_status` shape reports it; a
  larger posture (`full-sharded`) is chosen by arithmetic the smaller
  box shows as rejected.
- **R4 — the attribution demo.** One recipe, two variants: one moves
  only the dataset mix, one moves only the base architecture. The
  bench card answers "which brick earned the delta" with the same
  numbers the gates enforce with — and a purpose-built adapter
  derived from one of those datasets ships from the same screen.

The demo at the end of the cycle is one settings change: point
`MODEL=` at a different record and watch the same organism wake up at
a different scale.

## What this cycle does not do (deliberately)

- No RL loop, no multi-resident ensembles, no router — at most two
  residents (one per lane), and only if the owner rules for it.
- No automatic model selection or acquisition: which weights enter is
  an owner call, made through the import door, every time.
- No autonomous experimentation: the bench runs what the owner (or a
  session, deliberately) queues; nothing self-experiments, and no
  experiment result promotes into a serving lane except through the
  standing gates. No hyperparameter auto-search this cycle — the
  bench makes sweeps cheap; it does not run them unasked.
- No abandonment of the birth path: bootstrap training its own model
  from nothing remains the default origin story; imports are adopted,
  and their lineage tags say so forever.
- No bespoke inference optimization beyond what the ladder needs
  (4-bit load is a posture; kernel work is not this cycle).

## Owner calls collected

1. **RULED (owner, 2026-08-18) — registry shape.** Models, datasets,
   recipes, and adapters are products of the system — user data —
   and live where user data already lives: records in the runtime
   library (user settings and state), bytes in the runtime folder
   (user files). Never in the agent library, never shipped: the app
   ships mechanism plus compiled-in defaults (the persona-seed
   precedent; the nanochat birth recipe ships the same way), and a
   fresh install bootstraps its own model. Rule of thumb for housing
   user data (owner): unless it needs table scans, it probably
   doesn't need a dedicated library. Settings: `MODEL=` names a
   registry record; `MODEL_CHECKPOINT` stays as the
   unregistered-directory alias (S1/S2/S4/S6).
2. **RULED (owner, 2026-08-18) — anchor by lineage.** Born-here
   models of any architecture anchor to a held-out slice of their own
   training datasets — the recipe knows what trained them, so this is
   automatic and exact, and the anchor problem exists only for
   imports. The import door mints a self-generated anchor by default
   (sample the model at import, freeze it); an explicit anchor always
   overrides; the fineweb-edu sample ships as an optional dataset
   recipe, not a default (S2).
3. **RULED (owner, 2026-08-18) — adapters stack, gated as a unit.**
   Merged additive deltas commute, so order is meaningless; what is
   gated is the combination: any stack change or base movement
   re-runs every member's subject probe with the full stack applied,
   plus one standard-loss guard for the stack. A stack of one is
   persona's shipped behavior, unchanged (S4).
4. **RULED (owner, 2026-08-18) — solver headroom as proposed.** The
   serving reservation is computed from the model record and serving
   context (weights + KV + activation room); a 15% margin rides on
   top as a tunable setting. `MODEL_POSTURE=` forces a rung and is
   published as loudly as a solver choice; a forced posture that
   does not fit refuses at launch with the arithmetic shown — the
   trainer's step-OOM skip stays the runtime backstop, never the
   plan (S5).
5. **RULED (owner, 2026-08-18) — ring prunes by byte budget.** 100GB
   default, a tunable setting, with two floors that hold regardless:
   the protected set (user pointer, last_good, ready) never prunes,
   and the newest ring entry never prunes — the ring is the
   restart-recovery path and must never empty. Disk is a
   resource-map fact: the solver warns when the budget exceeds free
   disk (S5).
6. **RULED (owner, 2026-08-18) — two residents, on a residency
   ladder.** The user lane may ride a different lineage: configuring
   a second model record turns it on (one resident stays the default
   by construction — a fresh install has one nanochat), and
   shadow-soak supplies the user-gate evidence when lineages
   diverge. Residency is solver-walked per lane like posture
   (resident → quantized → offload → on-demand → external), fast
   lane priority, on-demand the expected shape on tight cards.
   Nothing refuses for RAM; it degrades, visibly (S5/S8).
7. **RULED (owner, 2026-08-18) — bench etiquette as proposed.** The
   standing yardstick is held-out salience pairs + the anchor sample
   + per-dataset holdouts of what the recipe trained on, pinned as a
   frozen snapshot when the run starts; a recipe may add eval
   datasets, never remove the standing three. On one card an
   experiment borrows the trainer's time-share — candidate steps
   pause, serving never does, the borrow published; with placement
   the bench takes spare GPUs instead. One-brick violations warn and
   are recorded honestly, never refused (S6).
8. **RULED (owner, 2026-08-18) — external engine as proposed.** The
   seam lands in S3 (generation behind the scorer); the engine
   itself stays out of the default environment, its install a
   donefile-guarded bootstrap stage that runs only when an engine is
   configured. Dialect fidelity through the wire is part of the
   engine adapter's acceptance — unparseable-escalates applies to
   remote verdicts unchanged — and promotion at that rung moves
   adapters, not weights: hot-swap, which the posture ladder
   guarantees is what there is to move (S7).
9. **RULED (owner, 2026-08-18) — acquisition as proposed.** The only
   network touches in the subsystem are inside the two deliberate
   doors, `agent-model-import` and `dataset_add`; each takes a hub
   reference or a local path, and offline boxes lose nothing. Hub
   fetches are revision-pinned — the registry records the exact
   revision hash, never a floating latest; a newer revision is a new
   record with its own lineage, not a mutation of the old. No token
   by default; `HF_TOKEN` covers gated models and is read only by
   the two doors. Downloads land under the runtime folder,
   filelock-guarded and resumable, so concurrent multi-node imports
   dedupe over NFS (S1/S2).
10. **RULED (owner, 2026-08-19) — derivation governance and stream
    defaults as proposed.** Derivation runs by deliberate command
    always, and as a drive-budgeted rumination act once H5 lands —
    journaled, accounted, never ambient — with governance uniform
    across generators: frontier arm, resident, or procedural
    transform obey one rule, the generator a provenance tag, never a
    branch. The live CPT loop rides streams; the bench and SFT runs
    pin snapshots (S2/S6).
