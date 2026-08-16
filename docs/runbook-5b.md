# Runbook: Phase 5b — the resident model serves salience

**For the owner's GPU box. 2026-08-16.** Everything here except step 4
was verified end-to-end in-container with the stub scorer before this
runbook shipped: the executive's verdicts flowed through
`SALIENCE_CTL → agent-model-salience → HTTP → service.py`, a killed
service degraded the executive to no-verdict without missing a tick,
a restart resumed verdicts, `curriculum_export` produced a 173-sample
batch the trainer skeleton drained with the checkpoint ring rotating,
and a `/promote` double-buffer swap served through the transition.
The GPU day's genuinely new surface is one class: `NanochatScorer`.

Out of scope for 5b (Phase 6, later): actual CPT stepping, the replay
ratio and gate thresholds (owner calls, still open), the gated
user-facing pointer, the eval harness with auto-rollback, LoRA
re-derivation.

## 0. Prereqs

- The box has: a CUDA-capable GPU, python3 with torch, and a clone of
  nanochat.
- Pull masters and rebuild:

      cd ~/path/to/newbound && git pull
      (cd ../newbound-agent && git pull)   # or wherever the overlay lives
      ../newbound-agent/tools/setup.sh     # idempotent: overlay, build, stage

- Sanity: `./target/release/newbound mcp` answers, and
  `tools/nb-call.py agent-executive-status '{}'` returns `running: false`.

## 1. The base model — fresh nanochat run

Train the base from standard nanochat data per nanochat's own
instructions (the speedrun). This is the GPU-hours step; everything
after it is minutes. Note the resulting checkpoint directory — call it
`$CKPT`. (When curriculum export matures, the class-stamped seed joins
this mix; for 5b the standard run is the base.)

## 2. Start the service in stub mode first

Prove the wiring on this box before the model enters:

    cd ~/path/to/newbound
    python3 ../newbound-agent/tools/model-service/service.py \
        --data-dir runtime/model --port 8077 &

    curl -s http://127.0.0.1:8077/status
    # expect: {"status": "ok", "mode": "stub", "live_slot": "A", ...}

## 3. Point the seam at it

Append to `runtime/agent/botd.properties` (keep your existing LLM=
lines — the frontier stays the escalation judge):

    SALIENCE_CTL=agent:model:salience
    MODEL_SERVICE_URL=http://127.0.0.1:8077

Restart the newbound instance. Then:

    tools/nb-call.py agent-model-service_status '{}'
    # expect mode: stub — the command reaches the service through the store

Start the loop and watch a verdict arrive:

    tools/nb-call.py agent-sensor-start '{}'
    tools/nb-call.py agent-executive-start '{}'
    # touch any store record via dev.code, or just wait for real activity
    tools/nb-call.py agent-executive-status '{}'
    # expect last_context to carry salience + salience_why ("stub[...]")

This is the wiring proven with zero model risk. Every failure up to
here is plumbing, not ML.

## 4. Fill in NanochatScorer — the one GPU-specific edit

Open `tools/model-service/service.py`, class `NanochatScorer`. Two
marked blocks:

- `__init__`: load tokenizer + model from `self.checkpoint`, move to
  device, eval mode, delete the `NotImplementedError`.
- `score(perception, context)`: prompt the model with the perception's
  text/kind and the bound claims from `context`; ask for a 0..1
  salience with one sentence of reasoning; parse and clamp. Contract:
  return `(float, str)`. Keep it cheap — this runs at tick rate.

Restart the service with the real checkpoint:

    python3 ../newbound-agent/tools/model-service/service.py \
        --data-dir runtime/model --port 8077 --checkpoint $CKPT &
    curl -s http://127.0.0.1:8077/status   # expect mode: nanochat

## 5. The zero-executive-change test

Nothing else changes. No botd edit, no rebuild, no executive restart:

    tools/nb-call.py agent-executive-status '{}'
    # last_context.salience now comes from your weights;
    # salience_why is the model's own sentence

That's the acceptance criterion for the whole phase: the same
executive that ran on stubs is judged by the local model, and nothing
upstream noticed the swap.

## 6. Watch the curriculum write itself

With the loop running and the frontier configured, band verdicts
(0.35–0.65) escalate and disagreements land as training pairs:

    tools/nb-call.py agent-executive-salience_log '{}'
    # rows fill with {input, local, frontier, disagree} as activity flows

Export the feedstock into the trainer's intake and watch the skeleton
acknowledge it:

    tools/nb-call.py agent-model-curriculum_export \
        '{"path": "runtime/model/ingest/batch-day1.jsonl"}'
    # expect per-kind counts; within ~5s the service log prints
    # "[trainer] ... would step on N samples" and /status shows the
    # ring rotated

## 7. The degradation drill (do it once, deliberately)

    kill <service pid>
    tools/nb-call.py agent-executive-status '{}'
    # loop still running; last_context simply has no salience field
    # restart the service; verdicts resume on the next perception

The off switch, any time: remove the `SALIENCE_CTL` line and restart
the instance — salience is off, everything else unchanged.

## 8. Report back

Paste into the session: `/status` output in nanochat mode, one
`last_context` with a model verdict, `salience_log` totals after an
hour of activity, and the trainer's drain lines. Anything odd, include
`service.log` — diagnosis happens from the web session.

## Troubleshooting

- **`model service unreachable`** from the command: service not
  running, wrong port, or `MODEL_SERVICE_URL` typo. The executive is
  unharmed either way.
- **`scorer failed: ...`** in verdicts: your `score()` glue threw —
  the service caught it and answered err; the executive degraded. Fix
  the glue, restart the service only.
- **Verdicts feel constant/flat**: a fresh base is a weak judge —
  expected. The escalation log is where the correction signal
  accumulates; that's the flywheel's food, not a bug.
- **Port already in use**: a previous service instance survived —
  `kill $(cat service.pid)` style, by PID, never by pattern (a pkill
  pattern can match your own shell; the brain remembers).
