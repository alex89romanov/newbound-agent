# Spectrum S6 — the recipe and the bench

**Status: implementation notes for branch `claude/spectrum-s6-bench`,
from the ruled charter (`docs/spectrum-cycle.md` S6, rulings 7 and
10).** The lab: recipes snap the bricks together, the bench answers
"was it the data or the model?" by construction.

## What landed

- **Recipes** — the lab's unit of composition, records in the runtime
  library: base (`pointer` or a registered model) × datasets with mix
  weights × posture × budget × lr × evals, pure declaration, every
  brick validated against the registry at authoring. `recipe_author`,
  `recipe_clone` (edits object — one edited brick is a clean
  experiment), `recipes`, `recipe_remove`.
- **`agent-model-experiment`** — resolves control (+ variant) recipes,
  diffs their bricks (base, mix, posture, lr, evals, steps; notes is
  free text, not a brick), and hands the resolved recipes to the
  service. More than one moved brick WARNS with the list and is
  recorded in the report — never refused (ruling 7). Identical
  control/variant warns that it measures run-to-run noise.
- **The runner** — each arm is a fresh load of its base, a bare
  measurement, a bounded always-on hook-LoRA delta trained on the
  arm's mix, a with-delta measurement, weights freed. Instruments are
  the gates' own: per-dataset held-out loss, anchor/standard loss,
  agreement on held-out pairs. Eval material is PINNED at run start
  with dataset hashes recorded in the report — a stream honestly says
  `rolling`, a snapshot names its hash — so the report says exactly
  what was measured (ruling 7).
- **The borrow** — while an experiment runs, both standing trainer
  rungs skip their candidate steps (`EXPERIMENT["running"]` guard);
  serving never pauses; `trainer.borrowed_by` publishes who has the
  time-share. With S7 placement the bench takes spare GPUs instead.
- **Reports** — appended to `runtime/agent/model/experiments.jsonl`
  (user data, the runtime folder); `agent-model-experiments` returns
  the live run plus the last ten reports; `/status.experiment` carries
  the live view for the mind tab's bench card.

## Battery (CPU, tiny model — passed 2026-08-19)

`bench-a` (mix `memory=1.0`) cloned to `bench-b` (mix
`memory-docs=1.0`) — the experiment diffed exactly `['mix']`,
`one_brick: true`. The run pinned `memory: rolling` and
`memory-docs: <hash>`, ran both arms fresh-base with their own pools
(191 docs each), and reported before/after per eval. The two eval
sets scored identically — correct, and itself a bench finding:
`memory-docs` IS the render-dialect derivation of `memory`, so their
holdouts are the same text. A two-brick clone (mix + lr) warned
"attribution will be confounded" with the moved bricks listed, ran
anyway, recorded honestly. Validation refused an unregistered mix
dataset. The standing trainer resumed stepping after each run. R1
restored to stub. Known nit for the backlog: a slow first hf load can
outlive bootstrap's 20s probe, reporting `launch_failed` for a
service that comes up seconds later.

Owner-box battery owed: an experiment at real scale where the arms
genuinely diverge, and the borrow observed across a long run.

## Merge word

`Merge: S6 - the recipe and the bench`.
