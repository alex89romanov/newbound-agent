# Spectrum S3 — the backend seam

**Status: implementation notes for branch `claude/spectrum-s3-backend`,
from the ruled charter (`docs/spectrum-cycle.md` S3).** One branch, one
battery, one merge word. Standing rule 5 opens and closes it: stub and
nanochat serving are byte-for-byte the shipped behavior.

## The seam, concretely

Every scorer — Stub, Nanochat, HF — carries one generation contract:
`generate_text(messages, max_tokens, temperature, top_k) -> str`.
Everything above it is backend-blind: `score_via` builds THE serving
prompt and parses THE answer once for all backends
(unparseable-escalates included); `/chat`, the user-pointer watchdog
(`user_agreement`), and anchor minting all route through it. The
structural difference between the backends' loss conventions —
nanochat's GPT takes pre-shifted `(x, y)` with `-1` ignore, HF models
take aligned labels with `-100` and shift internally — lives in exactly
two helpers (`masked_lm_loss_t`, `plain_lm_loss_t`) so no caller ever
reimplements it wrong.

`HFScorer` serves an HF model dir through transformers (the format's
reference implementation — the one third-party door the seam needs; no
PEFT, no vLLM). Chat templates render the dialect; models without one
get plain concatenation, honestly. On CUDA it loads bf16; elsewhere
fp32.

## What changes where

- **Soak**: an hf resident's soak key is `base:hf:<name>` — the base
  soaks on the fast lane exactly like a birth checkpoint, so the
  STANDARD user gate covers it with zero special cases. Same lineage,
  same canary. `USER_GATE soak_s=/verdicts=` are the owner's
  early-access levers, as shipped.
- **Trainer**: the CPT trainer does not run on hf yet — the delta
  trainer lands in S5. Posture is `frozen` and `/status.trainer.posture`
  says so (standing rule 4); banking, streams, and the watchdog all
  continue. `/promote` explains itself instead of failing weirdly.
- **Persona**: the hook-LoRA machinery is backend-blind — one target
  vocabulary (`c_q.c_v...`) maps onto nanochat's module paths or an HF
  model's `q_proj`/`v_proj`/... leaves; derivation, the gate, the
  probe, and the merged apply are unchanged. Multi-turn persona rows
  mask only the final assistant reply on hf (single-turn pairs exact) —
  a documented simplification until the delta trainer needs more.
- **Bootstrap**: the hf refusal is gone. An hf record gets its own
  donefile-guarded env stage (venv + torch + transformers; CPU wheels
  when no GPU is visible, so container batteries never download the
  CUDA bundle), and the launch line carries `--backend` from the
  record. The nanochat env and the birth-training stage run only for
  the nanochat backend. Minting works for both backends through the
  seam — the serving env must match the record's backend, and a
  mismatch records an honest error in `/status.mint`.

## Battery

R1 first and last: stub serving and a bare `MODEL_CHECKPOINT` behave
exactly as master. Then the CPU-scale R2: fabricate a tiny
random-weight HF model (offline — trained BPE tokenizer + chat
template + 2-layer Llama config), import it, `MODEL=` it, bootstrap
(hf env installs, service serves `hf:<name>`), `/salience` answers
(a random model's gibberish exercises unparseable-escalates → 0.5,
parsed:false), soak → READY → `/user_promote` → `/chat` replies,
mint generates the anchor, persona derives through the hf path and
faces its gate. Restore to stub, verify R1 again.

## Merge word

When the battery passes — including the owner-box run with a real
open-weight model — `Merge: S3 - the backend seam`.
