# Runbook: Phase 5b — the resident model serves salience

**For the owner's GPU box. Updated 2026-08-16: no seam, no manual
service management.** Salience is ON or OFF in settings, and the agent
builds its own nanochat server — install, script, launch are all
`agent.model.bootstrap`, which the executive fires itself whenever
salience is on and nothing is answering. Everything below except step 3
was verified end-to-end in-container before this shipped: the off
switch, the self-bootstrap (script written from the compiled-in asset,
service launched, verdicts flowing on the next perception), escalation
and audit rows, the degradation drill, the nanochat env install
(donefile-guarded clone + venv), and curriculum export draining into
the trainer skeleton.

Out of scope (Phase 6): actual CPT stepping, replay ratio and gate
thresholds (owner calls, still open), the gated user-facing pointer,
eval + auto-rollback, LoRA re-derivation.

## Settings (runtime/agent/botd.properties)

    SALIENCE=on                  # the whole subsystem; absent = off
    MODEL_CHECKPOINT=<path>      # optional; default stub (no install)
    MODEL_SERVICE_PORT=8078      # optional; default 8077
    NANOCHAT_REPO=<url>          # optional; default karpathy/nanochat

## 1. Wiring first, on stubs — no GPU involved

Pull masters, run `tools/setup.sh`, then set only `SALIENCE=on` and
restart the instance. Start the sensor and executive:

    tools/nb-call.py agent-sensor-start '{}'
    tools/nb-call.py agent-executive-start '{}'

`agent-executive-start` fires bootstrap eagerly, in the background —
install and launch happen at start, never lazily behind a perception.
Within a few seconds `runtime/model/service.py` exists, the service
answers, and the first perception already carries a verdict:

    tools/nb-call.py agent-model-service_status '{}'   # mode: stub
    tools/nb-call.py agent-executive-status '{}'       # last_context.salience

## 2. The base model — fresh nanochat run

Train the base from standard nanochat data per nanochat's own
instructions (the speedrun; this is the GPU-hours step). Note the
checkpoint directory — `$CKPT`.

## 3. Fill in NanochatScorer — the one GPU-specific edit

The service script is a **library asset**: edit
`data/agent/_ASSETS/service.py` in the repo checkout (NOT
`runtime/model/service.py` — bootstrap rewrites that copy from the
compiled-in asset whenever they differ; the glue belongs in git so
every instance gets it). Class `NanochatScorer`, two marked blocks:

- `__init__`: load tokenizer + model from `self.checkpoint`, device,
  eval mode; delete the `NotImplementedError`.
- `score(perception, context)`: prompt for a 0..1 salience plus one
  sentence of reasoning over the perception text/kind and bound
  claims; parse, clamp, return `(float, str)`. Keep it cheap — this
  runs at tick rate.

Rebuild the agent dylib (`cargo build --release` in `agent/` —
hot-reloads) and commit the asset + regenerated src on a branch.

## 4. Turn the checkpoint on

    MODEL_CHECKPOINT=/path/to/$CKPT

Restart the instance. Bootstrap now also builds the serving env
(clones `NANOCHAT_REPO` under `runtime/model/deps`, venv + deps,
donefile-guarded — one-time), rewrites the service script from the
new asset, and launches with your weights. Kill the old stub service
first if one is running (by PID — never by pattern).

    tools/nb-call.py agent-model-service_status '{}'   # mode: nanochat

**Acceptance — zero executive change:** the same executive that ran on
stubs now gets `last_context.salience` from your checkpoint, with the
model's own reasoning sentence. Nothing upstream was touched.

## 5. Watch the curriculum write itself

Band verdicts (0.35–0.65) escalate to the frontier; disagreements land
as training pairs:

    tools/nb-call.py agent-executive-salience_log '{}'

Export the feedstock; the trainer skeleton drains it within ~5s and
rotates the checkpoint ring:

    tools/nb-call.py agent-model-curriculum_export \
        '{"path": "runtime/model/ingest/batch-day1.jsonl"}'

## 6. The degradation drill (once, deliberately)

Kill the service by PID: the loop keeps running, `last_context` simply
has no salience field. Restart the executive (or call
`agent-model-bootstrap` yourself) and verdicts resume. The off switch,
any time: delete `SALIENCE=on`, restart the instance.

## 7. Report back

Paste: `/status` in nanochat mode, one `last_context` with a model
verdict, `salience_log` totals after an hour, the trainer drain lines
from `runtime/model/service.log`. Anything odd, include the log —
diagnosis happens from the web session.

## Troubleshooting

- **No verdicts, `SALIENCE=on`**: check `service_status`; then
  `runtime/model/service.log`. Bootstrap fires eagerly at executive
  start — restart the executive to retry, or run
  `agent-model-bootstrap` by hand (no parameters, `'{}'`) for the full
  report (`nanochat_env`, `script_written`, `service`). The manual
  call blocks through the whole install — that's it working.
- **`launch_failed` with a real checkpoint**: almost always the
  NanochatScorer glue (step 3) — the log shows the exact exception.
- **Verdicts feel flat**: a fresh base is a weak judge — expected.
  The escalation log is the correction signal accumulating; that's
  the flywheel's food, not a bug.
