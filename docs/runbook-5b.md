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

**Phase 6 is in (2026-08-16): the trainer is real.** When a nanochat
checkpoint serves and `MODEL_TRAIN=on` (the default), a candidate copy
of the live model steps continuously on mixed mini-batches - fresh
curriculum from ingest, a replay reservoir, standard pretraining data -
and every `MODEL_GATE every=` steps it faces the gate: no regression on
held-out standard data (forgetting guard), and — once at least 4
held-out salience pairs have accumulated — GENERATED-verdict agreement
with the frontier's labels (6b: the gate measures the actual job;
candidate agreement must be within `agree_slack` of live, over
`agree_n` pairs; before that, a loss proxy on held-out curriculum).
The service journals every served verdict, loss samples, and every
gate to `runtime/agent/model/metrics.jsonl` (self-capped);
`agent-model-metrics` serves the mind tab's trends from it. Pass -> ring
checkpoint + promotion through the double buffer, zero serving
interruption; fail -> hold, and after `fails=` consecutive holds the
candidate resets to the live weights. Settings (defaults are proposals
- the replay mix and gate thresholds are owner calls):

    MODEL_TRAIN=on                                  # off = 5b behavior
    MODEL_MIX=fresh=0.25,replay=0.25,standard=0.5   # replay ratio
    MODEL_TRAIN_LR=2e-5
    MODEL_GATE=every=50,regress=0.02,fails=3,agree_slack=0.05,agree_n=8
    MODEL_TRAIN_INTERVAL=10                         # seconds per step

Watch it: `service_status` carries a `trainer` block (steps, loss_ema,
gates, promotions, resets, replay_size, last_gate with both eval
pairs). Feed it: `agent-model-curriculum_export` into
`runtime/agent/model/ingest/`. The serving pointer name shows the
lineage: `nanochat:sft` at birth, `nanochat:cpt-<step>` once its own
training has passed a gate.

**Phase 7 is in (2026-08-17): salience steers.** The verdict is
computed FIRST, on bound-claims-only context (the same conditions the
agreement gate measures under), then steers orientation: below
`SALIENCE_BANDS low=` the perception takes the fast path (no recall,
no escalation exposure - the epsilon audit still applies, so the fast
lane stays observed); above `high=` it earns deep recall (`deep=`
limit instead of 3); between, exactly the old behavior. Counters
`fast_skips` / `deep_orients` ride executive status and the mind tab.

    SALIENCE_STEER=on                    # off = record-don't-act (live)
    SALIENCE_BANDS=low=0.2,high=0.8,deep=6   # keep low/high outside
                                             # the 0.35-0.65 escalation band

**Phase 8a is in (2026-08-17): the pointer splits in two.** The
salience pointer stays the fast lane above. A **user-facing pointer**
now serves `/chat` and only advances through a stricter, slower gate:
a candidate must have SOAKED as the salience pointer (`soak_s=`
seconds and `verdicts=` served verdicts - the fast lane is the slow
lane's canary), the last agreement measurement must clear `agree=`,
and held-out standard loss must not have crept past the last user
promotion by `regress=`. `mode=manual` (the default) stops there: the
mind tab shows **READY with evals attached** and waits for your
click (`promote user pointer`, or `agent-model-user_promote`);
`mode=auto` promotes on its own. A watchdog re-audits the serving
user pointer against the growing held-out pair set every `check_s=`
seconds and auto-rolls-back to last_good if its agreement decays;
`roll back` / `agent-model-user_rollback` is the manual twin. Both
user checkpoints are protected from ring pruning; the pointer
persists across restarts (`user_pointer.json`); soak clocks reset on
restart, deliberately.

    USER_GATE=mode=manual,soak_s=21600,verdicts=100,agree=0.75,regress=0.05,check_s=300

**Routing chat to it is a separate, deliberate flip**: `LLM=LOCAL` in
botd sends the agent app's chat to the user pointer (text-only - no
tool calls; the service refuses until a pointer is promoted). At
d20-scale quality, leave `LLM` on your frontier arm and let the
machinery run ahead of its passenger - that is the point of gating it
separately.

