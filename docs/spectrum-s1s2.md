# Spectrum S1+S2 — the registry and the dataset manager

**Status: implementation plan for branch `claude/spectrum-s1s2-registry`,
sequenced 2026-08-19 from the ruled charter (`docs/spectrum-cycle.md`,
plan of record).** One branch, two phases, one battery, one merge word
— the established rhythm. Standing rule 5 is the battery's first line:
the shipped 5b–8b loop runs unchanged on the same runbook steps, and
every work item below is additive until it proves itself.

## The shape (from the rulings)

- **Records in the runtime library** (ruling 1): two record families,
  `models` and `datasets`, written ONLY by the agent library's
  commands, living beside `salience_log`. Bytes under
  `runtime/agent/model/weights/` and `.../datasets/` — the runtime
  folder, user files. Nothing new ships in `data/agent` but commands.
- **The service never reads the store.** The commands render
  `runtime/agent/model/registry.json` (models + datasets manifest,
  canonical serialization) on every registry write; the service picks
  it up by mtime — the persona.jsonl pattern, already proven.
- **Settings**: `MODEL=` names a model record; `MODEL_CHECKPOINT=`
  stays the unregistered-directory alias, resolved exactly as today
  when `MODEL=` is absent. `MODEL_MIX=` weights may name datasets once
  S2 lands; the built-in pool names (`fresh`, `replay`, `standard`)
  keep working untouched.

## S1 — the model record and the resource map

1. **`agent-model-import`** — the deliberate door. Params (all
   required, per platform rule): `name`, `source`, `backend`
   (`nanochat` | `hf`), `anchor` (`mint` | `none` | a dataset name).
   `source` is a local path first (R1 continuity: importing a local
   nanochat base dir must be trivial); the hub form
   (`hf:org/repo@revision`) lands second — revision-pinned into the
   record, `HF_TOKEN` read only here, download filelock-guarded and
   resumable under `weights/<name>/` (ruling 9). A `backend:hf`
   import stages its record honestly and serving REFUSES with "the
   backend lands in S3" — records first, seam next phase.
2. **The model record**: `{name, backend, source, revision, hash,
   path, params, dtype, context_len, anchor, lineage, at}`. Lineage
   is `born:<recipe>` or `imported`; the pointer-name story
   (`nanochat:cpt-N`) generalizes to `<name>:cpt-N` later, unchanged
   now.
3. **`agent-model-models`** (list, with provenance) and
   **`agent-model-model_remove`** (record always; bytes only with
   `purge:true`; refuses on the record `MODEL=` currently names).
4. **The resource map.** Primary in the service (it owns torch):
   `/status` gains `resources` — `{gpus: [{index, name, total_mb,
   free_mb}], disk_free_gb, nodes}` — from
   `torch.cuda.mem_get_info` per device plus a statvfs on the data
   dir. `agent-model-resources` proxies it; when the service is down
   the command falls back to parsing `nvidia-smi` so the map is
   never simply absent. First customers: the S5 solver, ring
   byte-budget warnings (ruling 5), and sizing the birth run's
   `NANOCHAT_TRAIN_ARGS` defaults from the map instead of a
   hand-picked constant.
5. **Bootstrap resolves `MODEL=`**: record → path + backend on the
   launch line. No `MODEL=`? The `MODEL_CHECKPOINT` path runs
   byte-for-byte as today.
6. **Anchor minting** (ruling 2): `anchor:mint` asks the service —
   new endpoint `/mint_anchor` — to sample a few hundred generations
   from the imported model into a frozen `anchor` dataset registered
   through S2, recorded on the model. Nanochat-backed models only
   until S3 makes generation backend-agnostic; `mint` on an hf
   import stages the request and reports it deferred.

**S1 battery** (disposable copy for anything mutating, per
`tools/scratch-instance.md`): R1 first — stub mode and a bare
`MODEL_CHECKPOINT` behave identically to master, `service_status` /
runbook steps unchanged. Then: import a local nanochat base dir →
record listed with hash and provenance; `MODEL=` serves it;
`registry.json` appears and the service reports the registered name;
`resources` sane on a GPU box and empty-but-present on CPU-only;
`model_remove` refuses on the serving record and purges an unused
one; hf import stages and serving refuses with the S3 message.

## S2 — the dataset manager and the feed contract

1. **The dataset record**: `{name, kind (cpt|sft|eval|persona|anchor),
   format (jsonl|parquet|txt), path, rows, hash, holdout (every=N),
   mode (stream|snapshot), lineage, provenance, at}`.
2. **Commands**, in the curriculum_export idiom:
   - `agent-model-dataset_add` — local file or directory first, hub
     datasets through the same door later; hashes, counts, registers.
   - `agent-model-dataset_list` / `agent-model-dataset_inspect`
     (counts, hash verify, peek N rows).
   - `agent-model-dataset_snapshot` — pin a frozen, hashed cut of a
     stream as a new record with lineage (ruling 10: the bench and
     SFT runs pin; the live loop rides streams).
   - `agent-model-dataset_derive` — procedural transforms only on
     this branch (first transform: the serving-dialect rendering
     `render_sample` already is), recording lineage + transform
     recipe. Model-driven generation waits for its governed spender
     (ruled deliberate/drive-budgeted; the plumbing belongs beside
     the bench).
3. **The feed contract begins**: `curriculum_export` learns to append
   into stream datasets (`salience-pairs`, `memory`) registered on
   first use, while the loose-file ingest path keeps working
   untouched (R1) and is marked legacy in the runbook. Full
   no-orphan-banks migration completes when the trainer reads only
   datasets; not before.
4. **Trainer generalization** (`service.py`): pools built from
   registry.json datasets joined with `MODEL_MIX` weights by name;
   the three built-ins remain when no datasets are configured —
   today's behavior bit-for-bit. Held-out policy honored per dataset
   (every-Nth to a holdout file, never trained — the persona split
   pattern); the gate's eval extends to per-dataset held-out losses,
   reported in the `trainer` status block. This is the bench's
   yardstick (ruling 7) accumulating before the bench exists.
5. **The anchor rule lands** (ruling 2): `anchor` datasets resolve
   through the manager. A born-here model's anchor auto-registers on
   first launch from its own `base_data` shards (a sampled, frozen
   cut); the forgetting gate reads through the manager from then on,
   same numbers as today.

**S2 battery**: R1 again — no datasets configured, master behavior
identical, ingest drain works. Then: `dataset_add` a JSONL → listed
with rows + hash, inspect peeks, hash verify catches a tampered
byte; `MODEL_MIX=salience-pairs=0.3,standard=0.7` → trainer status
shows the named pools and per-pool held-out loss; snapshot pins a
cut whose hash survives the stream growing; export appends to the
stream dataset AND the legacy drain still trains; the born-here
anchor auto-registers and the gate's `cand_std`/`live_std` numbers
match master's on the same data.

## Sequencing within the branch

S1 items 1–3 (records + commands, local-path import only) → S1
battery on stubs → S1 items 4–6 → S2 items 1–2 → S2 items 3–5 → full
battery on the GPU box. Registry plumbing proves out before anything
touches the trainer; the trainer change (S2 item 4) is the only edit
to shipped behavior and lands last, behind its own R1 check.

## Merge word

When the battery passes end-to-end on the owner's box:
`Merge: S1+S2 - the registry and the dataset manager`.
