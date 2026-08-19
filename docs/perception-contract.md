# The perception contract

**Status: design of record, v1 — 2026-08-15** (one-memory cycle Track
B1). This contract ships with the agent library and is consumed by
`agent.executive` (Track B2). Providers implement it from outside: the
built-in codebase sensor family is the reference *implementation*
(landed 2026-08-15: `agent.sensor`, the journal tailer of §5, on
claude/phase2-sensor), hollis is the reference *plugin*, camera
follows. Governing intent
(owner, 2026-08-15): the paradigm must be **exceptional at code first**
and expand to all non-code environmental input — situational awareness
is bounded by this contract, not by the codebase.

A perception is a typed, timestamped observation, delivered to the
executive's input queue, **already attached to the beliefs it touches**.
Sensors are procedural (understandingloop.md commitment 2): no model
cycles are spent on the derivable, and the LLM's work starts where
derivation ends — adjudicating what a change *means*.

---

## 1. The envelope

Modality-agnostic; only payloads are modal. Every perception is one
JSON object:

```json
{
  "v": 1,
  "kind": "store_change",
  "time": 1765822800000,
  "sensor": "store",
  "payload": { },
  "claims": [
    { "lib": "dev", "ctl": "code", "claim": "list_control_patches returns newest-FIRST ...", "stale": true }
  ],
  "salience_hint": 0.7
}
```

- **`v`** — envelope version, frozen at 1. Evolution is additive (new
  kinds, new optional fields); a breaking change requires a new major
  and a deliberate migration.
- **`kind`** — one of the registered kinds (§4). Unknown kinds are
  queued, not dropped: the executive treats them as opaque
  low-salience perceptions, so a new sensor works before anyone
  teaches the executive its vocabulary.
- **`time`** — epoch milliseconds, *sensor-observed* time (when the
  thing happened, not when it was delivered).
- **`sensor`** — the providing sensor's stable id (`store`, `fs`,
  `peer`, `text`, `hollis`, `camera`, ...). One id per sensor, not per
  event source; the payload carries finer origin.
- **`payload`** — modality-defined object; each kind documents its own
  schema (§4). Payloads carry observations, never conclusions: "RMS
  rose 12dB at loc [x,y,z]" is a payload; "someone arrived" is a claim
  the executive may or may not derive from it.