**Phase 8b is in (2026-08-17): the personality adapter.** A standing
LoRA rides the USER-FACING pointer only - the salience lane never
wears it. Feed it a persona corpus at
`runtime/agent/model/persona/persona.jsonl` (one JSON row per line:
`{"user": "...", "assistant": "..."}` or a full `{"messages": [...]}`
conversation; every 5th row is held out and never trained on). The
**mind tab's persona card is the editor**: when nothing is saved it
opens pre-loaded with the SHIPPED DEFAULT SEED (28 exchanges in the
resident model's voice - nanochat-identity style, adapted to this
platform); edit to taste and save to adopt. Saves are validated
whole-file (a bad row rejects with its line number, never truncates)
and the service picks them up on its own. Once a
user pointer is serving and the corpus has >=5 rows, the FIRST
derivation fires on its own; after that the **probe** - held-out
persona loss of the serving model, measured on the watchdog cadence -
triggers background re-derivation whenever the skin slips past
`slack=` (the base grew underneath, or you rewrote the persona). Every
derivation faces its own gate: held-out persona loss must improve by
`min_gain=` over the bare base AND standard LM loss must not rise past
`guard=` (a personality must not lobotomize the base); rejection
leaves serving untouched. The mind tab's judge card shows corpus size,
probe/baseline drift, and a **re-derive persona** button
(`agent-model-persona_rederive` - blocks through the run, returns the
full report). The adapter persists (`persona/adapter.pt`) and
re-applies on every user promotion, rollback, and restart.

    USER_LORA=mode=on,rank=8,alpha=16,lr=1e-3,steps=200,slack=0.1,min_gain=0.01,guard=0.2,targets=c_q.c_v

(`targets` is dot-separated: c_q.c_k.c_v.attn_proj.c_fc.mlp_proj;
`mode=off` disables the whole subsystem.)

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
Within a few seconds `runtime/agent/model/service.py` exists, the service
answers, and the first perception already carries a verdict:

    tools/nb-call.py agent-model-service_status '{}'   # mode: stub
    tools/nb-call.py agent-executive-status '{}'       # last_context.salience

## 2. The base model — bootstrap trains it

Point `MODEL_CHECKPOINT` at a directory. If it holds no loadable
checkpoint (empty is fine), bootstrap starts training in the
background: nanochat's own speedrun pipeline (dataset → tokenizer →
`base_train` → `chat_sft`) on every GPU `nvidia-smi` can see, logged
to `runtime/agent/model/train.log`, pidfile-guarded so repeat
bootstraps report `running` instead of double-starting. This is the
GPU-hours step. Size it to the hardware with:

    # default, sized for one ~32GB consumer GPU without FA3:
    NANOCHAT_TRAIN_ARGS=--depth=20 --device-batch-size=8 --window-pattern=L
    # speedrun-scale (8xH100): --depth=24 --device-batch-size=16 --fp8
    # still OOM? drop --device-batch-size to 4 (grad accum keeps the
    # total batch identical; chat_sft inherits the size from pretrain)

**Multi-node** (e.g. a DGX Spark pair over its ConnectX link): set on
each node, with only `rank=` differing —

    NANOCHAT_DIST=nnodes=2,rank=0,master=192.168.100.1:29500,iface=<if>

