# Spectrum S7 — placement and the engine seam's far end

**Status: implementation notes for branch
`claude/spectrum-s7-placement`, from the ruled charter
(`docs/spectrum-cycle.md` S7, rulings 6 and 8).**

## What landed

- **Placement** — `MODEL_PLACEMENT=serve=0,train=1` names GPU roles;
  empty is the shipped time-share; roles asking for absent hardware
  fall back to serve, honestly, and `/status.placement` publishes the
  spec, the visible GPUs, the resolved roles, and the mode. On this
  branch the placed train device carries the training-side FRESH
  loads — bench arms, named-adapter derivations, mints (the loads
  that train and free, never entering serving). The full-rung
  candidate split and multi-node CPT stay time-shared/birth-path
  until the owner-box run can actually prove device juggling;
  `NANOCHAT_DIST` remains the multi-node story for birth runs.
- **The external rung** (ruling 8) — `MODEL_ENGINE_URL=` puts an
  OpenAI-compatible engine (vLLM-class) behind the scorer surface:
  `ExternalScorer` only TRANSPORTS, `score_via` builds THE dialect
  and parses THE answer, so unparseable-escalates applies to remote
  verdicts unchanged. The weights live with the engine: the solver
  answers `frozen` with the hot-swap note, nothing derives or stacks
  locally (each path refuses with the reason), and `/promote`
  explains that this rung promotes by adapter hot-swap on the engine
  — wired when a hot-swap-capable engine is configured. No engine
  install stage yet, per the ruling: seam now, dependency when a box
  needs it.

## Battery (CPU, mock engine — passed 2026-08-19)

A stdlib mock OpenAI server stood in for the engine. The service
served `external:mock-70b` (the model id discovered from
`/v1/models`); the salience lane answered through the wire with
`parsed: true` on well-formed JSON and escalated `0.5/parsed: false`
on deliberate garbage — the dialect discipline crossing the wire
intact. `/chat` served the user pointer through the engine (1ms round
trips). Posture read `frozen` with the engine reason; `/promote`
explained hot-swap; `/status.placement` reported the degenerate
time-share on a GPU-less box. R1 restored to stub.

Nits noted: a restored user pointer's display NAME keeps its
persisted `+lora` suffix even when the serving scorer (external, or
persona-skipped) is bare — display drift only, the scorer itself is
correct. Owner-box battery owed: a real vLLM behind the URL, the
split placement observed on two GPUs, and hot-swap wiring when an
engine that supports it is standing.

## Merge word

`Merge: S7 - placement and the engine seam`.
