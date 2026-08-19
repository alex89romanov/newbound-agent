# Spectrum S4 — adapters on demand

**Status: implementation notes for branch `claude/spectrum-s4-adapters`,
from the ruled charter (`docs/spectrum-cycle.md` S4, ruling 3).** The
persona pattern — corpus → hook-LoRA → gate → apply — promoted to a
general organ: named adapters derived from any registered dataset,
applied to the user pointer as a stack, the COMBINATION gated whole.

## The shape

- **Derive** (`agent-model-adapter_derive name dataset base targets
  rank steps`): corpus from a registered dataset (persona-shaped jsonl;
  its holdout policy splits train/held-out, 0 falls back to every
  5th — a derivation must hold something out for its gate to mean
  anything). Base is `pointer` (the serving user pointer's weights,
  fresh copy) or any registered model. Training and the gate run in
  the service (`/adapter_derive`, blocks through the run like
  persona_rederive); the knobs nobody passed come from `USER_LORA` —
  one knob set, the owner's. The gate is persona's: `min_gain` on the
  subject's held-out loss, `guard` on standard loss. Accept → blob in
  the managed adapters dir + a record (gate report verbatim) in the
  runtime library. **Deriving never applies.**
- **Apply/unapply** (`agent-model-adapter_apply name on:true|false`) —
  ruling 3 executed: the service rebuilds the user scorer from a FRESH
  base + persona + every member (merged additive deltas commute, so
  there is no order; rebuilding beats subtracting in bf16), then gates
  the combination — every member's subject held-out loss with the full
  stack applied must still clear `min_gain` against the fresh ground,
  one standard-loss guard covers the whole stack. Pass → the stack
  serves and persists (`adapters/stack.json`, re-applied on promotion,
  rollback, and restart); fail → serving untouched, the numbers name
  the member that broke.
- **Watch**: the user-gate loop probes each applied member's subject
  loss on the SERVING model against its derivation baseline every
  `check_s`; slip past `slack` is surfaced in `/status.adapters` and
  the metrics, never silently acted on — unapply is the owner's
  deliberate act. `agent-model-adapters` lists the records;
  `agent-model-adapter_delete` removes record + blob and refuses while
  applied.
- **Persona** stays the standing first skin: the stack stands on
  base+persona ground, its own probe/re-derive loop untouched (R1).
  Folding persona fully into the records-based mechanism is deferred
  to the S5 refactor that touches this code anyway.

## Battery (CPU, tiny model — passed 2026-08-19)

Derive from a 12-row registered sft dataset against the hf pointer
(gate: 6.21 → 5.95 held-out, accept); apply → stack of one serving as
`+lora+adp` (persona coexisting beneath); second derive + apply →
stack of two, both members re-cleared the unit gate on fresh ground;
delete refused while applied; gated unapply back to one; delete with
blob cleanup; service restart → stack re-applied and named correctly
(a marker bug found and fixed in-battery: `+lora`/`+adp` suffixes
were endswith-checked and broke once stacked). R1 restored to stub.
Owner-box battery owed: a real model, real corpora, and the stack
probe observed across a real base promotion.

## Merge word

`Merge: S4 - adapters on demand`.