Every node runs the same bootstrap; rank 0 trains the tokenizer and
writes `train_done`, other ranks wait for the tokenizer then join the
torchrun rendezvous (`iface=` sets `NCCL_SOCKET_IFNAME` — use the
ConnectX interface). The `MODEL_CHECKPOINT` base dir must hold the
same data on every node: share it over NFS (downloads are
filelock-guarded, concurrent nodes dedupe) or pre-sync local copies.
Empty/unset = single node, exactly as before. On 128GB-unified boxes
raise the batch: `--device-batch-size=32` (and the pair's real payoff
is depth the 32GB card can't hold, e.g. `--depth=26`).

Meanwhile the service sits in `mode: waiting` and retries its load
every 60s — verdicts begin on their own the moment base weights land.
Training ends with `chat_sft`; if the service picked up the bare base
first, restart it (kill by PID; the executive's next start relaunches)
to upgrade to `nanochat:sft`. A pre-existing base dir (the
`~/.cache/nanochat` layout, or a copy) is honored as-is: `training:
not_needed`, immediate load.

## 3. The NanochatScorer glue — shipped, validate on first contact

The glue is written into the asset (`data/agent/_ASSETS/service.py`,
class `NanochatScorer`) against nanochat's real API:
`load_model(source, device, phase="eval")` trying `rl` → `sft` →
`base`, `Engine.generate_batch` for the completion, JSON parse with a
salvage fallback. If it ever needs editing, edit the **asset** in the
repo checkout, never `runtime/agent/model/service.py` — bootstrap
rewrites that copy from the compiled-in asset whenever they differ;
glue belongs in git so every instance gets it. Rebuild the agent dylib
after any asset change (`cargo build --release` in `agent/` —
hot-reloads).

## 4. Turn the checkpoint on

**`MODEL_CHECKPOINT` is a nanochat BASE directory** — the layout
nanochat maintains under `~/.cache/nanochat` after a run:
`tokenizer/` plus `base_checkpoints/` (and `chatsft_checkpoints/` /
`chatrl_checkpoints/` if those phases ran). Point at a copy of that
whole directory, not at a single `model_<step>.pt`:

    MODEL_CHECKPOINT=/path/to/nanochat-base-dir

Restart the instance. Bootstrap builds the serving env one time
(clones `NANOCHAT_REPO` under `runtime/agent/model/deps`, venv, the
pyproject dependencies — the nanochat package itself is never
installed; the service runs with `PYTHONPATH` at the clone, because
the repo's flat layout refuses `pip install -e .`). The `env_ready`
sentinel is written by the install script itself only on full success,
so a half-failed install always retries from clean on the next
bootstrap. Kill the old stub service first if one is running (by PID —
never by pattern).

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
        '{"path": "runtime/agent/model/ingest/batch-day1.jsonl"}'

## 5b. Sharing the GPU

The agent's entire GPU footprint is the one service process. To take
the GPU back: the mind tab's **release GPU** button (or
`agent-model-service_stop`) — clean exit, everything freed. Ring
checkpoints, the replay reservoir, held-out sets, and metrics survive
on disk; the only loss is the candidate's steps since its last
promotion. Resume: **bootstrap** — the relaunched service loads the
NEWEST RING CHECKPOINT, so gate-promoted CPT progress carries across
the off/on cycle (verified: trained to cpt-168, shut down, relaunched,
resumed serving cpt-168). To keep it off across an executive restart
(which fires bootstrap eagerly), flip `SALIENCE=off` in the config
card first — it's a live key — and back `on` before resuming.

## 6. The degradation drill (once, deliberately)

Kill the service by PID: the loop keeps running, `last_context` simply
has no salience field. Restart the executive (or call
`agent-model-bootstrap` yourself) and verdicts resume. The off switch,
any time: delete `SALIENCE=on`, restart the instance.

## 7. Report back

Paste: `/status` in nanochat mode, one `last_context` with a model
verdict, `salience_log` totals after an hour, the trainer drain lines
from `runtime/agent/model/service.log`. Anything odd, include the log —
diagnosis happens from the web session.

## 8. Claude inside the agent (the CLAUDECODE arm)

The frontier arm can be Claude itself, drawing on a Pro/Max
subscription's OAuth login instead of metered API credits, through the
Claude Code CLI. One flip covers **every** frontier surface at once -
the agent app's chat, the dev-session chat plugin, AND the salience
escalations + epsilon audits (`ask_llm` IS `chat_llm`):

    LLM=CLAUDECODE
    LLM_CTL=agent:llm:claude_code

Prerequisite on the box: Claude Code installed and logged in
(`claude` on PATH; `CLAUDE_CODE_BIN=` if elsewhere). Live keys - the
arm re-reads botd on every call, no restart needed.

**Two postures**, chosen by how much you give the delegate:

- *Cheap oracle* (the default): built-in tools off, system prompt
  REPLACED by the agent's own - ~200 tokens per call instead of the
  ~38k Claude Code's full prompt costs. Right for salience
  escalations and plain chat.
- *Full agent*: hand the delegate the whole store -

      CLAUDE_CODE_SYSTEM_MODE=append
      CLAUDE_CODE_MCP={"mcpServers":{"newbound":{"command":"./target/release/newbound","args":["mcp"]}}}
      CLAUDE_CODE_CWD=/path/to/your/newbound/checkout
      CLAUDE_CODE_PERMISSION_MODE=bypassPermissions

  Now a chat turn can read controls, run store commands, and edit
  code - a real Claude session living inside the agent's chat. Note
  the arm always returns finished text (Claude Code runs its own
  loop; newbound's tool_loop terminates on it), and each answer
  carries `cost_usd` - what the turn notionally cost against the
  plan's allowance.

Escalation traffic note: with this arm on, every band escalation
(capped at one per 5s) spawns a CLI call. Cheap in replace mode, but
if the escalation log runs hot you are spending plan allowance on
judgment calls a vLLM box could make - the trade is yours to pick per
box. `CLAUDE_CODE_MODEL=` / `CLAUDE_CODE_EFFORT=` tune the delegate;
`CLAUDE_CODE_TIMEOUT=` (default 600s) bounds a stuck call.

## Troubleshooting

- **No verdicts, `SALIENCE=on`**: check `service_status`; then
  `runtime/agent/model/service.log`. Bootstrap fires eagerly at executive
  start — restart the executive to retry, or run
  `agent-model-bootstrap` by hand (no parameters, `'{}'`) for the full
  report (`nanochat_env`, `script_written`, `service`). The manual
  call blocks through the whole install — that's it working.
- **`launch_failed` with a real checkpoint**: almost always the
  NanochatScorer glue (step 3) — the log shows the exact exception.
- **Verdicts feel flat**: a fresh base is a weak judge — expected.
  The escalation log is the correction signal accumulating; that's
  the flywheel's food, not a bug.