- **`claims`** — the **binding** (§2): references to existing claims
  this perception touches, each `{lib, ctl, claim}` (the claim's exact
  text is its identity, per the archivist's dedupe key) plus optional
  per-binding flags (`stale: true` when the perception invalidates the
  claim's source hash). May be empty — an unbound perception is still
  deliverable; binding is best-effort and procedural, never a gate.
- **`salience_hint`** — optional, advisory only, 0.0–1.0: the sensor's
  own prior ("a transcript outranks a fan turning on"). The salience
  tier may use it as a feature; it never substitutes for the judgment.

## 2. Binding — each sensor owns its binding function

Perceptions arrive attached to the beliefs they touch. *How* they get
attached is modality-specific; *that* they arrive attached is the
contract:

- **Code** (built-in): exact and procedural — a changed store record is
  joined to every claim whose source pointer `{lib, ctl, facet, hash}`
  covers it; the join is a staleness-hash comparison, and a mismatch
  sets `stale: true` on the binding. This is the reference binding: no
  heuristics, no model.
- **Hollis**: voiceprint→entity resolution — the sensor's perceptual
  state (acoustic signatures) resolves a speaker, and the binding
  targets the claims that name that entity (e.g. the signature→"Marc"
  binding claim in the brain).
- **Camera**: its visual-signature mapping, same shape, later.

**Sensor-state vs. claim is a contract clause.** The sensor keeps its
perceptual state — journal cursors and watch positions for code,
voiceprints and array geometry for hollis, embeddings for camera —
private, local, and disposable-in-principle. The store keeps the
meaning. A sensor never writes claims directly; it *proposes*
perceptions, and the executive's adjudication writes memory (under the
hysteresis rule: a flaky sensor moves a claim's confidence, never
toggles the claim).

## 3. Delivery

- **The sink** is a command: `agent.executive.perceive`, taking the
  envelope as its single declared param (`perception`). It validates
  the envelope, enqueues, and returns — it does not block on
  processing, and it does not journal (perceptions are sensory flow,
  not memory; what deserves history reaches the store as claims, and
  the store's own journals then re-observe it — the loop's self-model).
  The command lands with Track B2; until then this section is the
  specification it is built to.
- **In-process sensors** (the built-in family, running as executive
  tasks) may enqueue directly — same envelope, no command overhead.
  Plugin sensors dispatch the command (`Command::lookup` in-process for
  dylib plugins; the peer route for remote sensors — a peer-delivered
  perception is distinguishable by its `sensor` id and subject to the
  same trust posture as any foreign-authored input).
- **Coalescing is the sensor's first responsibility and the
  executive's backstop.** A rebuild touches thousands of records; a
  conversation produces one transcript, not four hundred audio frames.
  Sensors deliver at the granularity of *meaning* (one perception per
  journal entry, per utterance, per state shift — never per token, per
  frame, per sample). The executive additionally coalesces bursts by
  binding key before orientation. Perception storms are a sensor bug
  by definition.
- **Ordering**: best-effort per sensor, none across sensors. `time` is
  the reconciliation field.

## 4. The kind registry

### Built-in kinds (the codebase sensor family, ships with agent)

| kind | payload | notes |
|---|---|---|
| `store_change` | `{lib, ctl, id, facet?, patch: {id, label, author}?}` | From the store's `_patches` journals — the change feed of the entire object graph, and the reference implementation (§5). Because the executive acts only through platform commands, its own acts return through this kind: the self-model. |
| `file_change` | `{path, op: created\|modified\|removed}` | Checkout files outside the store (docs, manifests, build outputs). |
| `peer_event` | `{peer, event, detail?}` | P2P layer observations: peer arrival/departure, library installs, sync activity. |
| `text_input` | `{text, source, speaker?}` | Direct textual address (the donor repo's `hear`, chat surfaces). |
| `system_state` | `{metric, value, band, prev_band, detail?}` | **[landed 2026-08-19, harvest H4]** The box sensing itself: disk, RAM, load-per-cpu, GPU headroom, service liveness. Emitted only on band CROSSINGS (the coalescing rule made structural); first sweep seeds silently. Reads the resource map (`agent-model-resources`) — one probe, two consumers: the posture solver takes it as data, this sensor emits its transitions as perceptions. Payloads are observations; "disk will fill by Friday" is a claim the executive may infer. |

### Plugin kinds

| kind | payload | notes |
|---|---|---|
| `acoustic_event` | `{event, text?, entity?, location?, label?, db?, delta_db?, confidence?, duration_ms?}` | Hollis (§5). `event` discriminates: `transcript`, `transient`, `continuous`, `ambience_shift`, `state_change`, `micro_transient`, `cadence`, `occupancy`. |
| `visual_event` | *reserved* | Camera. Shape to be specified against this contract when camera lands; the acceptance criteria (§6) apply to it unchanged. |

New kinds are added by documenting them here and shipping a sensor that
emits them. **Nothing else changes** — see §6.

## 5. Reference implementations

### The journal tailer (built-in; reference implementation)

Tails every library's `_patches` journals from a persisted cursor
(sensor state). Each new journal entry becomes one `store_change`
perception; binding scans claims whose source pointers name the changed
`lib.ctl` facet and compares stored hash to current content hash
(`stale: true` on mismatch). Purely procedural end to end — the bar
every other sensor's *mechanism* is measured against, even when their
bindings are soft.

### Hollis (plugin; reference plugin) **[landed 2026-08-17]**

The transcript row of the mapping below is live: the cortex's
Transcript arm calls `emit_acoustic_perception` (home:
`hollis.audio.inject`, which doubles as the manual test surface),
binding via the agent's own recall on the resolved entity label and
delivering through `perceive` by `Command::lookup` — a missing agent
library is a counted skip, so hollis still runs standalone. The
`emit=off` key in `system.apps.hollis.runtime` is the sensor's off
switch. Remaining rows follow the same emit path as they are wired.

Hollis's `SemanticEvent`/`EventKind` enum is ~80% of `acoustic_event`
already; the mapping, which also retires its dead variants into
contract counterparties:

| hollis `EventKind` | `acoustic_event.event` | payload mapping |
|---|---|---|
| `Transcript { text }` | `transcript` | `text`; `entity` from the resolved speaker; `location` from the track |
| `Transient { label, confidence, peak_db }` | `transient` | `label`, `confidence`, `db` = peak_db |
| `Continuous { label, is_speech }` | `continuous` | `label`; speech-ness folds into `label`/`confidence` |
| `AmbienceShift { delta_db, new_floor_db }` | `ambience_shift` | `delta_db`, `db` = new floor |
| `StateChange { previous_db, current_db }` | `state_change` | `delta_db` derived, `db` = current |
| `MicroTransient { label, peak_db, margin }` | `micro_transient` | `label`, `db`, `confidence` from margin |
| `CadenceUpdate { state, duration_ms }` | `cadence` | `label` = state, `duration_ms` |
| `ContextUpdate(briefing)` | — | retired: briefings were the vestigial cortex's conclusions; conclusions are the executive's job now |

The donor repo's `{millis, sink}` `STT_CTL` contract survives for raw
listen-and-transcribe delegation; hollis's *semantic* stream rides
`perceive` with the annotations above, so nothing rich is flattened to
text at the boundary.

## 6. Acceptance criteria

1. **The zero-executive-change test.** Adding a modality requires a
   new payload kind in §4 and a new binding function in the sensor —
   and *no change to the executive*. If the envelope cannot express a
   code perception crisply, fix the envelope; never special-case a
   sensor. (Unknown-kind tolerance in §1 is this test's runtime half.)
2. **Per-sensor binding.** A sensor that cannot bind still conforms
   (empty `claims`), but a sensor that *can* bind procedurally must:
   unbound perceptions spend model cycles on the derivable, which
   commitment 2 forbids.
3. **Exceptional at code.** The built-in family is the quality bar:
   every contract change is validated against the code sensor first,
   and a change that would degrade code perception to accommodate
   another modality is rejected — widen, never weaken.
4. **Sensor quality — the rubric** (harvest H4's written deliverable;
   hollis is measured against it, camera will be built against it):
   - **Coalescing discipline.** One perception per state SHIFT, never
     per frame or per sweep: a dishwasher starting is one event, a
     dishwasher running is zero, a full disk staying full is zero. A
     sensor that cannot say what changed since its last emission is
     not done being written. Discrete events (a transient, an
     utterance) coalesce by repetition window instead: the same label
     from the same source within the window is one perception.
   - **Salience-hint honesty.** The hint is the sensor's own prior and
     nothing else. A sensor must not learn to inflate it: hints are
     auditable against the judged salience of the same perceptions,
     and a sensor whose hints diverge consistently loses the hint (the
     tier ignores it) rather than the queue.
   - **Payload = observation, never conclusion.** "RMS rose 12dB at
     [x,y,z]" is a payload; "someone arrived" is the executive's
     inference. The moment a sensor starts concluding, its conclusions
     stop being auditable against anything — hollis losing its LLM is
     the precedent, not an incident.
   - **Self-reporting.** A sensor's own condition is part of the
     environment: calibration drift, a muted microphone, a discovery
     change, a stalled probe are perceptions like any other (emitted
     under the same coalescing discipline), so the agent knows when
     its senses degrade and can say so. Chartered for hollis
     (calibration health, mic state); the system sensor's
     service-liveness band is the first live instance.
   - **Emit defaults are declared, not discovered** (owner call 6,
     proposal): `ambience_shift`, `state_change`, and located
     `transient`s on by default; `continuous` emits on source START
     only; `micro_transient` and `cadence` off by default;
     per-frame anything, never. Each event's gate is a runtime key
     (`emit_<event>=on|off`) beside the sensor's master `emit` switch.

## 7. What this contract does not cover

Adjudication (executive, Phase 3), salience (Phase 5), the epistemic
work queue (Phase 4), and memory formation (the archivist) are all
downstream of the queue this contract feeds. A sensor's obligations end
at delivery; a consumer's begin there.
