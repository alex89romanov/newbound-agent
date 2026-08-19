# Spectrum S5 — the delta trainer and the posture solver

**Status: implementation notes for branch
`claude/spectrum-s5-delta-trainer`, from the ruled charter
(`docs/spectrum-cycle.md` S5, rulings 4 and 5).** The heart: the
candidate becomes base + delta, and one solver's published arithmetic
decides how it trains.

## What landed

- **The posture solver** (ruling 4): at trainer start, fits = free
  memory (GPU 0 when present, /proc/meminfo on a CPU box — measured,
  never assumed) minus a margin (`MODEL_HEADROOM=`, default 15% of
  total). The ladder on this branch: `full → lora(r) → frozen`;
  `full-sharded` waits for S7 placement, `qlora` for a
  quantization-dependency ruling, and `full` stays nanochat-only until
  the ring learns full hf saves. `MODEL_POSTURE=` forces a rung; a
  forced rung that does not fit REFUSES training loudly — posture
  reads `refused: <why>` with the arithmetic on status, serving
  untouched — rather than OOMing at step one. Everything publishes:
  `trainer.posture` and `trainer.arithmetic` (params, weights_mb,
  free_mb, margin_mb, per-rung needs, source).
- **The lora rung**: CPT as base + delta on the SERVING model's own
  weights — zero-copy. The delta is hook-LoRA gated per-thread
  (`TRAIN_TLS`): only the trainer thread sees the candidate; serving
  threads never do. Same drain, same mix and dataset pools, same
  gates (agreement on held-out pairs when they exist, the forgetting
  guard against anchor/standard, per-dataset losses reported).
  Promotion MERGES the delta into the serving weights, rings the
  blob with its base ref (kilobytes, not gigabytes), and starts a
  fresh delta; reset zeroes it. Restart resume re-merges the delta
  ring chronologically (each delta was trained relative to base plus
  every earlier merge, so the reconstruction is exact); pointer keys
  naming delta checkpoints load the same way. **This is CPT on an hf
  resident** — the rung S3 left frozen.
- **The ring byte budget** (ruling 5): `MODEL_RING_GB=` (default 100)
  replaces the count prune, with the two ruled floors — the protected
  user set never prunes, the newest entry never prunes. Full
  checkpoints and delta blobs meter the same budget.
- **The full rung** is the shipped deepcopy path, byte-for-byte (R1),
  entered when the solver says so.

Deferred, deliberately: `USER_MODEL=` two-residents with the residency
ladder (ruling 6) — its own branch; `full-sharded` (S7); qlora (needs
a quantization dependency the doctrine must rule on).

## Battery (CPU, tiny model — passed 2026-08-19)

Solver on meminfo chose `lora` for the hf resident (arithmetic
published: need 500MB + 2245MB margin against 14973MB free); the
trainer trained the serving model live on the `memory` stream's pool
(23 steps in the first half-minute), gated every 6 steps with
per-dataset losses in the gate rows, and merge-promoted five times to
`hf:tiny-hf:cpt-30` with 19KB delta blobs in the ring. Stop/relaunch
resumed at `cpt-42` — the ring reconstruction carries CPT across the
off/on cycle. `MODEL_POSTURE=full` on the hf resident refused loudly
with the reason on status, serving untouched. R1 restored to stub.
Owner-box battery owed: the same arc on a real model, the full rung
regression-checked on nanochat, and a budget-sized ring prune
observed.

## Merge word

`Merge: S5 - the delta trainer and the posture solver`.
