#!/usr/bin/env python3
"""The resident model service (understandingloop.md commitment 5; Phase 5b).

Trainer and server in ONE process, beside the newbound instance, speaking
HTTP on localhost. The agent.model control's commands are thin clients on
this; with SALIENCE=on in botd.properties the executive's verdicts route
here (and agent-model-bootstrap installs, writes, and launches this file
itself). Stdlib only in stub mode - torch is imported ONLY when a real
checkpoint is named, so the whole chain verifies on any machine before a
GPU ever gets involved.

    python3 runtime/agent/model/service.py --data-dir runtime/agent/model
    python3 runtime/agent/model/service.py --data-dir runtime/agent/model \
        --checkpoint /path/to/nanochat/checkpoint

Endpoints:
    POST /salience      {perception, context} -> {salient, reasoning, pointer, ms}
    GET  /status        -> mode, live slot, counters, trainer + user blocks
    POST /ingest        (JSONL body) -> queued into the ingest directory
    POST /promote       load newest checkpoint into the INACTIVE slot, swap live
    POST /chat          {messages} -> {text, pointer, ms}  (the USER pointer)
    POST /user_promote  advance the user pointer onto the gate's ready candidate
    POST /user_rollback revert the user pointer to last_good
    POST /shutdown      clean exit (the GPU off switch)

The live pointer is double-buffered from day one: /promote loads into the
slot not being served and swaps under a lock, so serving never waits on a
load.

Phase 8a: the pointer splits in two. The SALIENCE pointer is the fast
lane above - permissive gate, cheap failures, every verdict audited.
The USER pointer serves /chat and only advances through a stricter,
slower gate: the candidate must have SOAKED as the salience pointer
(--user-gate soak_s= seconds and verdicts= served verdicts on the fast
lane - the fast lane is the slow lane's canary), the last agreement
measurement must clear agree=, and held-out standard loss must not have
crept past the last user promotion by regress=. mode=manual (default)
stops there: the candidate is marked READY and waits for a deliberate
/user_promote (the mind tab's button); mode=auto promotes on its own.
A watchdog re-audits the serving user pointer against the GROWING
held-out pair set every check_s= seconds and auto-rolls-back to
last_good if its agreement decays - and both user checkpoints are
protected from ring pruning. Restart resets soak clocks (conservative)
but the user pointer itself persists in user_pointer.json.

Phase 6: the trainer is REAL. When a nanochat checkpoint is serving and
--train is on, a candidate copy of the live model steps continuously on
MIXED mini-batches - fresh curriculum from ingest, a replay reservoir of
older curriculum, and standard pretraining data at a set ratio
(arrival-order training is doctrine-rejected: correlated gradients,
recency capture, catastrophic forgetting). Every --gate `every` steps
the candidate is evaluated against the LIVE pointer on held-out sets: it
must not regress on held-out standard data (forgetting guard) and must
not be worse than live on held-out curriculum (learning check). Pass ->
checkpoint to the ring and promote through the double buffer; fail ->
count, and after `fails` consecutive failures the candidate resets to
the live weights (auto-rollback of unpromoted drift). The stub scorer
never trains - stub mode keeps the 5b skeleton behavior.
"""
import argparse
import json
import os
import re
import shutil
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

START = time.time()
LOCK = threading.Lock()
TRAINER = {
    "active": False,
    "steps": 0,
    "loss_ema": None,
    "fresh_pending": 0,
    "replay_size": 0,
    "standard_docs": 0,
    "gates": 0,
    "promotions": 0,
    "fails": 0,
    "resets": 0,
    "last_gate": None,
    "mix": None,
}
TRAIN_Q = []          # {doc, pair} curriculum entries awaiting training (LOCK)
HELDOUT_FRESH = []    # reserved curriculum docs, never trained on (LOCK)
HELDOUT_PAIRS = []    # structured held-out salience pairs for the agreement gate
MET_COUNT = [0]


def append_metric(data_dir, row):
    """The metrics journal (metrics.jsonl): every served verdict, loss
    samples, every gate. Instance-owned, capped in place - the mind
    tab's trends read it through agent-model-metrics."""
    row = dict(row)
    row["t"] = int(time.time() * 1000)
    path = os.path.join(data_dir, "metrics.jsonl")
    try:
        with LOCK:
            with open(path, "a") as f:
                f.write(json.dumps(row) + "\n")
            MET_COUNT[0] += 1
            if MET_COUNT[0] % 500 == 0:
                with open(path) as f:
                    lines = f.readlines()
                if len(lines) > 8000:
                    with open(path, "w") as f:
                        f.writelines(lines[-4000:])
    except Exception:
        pass


RESOURCES_CACHE = {"at": 0.0, "val": None}


def probe_resources(data_dir):
    """The resource map (spectrum S1): GPUs + disk, cheap and honest.
    torch when the venv has it, nvidia-smi when it doesn't, empty -
    but present - on a box with neither. Cached briefly: /status is
    polled and nvidia-smi is a subprocess. First customers: the S5
    posture solver, ring byte-budget warnings, birth-run sizing."""
    now = time.time()
    if RESOURCES_CACHE["val"] is not None and now - RESOURCES_CACHE["at"] < 5:
        return RESOURCES_CACHE["val"]
    gpus = []
    try:
        import torch
        if torch.cuda.is_available():
            for i in range(torch.cuda.device_count()):
                free, total = torch.cuda.mem_get_info(i)
                gpus.append({"index": i,
                             "name": torch.cuda.get_device_name(i),
                             "total_mb": int(total / 1048576),
                             "free_mb": int(free / 1048576)})
    except Exception:
        try:
            import subprocess
            out = subprocess.run(
                ["nvidia-smi",
                 "--query-gpu=index,name,memory.total,memory.free",
                 "--format=csv,noheader,nounits"],
                capture_output=True, text=True, timeout=5).stdout
            for ln in out.strip().splitlines():
                parts = [p.strip() for p in ln.split(",")]
                if len(parts) >= 4:
                    gpus.append({"index": int(parts[0]), "name": parts[1],
                                 "total_mb": int(parts[2]),
                                 "free_mb": int(parts[3])})
        except Exception:
            pass
    disk_free_gb = None
    try:
        st = os.statvfs(data_dir)
        disk_free_gb = round(st.f_bavail * st.f_frsize / 1073741824, 1)
    except Exception:
        pass
    ram_free_mb = None
    try:
        with open("/proc/meminfo") as f:
            for ln in f:
                if ln.startswith("MemAvailable:"):
                    ram_free_mb = int(ln.split()[1]) // 1024
                    break
    except Exception:
        pass
    val = {"gpus": gpus, "disk_free_gb": disk_free_gb,
           "ram_free_mb": ram_free_mb}
    RESOURCES_CACHE["at"] = now
    RESOURCES_CACHE["val"] = val
    return val


def registry_info(data_dir):
    """The registry's window into /status: names only. The record of
    truth lives in the runtime library; the service reads only the
    rendered registry.json (it never reads the store), picked up by
    mtime like persona.jsonl."""
    path = os.path.join(data_dir, "registry.json")
    if not os.path.exists(path):
        return None
    try:
        with open(path) as f:
            reg = json.load(f)
        return {"models": [m.get("name", "?") for m in reg.get("models", [])],
                "datasets": [d.get("name", "?")
                             for d in reg.get("datasets", [])],
                "rendered_at": reg.get("rendered_at")}
    except Exception as e:
        return {"error": f"registry.json unreadable: {e}"}


REGISTRY_DS = {"mtime": 0.0, "list": []}


def load_registry_datasets(data_dir):
    """Registered datasets from registry.json (the service's store-blind
    window). Re-read when the file's mtime moves - the same pickup
    signal the whole registry uses."""
    path = os.path.join(data_dir, "registry.json")
    try:
        mt = os.path.getmtime(path)
    except OSError:
        REGISTRY_DS["list"] = []
        return REGISTRY_DS["list"]
    if mt != REGISTRY_DS["mtime"]:
        try:
            with open(path) as f:
                REGISTRY_DS["list"] = json.load(f).get("datasets", [])
            REGISTRY_DS["mtime"] = mt
        except Exception:
            pass
    return REGISTRY_DS["list"]


def dataset_paths(rec):
    """The files carrying a registered dataset's rows, by its format."""
    p = rec.get("path") or ""
    fmt = rec.get("format") or "jsonl"
    ext = {"jsonl": ".jsonl", "txt": ".txt",
           "parquet": ".parquet"}.get(fmt, ".jsonl")
    if os.path.isfile(p):
        return [p]
    if os.path.isdir(p):
        out = []
        for base, _dirs, files in os.walk(p):
            out.extend(os.path.join(base, n)
                       for n in sorted(files) if n.endswith(ext))
        return sorted(out)
    return []


def load_dataset_docs(rec, limit=2048):
    """(train_docs, holdout_docs) for one registered dataset. JSONL
    rows render through render_sample so training speaks THE dialect
    ({"text": ...} rows pass through); txt rows are docs as-is.
    holdout_every=N reserves every Nth row, never trained - the
    persona split pattern. Tail-capped so streams stay affordable."""
    fmt = rec.get("format") or "jsonl"
    if fmt == "parquet":
        return [], []  # pyarrow territory; the standard pool covers it
    lines = []
    for path in dataset_paths(rec):
        try:
            with open(path) as f:
                lines.extend(ln.rstrip("\n") for ln in f if ln.strip())
        except OSError:
            continue
    lines = lines[-limit:]
    hn = int(rec.get("holdout_every") or 0)
    train, hold = [], []
    for i, ln in enumerate(lines):
        doc = ln
        if fmt == "jsonl":
            try:
                o = json.loads(ln)
                if isinstance(o, dict):
                    doc = o["text"] if "text" in o else render_sample(o)
            except Exception:
                pass
        (hold if hn > 0 and i % hn == hn - 1 else train).append(doc)
    return train, hold[:64]


def load_anchor_docs(args):
    """Ruling 2: the forgetting guard reads its anchor through the
    manager WHEN the serving model is registered with one - resolve
    this service's checkpoint against the registry's model records,
    follow the record's anchor to a registered dataset, load its docs.
    Returns [] when unregistered or unanchored: the legacy base-dir
    path then runs byte-for-byte (R1)."""
    path = os.path.join(args.data_dir, "registry.json")
    try:
        with open(path) as f:
            reg = json.load(f)
    except Exception:
        return []
    try:
        ck = os.path.realpath(args.checkpoint)
    except OSError:
        return []
    anchor = None
    for m in reg.get("models", []):
        try:
            if os.path.realpath(m.get("path") or "") == ck:
                a = m.get("anchor") or ""
                if a and a not in ("mint", "none"):
                    anchor = a
                break
        except OSError:
            continue
    if not anchor:
        return []
    for rec in reg.get("datasets", []):
        if rec.get("name") == anchor:
            docs, _hold = load_dataset_docs(dict(rec, holdout_every=0),
                                            limit=512)
            if docs:
                print(f"[trainer] anchor through the manager: "
                      f"'{anchor}' ({len(docs)} docs)", flush=True)
            return docs
    return []


MINT = {"running": False, "last": None}

MINT_SEEDS = [
    "Tell me about yourself.",
    "What do you know a lot about?",
    "Explain something interesting.",
    "Describe a process you understand well.",
    "What happened in the world recently?",
    "Write a short story about a machine that learns.",
    "Give me practical advice about computers.",
    "What makes a good explanation?",
    "Describe your ideal day.",
    "How does the weather work?",
    "What is the most important invention?",
    "Continue this thought: The system was designed to",
    "Summarize what you believe about language.",
    "What would you teach a beginner first?",
    "Describe a place you would like to visit.",
    "Why do people tell stories?",
]


def mint_anchor_thread(args, path, out_name, n, backend="nanochat"):
    """Ruling 2: an import without its pretraining data gets a MINTED
    anchor - a frozen sample of the model's own voice at the door,
    measuring drift-from-its-imported-self forever after. Loads its
    own scorer from `path` (the serving slots are never touched),
    samples with variety, writes datasets/<out_name>/anchor.jsonl +
    meta.json, and frees the weights. Progress rides /status.mint."""
    res = {"model": path, "name": out_name, "requested": int(n), "rows": 0,
           "started": int(time.time() * 1000), "status": "running"}
    with LOCK:
        MINT["running"] = True
        MINT["last"] = dict(res)
    scorer = None
    try:
        # the seam covers minting too: whichever backend the weights
        # are, generate_text is the one door (S3). The serving env must
        # be able to import the backend - a mismatch records an honest
        # error in /status.mint rather than half-minting.
        scorer = (HFScorer(path) if backend == "hf"
                  else NanochatScorer(path))
        outdir = os.path.join(args.data_dir, "datasets", out_name)
        os.makedirs(outdir, exist_ok=True)
        rows = 0
        with open(os.path.join(outdir, "anchor.jsonl"), "w") as f:
            for i in range(int(n)):
                prompt = MINT_SEEDS[i % len(MINT_SEEDS)]
                text = scorer.generate_text(
                    [{"role": "user", "content": prompt}],
                    max_tokens=256, temperature=0.8, top_k=50)
                f.write(json.dumps({"text": text}) + "\n")
                rows += 1
                if rows % 25 == 0:
                    with LOCK:
                        MINT["last"] = dict(res, rows=rows)
        meta = {"name": out_name, "kind": "anchor", "rows": rows,
                "model_path": path, "frozen": True,
                "at": int(time.time() * 1000)}
        with open(os.path.join(outdir, "meta.json"), "w") as f:
            json.dump(meta, f)
        res.update(rows=rows, status="done",
                   finished=int(time.time() * 1000))
        print(f"[mint] anchor {out_name}: {rows} rows", flush=True)
    except Exception as e:
        res.update(status="error", error=str(e)[:500])
        print(f"[mint] anchor {out_name} FAILED: {e}", flush=True)
    finally:
        del scorer
        try:
            import torch
            torch.cuda.empty_cache()
        except Exception:
            pass
    with LOCK:
        MINT["running"] = False
        MINT["last"] = res


STATE = {
    "slots": {"A": None, "B": None},
    "live": "A",
    "loading": True,
    "boot_error": None,
    "scored": 0,
    "ingested_files": 0,
    "ingested_samples": 0,
    "promotions": 0,
}
# ── Phase 8a: the user-facing pointer ────────────────────────────────
# Soak ledger: how long each pointer has served the salience lane and
# how many verdicts it answered there. Keys: "stub", "base:<src>" (the
# birth checkpoint), or a ring dir basename ("cpt-<stamp>-<steps>").
# In-memory only - a restart resets the clocks, deliberately.
SOAK = {"current": None, "rings": {}}
USER = {
    "pointer": None,      # soak key currently serving /chat
    "name": None,         # display name (user:<key>)
    "promoted_at": None,
    "eval": None,         # snapshot that justified the promotion
    "ready": None,        # soak key that passed the gate, awaiting approval
    "ready_eval": None,
    "last_good": None,    # previous pointer, the rollback target
    "promotions": 0,
    "rollbacks": 0,
}
USER_SLOT = {"scorer": None}
USER_GATE_DEFAULTS = {"mode": "manual", "soak_s": 21600, "verdicts": 100,
                      "agree": 0.75, "regress": 0.05, "check_s": 300}


def set_soak(key):
    """The salience pointer changed: start (or resume) its soak ledger.
    Called with None when the serving pointer has no loadable twin on
    disk (a ring save failed) - such a pointer can never qualify."""
    with LOCK:
        SOAK["current"] = key
        if key is not None and key not in SOAK["rings"]:
            SOAK["rings"][key] = {"since": time.time(), "verdicts": 0}


def soak_key_for(scorer):
    """The soak-ledger key for a freshly loaded scorer (load_initial and
    /promote paths; trainer promotions key by the ring dir they saved)."""
    name = getattr(scorer, "name", "stub")
    if name == "stub" or isinstance(scorer, StubScorer):
        return "stub"
    if name.startswith("nanochat:cpt-"):
        return name.split(":", 1)[1]
    if name.startswith("nanochat:"):
        return "base:" + name.split(":", 1)[1]
    if name.startswith("hf:"):
        # the hf base soaks on the fast lane exactly like a birth
        # checkpoint - same lineage, same canary, the standard user
        # gate covers it with zero special cases (S3)
        return "base:" + name
    return None


def user_state_path(data_dir):
    return os.path.join(data_dir, "user_pointer.json")


def save_user_state(data_dir):
    with LOCK:
        snap = {k: USER[k] for k in ("pointer", "name", "promoted_at", "eval",
                                     "last_good", "promotions", "rollbacks")}
    try:
        tmp = user_state_path(data_dir) + ".tmp"
        with open(tmp, "w") as f:
            json.dump(snap, f)
        os.replace(tmp, user_state_path(data_dir))
    except Exception as e:
        print(f"[user] state save failed: {e}", flush=True)


class StubScorer:
    """Deterministic heuristic scorer - no model, no dependencies.

    Honors the test markers (SAL-HI / SAL-MID / SAL-LO) the disposable
    batteries and the runbook's acceptance steps use, and otherwise
    scores by perception kind and how many claims the envelope arrived
    bound to. The point is not judgment quality - it is that every wire,
    swap, and failure path can be proven before a checkpoint exists.
    """

    name = "stub"

    def __init__(self, tag="stub"):
        self.tag = tag

    def score(self, perception, context):
        text = ""
        payload = perception.get("payload") or {}
        for v in payload.values():
            if isinstance(v, str):
                text += " " + v.lower()
        if "sal-hi" in text:
            return 0.9, "stub: high marker"
        if "sal-lo" in text:
            return 0.05, "stub: low marker"
        if "sal-mid" in text:
            return 0.5, "stub: uncertain marker"
        base = {
            "store_change": 0.55,
            "text_input": 0.65,
            "file_change": 0.5,
            "peer_event": 0.45,
            "acoustic_event": 0.5,
        }.get(perception.get("kind", ""), 0.4)
        bound = context.get("bound") or 0
        score = min(0.95, base + 0.1 * min(int(bound), 3))
        return score, f"stub[{self.tag}]: kind base with {bound} bound claims"

    def generate_text(self, messages, max_tokens=96, temperature=0.2,
                      top_k=50):
        last = str((messages or [{}])[-1].get("content", ""))[:120]
        return f"[stub {self.tag}] heard: {last}"


def salience_prompt(kind, text, matched, bound):
    """THE serving prompt - the skill the agreement gate measures is the
    skill the scorer serves, so both build prompts here."""
    return (
        "You judge salience for an autonomous agent's perception stream.\n"
        f"PERCEPTION kind={kind}: {text}\n"
        f"CONTEXT: {int(matched)} recalled and {int(bound)} bound memory claims.\n"
        "How much does this perception matter to the agent's "
        "understanding of its environment, from 0.0 (noise) to 1.0 "
        "(critical)? Reply with ONLY a JSON object: "
        '{"salient": <0.0-1.0>, "reasoning": "<one sentence>"}')


def parse_salience(completion):
    """(salient, reasoning, parsed) out of a model reply. parsed=False
    marks a salvaged number or a total parse failure - the executive
    treats an unparseable verdict as UNCERTAINTY and escalates it (a
    judge that cannot state its verdict is exactly what the frontier
    should adjudicate; each one becomes a format-teaching pair)."""
    s0, e0 = completion.find("{"), completion.rfind("}")
    if 0 <= s0 < e0:
        try:
            d = json.loads(completion[s0:e0 + 1])
            sal = float(d.get("salient"))
            why = str(d.get("reasoning") or "")[:300]
            return max(0.0, min(1.0, sal)), (why or "model gave no reasoning"), True
        except Exception:
            pass
    m = re.search(r"(?<![\w.])(?:0?\.\d+|[01](?:\.\d+)?)(?![\w.])", completion)
    if m:
        return (max(0.0, min(1.0, float(m.group(0)))),
                f"unparseable reply, salvaged number: {completion[:120]!r}", False)
    return None, f"unparseable reply: {completion[:120]!r}", False


# ── the backend seam (spectrum S3) ──────────────────────────────────
# Everything the serving side asks of a model goes through the scorer
# surface: generate_text(messages) -> str, score(), .name, .meta,
# .checkpoint. Two real backends implement it - nanochat and hf - and
# nothing above the seam knows which is serving. The dialect
# (salience_prompt + JSON answer + unparseable-escalates) is identical
# by construction: score_via builds it once for every backend.

def model_device(model):
    """nanochat's GPT carries get_device(); HF models answer through
    their parameters. One question, one place."""
    if hasattr(model, "get_device"):
        return model.get_device()
    import torch
    try:
        return next(model.parameters()).device
    except StopIteration:
        return torch.device("cpu")


def scorer_model(scorer):
    """The underlying nn.Module: nanochat keeps it on the engine, hf
    keeps it directly."""
    if hasattr(scorer, "engine"):
        return scorer.engine.model
    return getattr(scorer, "model", None)


def masked_lm_loss_t(model, tokenizer, ids, mask, device):
    """One masked-LM loss (tensor, backward-able) on an UNSHIFTED token
    sequence; mask=1 marks the tokens that train. The two backends
    disagree structurally: nanochat's GPT takes pre-shifted (x, y)
    with -1 ignore; HF models take aligned labels with -100 and shift
    internally. The seam owns that bookkeeping so no caller ever
    reimplements it wrong."""
    import torch
    if hasattr(tokenizer, "render_conversation"):  # nanochat
        x = torch.tensor([ids[:-1]], device=device)
        y = torch.tensor([[t if mask[i + 1] == 1 else -1
                           for i, t in enumerate(ids[1:])]], device=device)
        return model(x, y)
    x = torch.tensor([ids], device=device)
    labels = torch.tensor([[t if mask[i] == 1 else -100
                            for i, t in enumerate(ids)]], device=device)
    return model(input_ids=x, labels=labels).loss


def plain_lm_loss_t(model, tokenizer, chunk, device):
    """One plain LM loss (tensor) on a token chunk, same shift
    bookkeeping as masked_lm_loss_t."""
    import torch
    if hasattr(tokenizer, "render_conversation"):  # nanochat
        x = torch.tensor([chunk[:-1]], device=device)
        y = torch.tensor([chunk[1:]], device=device)
        return model(x, y)
    x = torch.tensor([chunk], device=device)
    return model(input_ids=x, labels=x.clone()).loss


def encode_doc(tokenizer, doc):
    """One doc -> token ids, per backend."""
    if hasattr(tokenizer, "render_conversation"):
        return tokenizer.encode(doc[:4000], prepend="<|bos|>")
    return tokenizer(doc[:4000]).input_ids


def render_masked(tokenizer, conv, max_tokens):
    """(ids, mask) for a conversation: mask=1 on the tokens the loss
    trains (the assistant's). nanochat's tokenizer owns this natively;
    for HF the final message must be the assistant's and the mask is
    its tail past the generation prompt - single-turn-pair exact,
    multi-turn masks only the final reply (documented simplification
    until the delta trainer needs more)."""
    if hasattr(tokenizer, "render_conversation"):
        return tokenizer.render_conversation(conv, max_tokens=max_tokens)
    msgs = conv["messages"]
    full = tokenizer.apply_chat_template(msgs, tokenize=True,
                                         add_generation_prompt=False)
    prompt = tokenizer.apply_chat_template(msgs[:-1], tokenize=True,
                                           add_generation_prompt=True)
    if hasattr(full, "input_ids"):
        full = full.input_ids       # BatchEncoding on newer transformers
    if hasattr(prompt, "input_ids"):
        prompt = prompt.input_ids
    ids = full[:max_tokens]
    cut = min(len(prompt), len(ids))
    mask = [0] * cut + [1] * (len(ids) - cut)
    return ids, mask


def score_via(scorer, perception, context):
    """THE score path, backend-blind: the serving prompt, one
    generation, the JSON parse with unparseable-escalates."""
    payload = perception.get("payload") or {}
    text = " ".join(str(v) for v in payload.values()
                    if isinstance(v, str))[:600]
    prompt = salience_prompt(perception.get("kind", "?"), text,
                             context.get("matched") or 0,
                             context.get("bound") or 0)
    completion = scorer.generate_text(
        [{"role": "user", "content": prompt}],
        max_tokens=96, temperature=0.2, top_k=50)
    sal, why, parsed = parse_salience(completion)
    if sal is None:
        return 0.5, "defaulting uncertain - " + why, False
    return sal, why, parsed


class NanochatScorer:
    """The real judge - a nanochat checkpoint served in-process.

    `checkpoint` is a nanochat BASE DIRECTORY - the layout nanochat
    itself maintains under ~/.cache/nanochat after a run: tokenizer/
    plus base_checkpoints/ (and chatsft_checkpoints/ /
    chatrl_checkpoints/ if those phases ran). The most-trained source
    available wins: rl, then sft, then base. Needs the nanochat repo
    importable (bootstrap launches this service with PYTHONPATH set to
    its clone) and its deps in the venv python running this process.
    The score() contract matches the stub's:
    (perception, context) -> (float 0..1, reasoning str).
    """

    name = "nanochat"

    def __init__(self, checkpoint):
        self.checkpoint = checkpoint
        # The base dir must be in the env BEFORE nanochat's modules
        # resolve it (get_base_dir also owns the tokenizer location).
        os.environ["NANOCHAT_BASE_DIR"] = checkpoint
        import torch
        from nanochat.checkpoint_manager import load_model
        from nanochat.engine import Engine
        self.torch = torch
        device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        last_err = None
        for source in ("rl", "sft", "base"):
            try:
                model, tokenizer, _meta = load_model(source, device, phase="eval")
                break
            except Exception as e:
                last_err = e
        else:
            raise RuntimeError(
                f"no loadable checkpoint under {checkpoint} "
                f"(expected the nanochat base-dir layout: tokenizer/ + "
                f"base_checkpoints/<tag>/model_<step>.pt etc): {last_err}")
        self.tokenizer = tokenizer
        self.engine = Engine(model, tokenizer)
        self.meta = _meta
        self.name = f"nanochat:{source}"

    @classmethod
    def from_ring(cls, base_checkpoint, ring_dir):
        """Load a ring checkpoint (a gate-promoted CPT candidate) with
        the tokenizer from the nanochat base dir - so a restarted
        service resumes at its own latest promotion instead of
        regressing to the birth checkpoint."""
        os.environ["NANOCHAT_BASE_DIR"] = base_checkpoint
        import torch
        from nanochat.checkpoint_manager import build_model, find_last_step
        from nanochat.engine import Engine
        device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        step = find_last_step(ring_dir)
        model, tokenizer, meta = build_model(ring_dir, step, device, "eval")
        self = object.__new__(cls)
        self.checkpoint = base_checkpoint
        self.tokenizer = tokenizer
        self.engine = Engine(model, tokenizer)
        self.meta = meta
        self.name = f"nanochat:{os.path.basename(ring_dir)}"
        return self

    @classmethod
    def from_parts(cls, model, tokenizer, meta, name, checkpoint):
        """Wrap an in-memory model (a gate-passed candidate) as a scorer
        without touching disk - promotion through the double buffer."""
        from nanochat.engine import Engine
        self = object.__new__(cls)
        self.checkpoint = checkpoint
        self.tokenizer = tokenizer
        self.engine = Engine(model, tokenizer)
        self.meta = meta
        self.name = name
        return self

    def generate_text(self, messages, max_tokens=96, temperature=0.2,
                      top_k=50):
        conversation = {"messages": list(messages)
                        + [{"role": "assistant", "content": ""}]}
        ids = self.tokenizer.render_for_completion(conversation)
        results, _masks = self.engine.generate_batch(
            ids, num_samples=1, max_tokens=int(max_tokens),
            temperature=float(temperature), top_k=int(top_k))
        return self.tokenizer.decode(results[0][len(ids):])

    def score(self, perception, context):
        return score_via(self, perception, context)


class HFScorer:
    """The second backend (spectrum S3): an HF-format model dir served
    in-process through transformers - the format's reference
    implementation, the one third-party door the seam needs. Same
    score()/generate_text() contracts, same dialect, same
    unparseable-escalates. The CPT trainer does not run on this
    backend yet: the delta trainer lands in S5, so an hf resident's
    posture is frozen - and /status says so (standing rule 4)."""

    name = "hf"

    def __init__(self, checkpoint):
        self.checkpoint = checkpoint
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer
        self.torch = torch
        self.device = torch.device(
            "cuda" if torch.cuda.is_available() else "cpu")
        self.tokenizer = AutoTokenizer.from_pretrained(checkpoint)
        dtype = torch.bfloat16 if self.device.type == "cuda" else torch.float32
        self.model = AutoModelForCausalLM.from_pretrained(
            checkpoint, torch_dtype=dtype, low_cpu_mem_usage=True
        ).to(self.device)
        self.model.eval()
        ctx = int(getattr(self.model.config,
                          "max_position_embeddings", 0) or 2048)
        self.meta = {"model_config": {"sequence_len": ctx}}
        self.name = f"hf:{os.path.basename(os.path.normpath(checkpoint))}"

    def generate_text(self, messages, max_tokens=96, temperature=0.2,
                      top_k=50):
        import torch
        tok = self.tokenizer
        try:
            ids = tok.apply_chat_template(list(messages),
                                          add_generation_prompt=True,
                                          return_tensors="pt")
            if hasattr(ids, "input_ids"):
                # newer transformers return a BatchEncoding here
                ids = ids.input_ids
        except Exception:
            # no chat template shipped with the model: plain
            # concatenation is the honest fallback
            text = "\n".join(str(m.get("content", ""))
                             for m in messages) + "\n"
            ids = tok(text, return_tensors="pt").input_ids
        ids = ids.to(self.device)
        with torch.no_grad():
            out = self.model.generate(
                ids, max_new_tokens=int(max_tokens),
                do_sample=float(temperature) > 0.05,
                temperature=max(float(temperature), 0.05),
                top_k=int(top_k),
                pad_token_id=(tok.pad_token_id
                              if tok.pad_token_id is not None
                              else tok.eos_token_id))
        return tok.decode(out[0][ids.shape[1]:], skip_special_tokens=True)

    def score(self, perception, context):
        return score_via(self, perception, context)


def newest_ring(data_dir):
    """Newest gate-promoted checkpoint dir (cpt-*, must hold weights)."""
    import glob as _glob
    dirs = sorted(d for d in _glob.glob(os.path.join(data_dir, "checkpoints", "cpt-*"))
                  if _glob.glob(os.path.join(d, "model_*.pt")))
    return dirs[-1] if dirs else None


def make_scorer(checkpoint, data_dir, backend="nanochat"):
    if checkpoint == "stub":
        return StubScorer()
    if backend == "hf":
        # gate-promoted CPT progress resumes across restarts on the
        # lora rung too: re-merge the delta ring (S5)
        return merge_ring_deltas(HFScorer(checkpoint), data_dir)
    ring = newest_ring(data_dir)
    if ring:
        try:
            return NanochatScorer.from_ring(checkpoint, ring)
        except Exception as e:
            print(f"[service] ring load failed ({e}); falling back to base dir", flush=True)
    return merge_ring_deltas(NanochatScorer(checkpoint), data_dir)


def newest_checkpoint(args):
    ckdir = os.path.join(args.data_dir, "checkpoints")
    entries = sorted(
        (e for e in os.listdir(ckdir)) if os.path.isdir(ckdir) else []
    )
    return os.path.join(ckdir, entries[-1]) if entries else None


def render_sample(o):
    """One curriculum JSONL row -> one training document. These are the
    adjudicated, structured residues (claims, curation traces, salience
    pairs) - raw logs never ride, per doctrine."""
    k = o.get("kind", "")
    try:
        if k == "salience_pair":
            # the serving prompt + the serving answer format, exactly as
            # the scorer serves and the agreement gate measures - training
            # in any other dialect teaches the parser's failure mode
            r = o.get("row") or {}
            target = r.get("frontier", r.get("local", ""))
            why = str(r.get("frontier_why", "") or "")[:200]
            try:
                tval = round(float(target), 2)
            except Exception:
                tval = 0.5
            ans = json.dumps({"salient": tval,
                              "reasoning": why or "no reasoning recorded"})
            return (salience_prompt("?", str(r.get("input", ""))[:600], 0, 0)
                    + "\n" + ans)
        if k == "claim":
            e = o.get("entry") or {}
            return (f"MEMORY [{o.get('home', '')}]: {e.get('claim', '')} "
                    f"(confidence: {e.get('confidence', '')})")
        if k == "curation_trace":
            t = o.get("trace") or {}
            return (f"CURATION [{o.get('home', '')}]: {t.get('action', '')} "
                    f"{t.get('relation', '')}: {t.get('claim', '')} "
                    f"because {t.get('reasoning', '')}")
    except Exception:
        pass
    return json.dumps(o)


def parse_kv(spec, defaults):
    out = dict(defaults)
    for part in (spec or "").split(","):
        if "=" in part:
            k, v = part.split("=", 1)
            try:
                out[k.strip()] = float(v)
            except ValueError:
                pass
    return out


def parse_kv_mixed(spec, defaults):
    """parse_kv that keeps non-numeric values as strings (the user
    gate's mode= rides beside its numeric thresholds)."""
    out = dict(defaults)
    for part in (spec or "").split(","):
        if "=" in part:
            k, v = part.split("=", 1)
            k, v = k.strip(), v.strip()
            try:
                out[k] = float(v)
            except ValueError:
                out[k] = v
    return out


def load_pointer_scorer(args, key):
    """A scorer for a soak-ledger key - how the user pointer material-
    izes weights. stub -> StubScorer; base:<src> -> the birth checkpoint
    from the base dir; cpt-* -> that ring checkpoint."""
    if key == "stub" or args.checkpoint == "stub":
        return StubScorer(tag=f"user:{key}")
    if key.startswith("base:"):
        if getattr(args, "backend", "nanochat") == "hf":
            return HFScorer(args.checkpoint)
        return NanochatScorer(args.checkpoint)
    ring_dir = os.path.join(args.data_dir, "checkpoints", key)
    if os.path.isfile(os.path.join(ring_dir, "delta.pt")):
        # a delta-ring key (S5's lora rung): base + every delta up to
        # and including this checkpoint, chronologically
        base = (HFScorer(args.checkpoint)
                if getattr(args, "backend", "nanochat") == "hf"
                else NanochatScorer(args.checkpoint))
        return merge_ring_deltas(base, args.data_dir, upto=key)
    if getattr(args, "backend", "nanochat") == "hf":
        raise RuntimeError(
            f"'{key}' is not a delta checkpoint - the hf backend rings "
            "deltas only (full hf saves wait for their own ruling)")
    return NanochatScorer.from_ring(args.checkpoint, ring_dir)


def user_agreement(scorer, n=8):
    """The watchdog's re-audit: the USER pointer's generated-verdict
    agreement against the newest held-out frontier pairs - the same
    measurement the trainer's gate makes, on the same prompts. None
    when there is nothing to measure (stub, or too few pairs)."""
    if isinstance(scorer, StubScorer) or len(HELDOUT_PAIRS) < 4:
        return None
    pairs = HELDOUT_PAIRS[-int(n):]
    total = 0.0
    for pr in pairs:
        completion = scorer.generate_text(
            [{"role": "user",
              "content": salience_prompt("?", pr["input"], 0, 0)}],
            max_tokens=64, temperature=0.01, top_k=1)
        sal, _why, _parsed = parse_salience(completion)
        if sal is None:
            sal = 0.5
        total += min(1.0, abs(sal - float(pr["target"])))
    return round(1.0 - total / len(pairs), 4)


def do_user_promote(args, reason):
    """Advance the user pointer onto the gate's READY candidate: load
    its weights fresh (never shared with the salience slots), swap, and
    persist. Returns (ok, payload)."""
    with LOCK:
        key = USER["ready"]
        snapshot = USER["ready_eval"]
    if not key:
        return False, {"status": "err", "msg": "no candidate is ready - "
                       "the user gate has not passed (soak/agree/regress)"}
    try:
        scorer = apply_persona_and_stack(args,
                                         load_pointer_scorer(args, key))
    except Exception as e:
        return False, {"status": "err", "msg": f"user pointer load failed: {e}"}
    with LOCK:
        prev = USER["pointer"]
        USER_SLOT["scorer"] = scorer
        USER["last_good"] = prev
        USER["pointer"] = key
        USER["name"] = ("user:" + key
                        + ("+lora" if "+lora" in getattr(scorer, "name", "") else "")
                        + ("+adp" if "+adp" in getattr(scorer, "name", "") else ""))
        USER["promoted_at"] = int(time.time() * 1000)
        USER["eval"] = snapshot
        USER["ready"] = None
        USER["ready_eval"] = None
        USER["promotions"] += 1
    save_user_state(args.data_dir)
    append_metric(args.data_dir, {"kind": "user_promote", "pointer": key,
                                  "prev": prev, "reason": reason,
                                  "eval": snapshot})
    print(f"[user] pointer -> {key} ({reason})", flush=True)
    return True, {"status": "ok", "pointer": key, "prev": prev,
                  "reason": reason, "eval": snapshot}


def do_user_rollback(args, reason):
    with LOCK:
        prev = USER["pointer"]
        target = USER["last_good"]
    if not target:
        return False, {"status": "err", "msg": "no last_good to roll back to"}
    try:
        scorer = apply_persona_and_stack(args,
                                         load_pointer_scorer(args, target))
    except Exception as e:
        return False, {"status": "err", "msg": f"rollback load failed: {e}"}
    with LOCK:
        USER_SLOT["scorer"] = scorer
        USER["pointer"] = target
        USER["name"] = ("user:" + target
                        + ("+lora" if "+lora" in getattr(scorer, "name", "") else "")
                        + ("+adp" if "+adp" in getattr(scorer, "name", "") else ""))
        USER["promoted_at"] = int(time.time() * 1000)
        USER["last_good"] = None    # one step back, deliberately - not a stack
        USER["rollbacks"] += 1
    save_user_state(args.data_dir)
    append_metric(args.data_dir, {"kind": "user_rollback", "from": prev,
                                  "to": target, "reason": reason})
    print(f"[user] ROLLED BACK {prev} -> {target} ({reason})", flush=True)
    return True, {"status": "ok", "pointer": target, "from": prev,
                  "reason": reason}


def user_gate_loop(args):
    """The stricter, slower lane. Every check_s: (1) decide whether the
    soaking salience pointer qualifies as the READY candidate - soak
    time and verdicts served on the fast lane, agreement from the
    trainer's last gate, no held-out-standard creep past the last user
    promotion; (2) in auto mode, promote it; (3) re-audit the SERVING
    user pointer against the newest held-out pairs and roll back to
    last_good if its agreement has decayed below agree - regress."""
    cfg = parse_kv_mixed(args.user_gate, USER_GATE_DEFAULTS)
    print(f"[user] gate: {cfg}", flush=True)
    # the watchdog's data must not depend on the trainer being on:
    # load persisted held-out pairs if nobody has yet
    pairs_path = os.path.join(args.data_dir, "heldout_pairs.jsonl")
    if os.path.exists(pairs_path) and not HELDOUT_PAIRS:
        try:
            with open(pairs_path) as f:
                for ln in f:
                    ln = ln.strip()
                    if ln:
                        try:
                            HELDOUT_PAIRS.append(json.loads(ln))
                        except Exception:
                            pass
        except Exception:
            pass
    while True:
        time.sleep(max(float(cfg["check_s"]), 1.0))
        try:
            with LOCK:
                cur = SOAK["current"]
                soak = dict(SOAK["rings"].get(cur) or {})
                serving = USER["pointer"]
                lg = dict(TRAINER.get("last_gate") or {})
                user_scorer = USER_SLOT["scorer"]
                prev_eval = dict(USER["eval"] or {})
            # a READY candidate that lost the fast lane (a newer
            # promotion took it) is stale: it qualifies only while it
            # IS the canary. Clear it; the new pointer soaks fresh.
            with LOCK:
                if USER["ready"] and USER["ready"] != cur:
                    print(f"[user] ready candidate {USER['ready']} superseded "
                          f"by {cur} on the fast lane - cleared", flush=True)
                    USER["ready"] = None
                    USER["ready_eval"] = None
            # (1) candidacy
            if cur and cur != serving:
                soaked = time.time() - soak.get("since", time.time())
                verdicts = int(soak.get("verdicts", 0))
                agree = lg.get("live_agree")
                std = lg.get("live_std")
                prev_std = prev_eval.get("std")
                soak_ok = (soaked >= float(cfg["soak_s"])
                           and verdicts >= int(cfg["verdicts"]))
                agree_ok = agree is None or agree >= float(cfg["agree"])
                std_ok = (std is None or prev_std is None
                          or std <= prev_std * (1 + float(cfg["regress"])))
                if soak_ok and agree_ok and std_ok:
                    snapshot = {"soak_s": int(soaked), "verdicts": verdicts,
                                "agree": agree, "std": std,
                                "at": int(time.time() * 1000)}
                    newly = False
                    with LOCK:
                        if USER["ready"] != cur:
                            USER["ready"] = cur
                            USER["ready_eval"] = snapshot
                            newly = True
                    if newly:
                        append_metric(args.data_dir, dict(
                            snapshot, kind="user_ready", pointer=cur))
                        print(f"[user] READY: {cur} {snapshot}", flush=True)
                    if str(cfg.get("mode")) == "auto":
                        do_user_promote(args, "auto")
            # (2b) the personality probe (8b): held-out persona loss of
            # the SERVING model vs the derivation-time baseline; slip
            # past slack -> re-derive in the background against the
            # pointer's current base. Corpus but no adapter yet -> the
            # first derivation fires on its own.
            lcfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
            if (str(lcfg.get("mode")) != "off" and user_scorer is not None
                    and serving and not isinstance(user_scorer, StubScorer)
                    and not PERSONA["deriving"]):
                _tr, _ho = load_persona_corpus(args.data_dir)
                with LOCK:
                    PERSONA["corpus"] = len(_tr) + len(_ho)
                if len(_tr) >= 4 and len(_ho) >= 1:
                    needs = False
                    if PERSONA["adapter"] is None:
                        needs = True   # corpus exists, no skin yet
                    else:
                        seq2 = (user_scorer.meta["model_config"]["sequence_len"]
                                if user_scorer.meta else 2048)
                        _um = scorer_model(user_scorer)
                        probe = persona_eval(_um,
                                             user_scorer.tokenizer, _ho,
                                             model_device(_um),
                                             seq2)
                        if probe is not None:
                            with LOCK:
                                PERSONA["probe"] = probe
                                base = PERSONA["baseline"]
                            append_metric(args.data_dir,
                                          {"kind": "persona_probe", "probe": probe,
                                           "baseline": base})
                            if base is not None and probe > base * (1 + float(lcfg["slack"])):
                                print(f"[persona] probe slipped ({probe} > "
                                      f"{base} * 1+{lcfg['slack']}) - re-deriving",
                                      flush=True)
                                needs = True
                    if needs:
                        threading.Thread(target=derive_adapter,
                                         args=(args, "probe" if PERSONA["adapter"] else "first"),
                                         daemon=True).start()
            # (2c) the stack probe (S4, ruling 3's cadence): each
            # applied member's subject held-out loss on the SERVING
            # model vs its derivation baseline; slip past slack is
            # surfaced (status + metric), never silently acted on -
            # unapply is the owner's deliberate act.
            with LOCK:
                probe_stack = list(ADAPTERS["stack"])
            if (probe_stack and user_scorer is not None and serving
                    and not isinstance(user_scorer, StubScorer)):
                _pm = scorer_model(user_scorer)
                _pdev = model_device(_pm)
                _pseq = (user_scorer.meta["model_config"]["sequence_len"]
                         if user_scorer.meta else 2048)
                import torch as _torch
                for _name in probe_stack:
                    try:
                        _blob = _torch.load(
                            adapter_blob_path(args.data_dir, _name),
                            map_location="cpu", weights_only=False)
                        _tr2, _ho2 = load_dataset_convs(
                            args, _blob["meta"].get("dataset", ""))
                        _probe = persona_eval(_pm, user_scorer.tokenizer,
                                              _ho2 or [], _pdev, _pseq)
                        _basel = _blob["meta"].get("heldout_adapted")
                        if _probe is not None:
                            _slipped = (_basel is not None and _probe
                                        > _basel * (1 + float(lcfg["slack"])))
                            with LOCK:
                                ADAPTERS["probes"][_name] = {
                                    "probe": _probe, "baseline": _basel,
                                    "slipped": _slipped}
                            append_metric(args.data_dir, {
                                "kind": "adapter_probe", "name": _name,
                                "probe": _probe, "baseline": _basel,
                                "slipped": _slipped})
                    except Exception as _e:
                        print(f"[adapters] probe of '{_name}' failed: "
                              f"{_e}", flush=True)
            # (3) the watchdog re-audit
            if user_scorer is not None and serving:
                fresh = user_agreement(user_scorer)
                if fresh is not None:
                    with LOCK:
                        USER["eval"] = dict(prev_eval, watch_agree=fresh,
                                            watch_at=int(time.time() * 1000))
                    if fresh < float(cfg["agree"]) - float(cfg["regress"]):
                        ok, _p = do_user_rollback(
                            args, f"watchdog: agreement {fresh} < "
                            f"{cfg['agree']} - {cfg['regress']}")
                        if not ok:
                            print(f"[user] watchdog wants rollback "
                                  f"(agree {fresh}) but has no last_good",
                                  flush=True)
        except Exception as e:
            print(f"[user] gate loop error: {e}", flush=True)


def restore_user_pointer(args):
    """Startup: the user pointer survives restarts. Load whatever
    user_pointer.json names, in the background, non-fatally."""
    path = user_state_path(args.data_dir)
    if not os.path.exists(path):
        return
    try:
        with open(path) as f:
            snap = json.load(f)
    except Exception as e:
        print(f"[user] state file unreadable: {e}", flush=True)
        return
    key = snap.get("pointer")
    if not key:
        return
    try:
        scorer = load_pointer_scorer(args, key)
    except Exception as e:
        print(f"[user] persisted pointer {key} failed to load: {e}", flush=True)
        return
    scorer = apply_persona_and_stack(args, scorer)
    with LOCK:
        USER_SLOT["scorer"] = scorer
        for k in ("pointer", "name", "promoted_at", "eval", "last_good"):
            USER[k] = snap.get(k)
        USER["promotions"] = int(snap.get("promotions") or 0)
        USER["rollbacks"] = int(snap.get("rollbacks") or 0)
        marks = ("+lora" if "+lora" in getattr(scorer, "name", "") else "") \
            + ("+adp" if "+adp" in getattr(scorer, "name", "") else "")
        if marks:
            USER["name"] = f"user:{key}{marks}"
    print(f"[user] restored pointer {key}", flush=True)


# ── Phase 8b: the personality adapter ────────────────────────────────
# A standing LoRA on the USER-FACING pointer only - the salience lane
# never wears it. Derived from the persona corpus
# (persona/persona.jsonl, one {"messages": [...]} or {"user":..,
# "assistant":..} per line; every 5th row is held out and never
# trained on). The base grows underneath; the adapter is re-derived as
# BACKGROUND MAINTENANCE when the probe says it slipped - the probe is
# held-out persona loss of the SERVING model, measured on the watchdog
# cadence, compared to the loss recorded at derivation time.
PERSONA = {
    "adapter": None,       # meta of the serving adapter (derived_from, at, rank)
    "baseline": None,      # held-out persona loss at derivation time
    "probe": None,         # latest probe of the serving model
    "deriving": False,
    "rederivations": 0,
    "last_result": None,
    "corpus": 0,
}
PERSONA_LORA_DEFAULTS = {"mode": "on", "rank": 8, "alpha": 16, "lr": 1e-3,
                         "steps": 200, "slack": 0.1, "min_gain": 0.01,
                         "guard": 0.2, "targets": "c_q.c_v"}


def persona_dir(data_dir):
    d = os.path.join(data_dir, "persona")
    os.makedirs(d, exist_ok=True)
    return d


def load_persona_corpus(data_dir):
    """(train_convs, heldout_convs) - every 5th row held out, never
    trained on. Rows normalize to nanochat conversations."""
    path = os.path.join(persona_dir(data_dir), "persona.jsonl")
    train, heldout = [], []
    if not os.path.exists(path):
        return train, heldout
    with open(path) as f:
        for i, ln in enumerate(f):
            ln = ln.strip()
            if not ln:
                continue
            try:
                o = json.loads(ln)
            except Exception:
                continue
            if "messages" in o:
                conv = {"messages": o["messages"]}
            elif "user" in o and "assistant" in o:
                conv = {"messages": [
                    {"role": "user", "content": str(o["user"])},
                    {"role": "assistant", "content": str(o["assistant"])}]}
            else:
                continue
            (heldout if i % 5 == 4 else train).append(conv)
    return train, heldout


def lora_target_paths(model, targets):
    """Module paths for the adapter, from a dot-separated target spec
    (c_q.c_v.attn_proj.c_fc.mlp_proj -> attention and MLP linears).
    Backend-aware (S3): one target vocabulary, two namings - nanochat's
    transformer.h.<i>.attn.c_q pattern, or the conventional q_proj/
    v_proj/... leaf names an HF model's named_modules carry. The
    hook-LoRA machinery downstream is architecture-blind either way -
    no adapter library needed (doctrine: avoid third-party deps)."""
    if hasattr(model, "transformer") and hasattr(model.transformer, "h"):
        names = {"c_q": "attn.c_q", "c_k": "attn.c_k", "c_v": "attn.c_v",
                 "attn_proj": "attn.c_proj", "c_fc": "mlp.c_fc",
                 "mlp_proj": "mlp.c_proj"}
        picked = [names[t] for t in str(targets).split(".") if t in names]
        out = []
        n_layer = len(model.transformer.h)
        for i in range(n_layer):
            for sub in picked:
                out.append(f"transformer.h.{i}.{sub}")
        return out
    hf_names = {"c_q": "q_proj", "c_k": "k_proj", "c_v": "v_proj",
                "attn_proj": "o_proj", "c_fc": "up_proj",
                "mlp_proj": "down_proj"}
    hf_picked = {hf_names[t] for t in str(targets).split(".")
                 if t in hf_names}
    out = []
    for name, mod in model.named_modules():
        leaf = name.rsplit(".", 1)[-1]
        if leaf in hf_picked and hasattr(mod, "in_features"):
            out.append(name)
    return out


def persona_eval(model, tokenizer, convs, device, seq_len, limit=16):
    """Held-out persona loss: masked LM loss on assistant tokens only
    (render_conversation's mask; -1 is the model's ignore_index)."""
    import torch
    if not convs:
        return None
    was_training = model.training
    model.eval()
    tot, n = 0.0, 0
    with torch.no_grad():
        for conv in convs[:limit]:
            try:
                ids, mask = render_masked(tokenizer, conv,
                                          min(int(seq_len), 1024))
            except Exception:
                continue
            if len(ids) < 3 or sum(mask) == 0:
                continue
            tot += float(masked_lm_loss_t(model, tokenizer, ids, mask,
                                          device))
            n += 1
    if was_training:
        model.train()
    return round(tot / n, 4) if n else None


def plain_lm_eval(model, tokenizer, docs, device, seq_len, limit=12):
    """The derivation's forgetting guard: plain LM loss on standard
    docs - a personality must not lobotomize the base."""
    import torch
    toks = []
    for d in docs:
        toks.extend(encode_doc(tokenizer, d))
    seq_len = min(int(seq_len), 2048)
    chunks = [toks[i:i + seq_len + 1]
              for i in range(0, len(toks) - seq_len - 1, seq_len)][:limit]
    if not chunks:
        return None
    was_training = model.training
    model.eval()
    tot = 0.0
    with torch.no_grad():
        for c in chunks:
            tot += float(plain_lm_loss_t(model, tokenizer, c, device))
    if was_training:
        model.train()
    return round(tot / len(chunks), 4)


def adapter_path(data_dir):
    return os.path.join(persona_dir(data_dir), "adapter.pt")


def apply_persona(args, scorer):
    """Merge the standing adapter into a freshly loaded user scorer's
    weights (W += scale * B@A per target). The adapter was derived from
    SOME base; applying it to the pointer's CURRENT base is the
    standing-skin-on-a-moving-base design - drift is what the probe
    watches. No-op in stub mode, with mode=off, or with no adapter."""
    cfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
    if (str(cfg.get("mode")) == "off" or isinstance(scorer, StubScorer)
            or not os.path.exists(adapter_path(args.data_dir))):
        return scorer
    try:
        import torch
        blob = torch.load(adapter_path(args.data_dir), map_location="cpu",
                          weights_only=False)
        scale = float(blob["alpha"]) / float(blob["rank"])
        model = scorer_model(scorer)
        applied = 0
        with torch.no_grad():
            for path, ab in blob["state"].items():
                try:
                    w = model.get_submodule(path).weight
                except AttributeError:
                    continue
                delta = (ab["B"].float() @ ab["A"].float()) * scale
                if delta.shape == w.shape:
                    w.add_(delta.to(w.device, w.dtype))
                    applied += 1
        if applied:
            with LOCK:
                PERSONA["adapter"] = dict(blob["meta"])
                PERSONA["baseline"] = blob["meta"].get("heldout_adapted")
            scorer.name = scorer.name + "+lora"
            print(f"[persona] adapter merged into {applied} linears "
                  f"(derived from {blob['meta'].get('derived_from')})", flush=True)
    except Exception as e:
        print(f"[persona] adapter apply failed (serving bare base): {e}", flush=True)
    return scorer


def derive_adapter(args, reason):
    """Derive (or re-derive) the personality adapter from the persona
    corpus against the user pointer's CURRENT base. Gate: held-out
    persona loss must improve by min_gain over the bare base, and
    standard LM loss must not rise past guard. Accept -> save + swap
    the serving scorer; reject -> report and leave serving alone."""
    cfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
    with LOCK:
        if PERSONA["deriving"]:
            return {"status": "err", "msg": "derivation already running"}
        key = USER["pointer"]
        if not key:
            return {"status": "err", "msg": "no user pointer promoted - "
                    "the adapter rides the user lane"}
        PERSONA["deriving"] = True
    try:
        return _derive_adapter_inner(args, cfg, key, reason)
    finally:
        with LOCK:
            PERSONA["deriving"] = False


def _derive_adapter_inner(args, cfg, key, reason):
    import torch
    if args.checkpoint == "stub":
        return {"status": "err", "msg": "stub mode has no weights to adapt"}
    train, heldout = load_persona_corpus(args.data_dir)
    with LOCK:
        PERSONA["corpus"] = len(train) + len(heldout)
    if len(train) < 4 or len(heldout) < 1:
        return {"status": "err",
                "msg": f"persona corpus too small ({len(train)} train / "
                       f"{len(heldout)} held-out; need 4/1) - add rows to "
                       f"persona/persona.jsonl"}
    t0 = time.time()
    scorer = load_pointer_scorer(args, key)   # a fresh copy; never the serving one
    model, tokenizer = scorer_model(scorer), scorer.tokenizer
    device = model_device(model)
    seq_len = scorer.meta["model_config"]["sequence_len"] if scorer.meta else 2048
    base_heldout = persona_eval(model, tokenizer, heldout, device, seq_len)
    standard = load_standard_docs(args, limit=64)
    base_std = plain_lm_eval(model, tokenizer, standard[:24], device, seq_len)
    # attach LoRA via forward hooks: y = Wx + (alpha/rank) * B(A(x))
    rank, alpha = int(cfg["rank"]), float(cfg["alpha"])
    scale = alpha / rank
    paths = lora_target_paths(model, cfg["targets"])
    ab, hooks = {}, []
    for path in paths:
        try:
            lin = model.get_submodule(path)
        except AttributeError:
            continue
        A = torch.zeros(rank, lin.in_features, device=device,
                        dtype=torch.float32).normal_(0, 0.02).requires_grad_(True)
        Bm = torch.zeros(lin.out_features, rank, device=device,
                         dtype=torch.float32).requires_grad_(True)
        ab[path] = (A, Bm)

        def mk_hook(A=A, Bm=Bm):
            def hook(_mod, inputs, output):
                x = inputs[0]
                return output + (x.float() @ A.t() @ Bm.t()).to(output.dtype) * scale
            return hook
        hooks.append(lin.register_forward_hook(mk_hook()))
    if not ab:
        return {"status": "err", "msg": "no LoRA targets matched the model"}
    params = [t for pair in ab.values() for t in pair]
    opt = torch.optim.AdamW(params, lr=float(cfg["lr"]))
    model.train()
    import random as _random
    steps = int(cfg["steps"])
    for step in range(steps):
        conv = train[_random.randrange(len(train))]
        try:
            ids, mask = render_masked(tokenizer, conv,
                                      min(int(seq_len), 1024))
        except Exception:
            continue
        if len(ids) < 3 or sum(mask) == 0:
            continue
        loss = masked_lm_loss_t(model, tokenizer, ids, mask, device)
        opt.zero_grad()
        loss.backward()
        opt.step()
    model.eval()
    adapted_heldout = persona_eval(model, tokenizer, heldout, device, seq_len)
    adapted_std = plain_lm_eval(model, tokenizer, standard[:24], device, seq_len)
    for h in hooks:
        h.remove()
    gain_ok = (adapted_heldout is not None and base_heldout is not None
               and adapted_heldout <= base_heldout * (1 - float(cfg["min_gain"])))
    guard_ok = (adapted_std is None or base_std is None
                or adapted_std <= base_std * (1 + float(cfg["guard"])))
    report = {
        "derived_from": key, "reason": reason, "rank": rank, "alpha": alpha,
        "steps": steps, "train_rows": len(train), "heldout_rows": len(heldout),
        "heldout_base": base_heldout, "heldout_adapted": adapted_heldout,
        "std_base": base_std, "std_adapted": adapted_std,
        "gain_ok": gain_ok, "guard_ok": guard_ok,
        "seconds": int(time.time() - t0), "at": int(time.time() * 1000),
    }
    verdict = "accept" if (gain_ok and guard_ok) else "reject"
    append_metric(args.data_dir, dict(report, kind="persona_derive",
                                      verdict=verdict))
    with LOCK:
        PERSONA["last_result"] = dict(report, verdict=verdict)
    print(f"[persona] derivation {verdict}: {report}", flush=True)
    if verdict == "reject":
        return {"status": "err", "msg": "derivation rejected by its gate",
                **report}
    torch.save({"state": {p: {"A": A.detach().cpu(), "B": B.detach().cpu()}
                          for p, (A, B) in ab.items()},
                "rank": rank, "alpha": alpha, "meta": report},
               adapter_path(args.data_dir))
    # swap serving: a fresh base + the new adapter merged (and the
    # named-adapter stack re-applied on top - S4)
    fresh = apply_persona_and_stack(args, load_pointer_scorer(args, key))
    with LOCK:
        USER_SLOT["scorer"] = fresh
        USER["name"] = f"user:{key}+lora"
        PERSONA["rederivations"] += 1
        PERSONA["baseline"] = adapted_heldout
        PERSONA["probe"] = adapted_heldout
    return {"status": "ok", **report}


# ── S4: adapters on demand ──────────────────────────────────────────
# The persona pattern - corpus -> hook-LoRA -> gate -> apply -
# generalized to NAMED adapters: derived from any registered dataset
# against the serving pointer's base (or any registered model), gated
# exactly as persona is, and stacked on the user pointer under ruling
# 3: merged additive deltas commute, so there is no order - what is
# gated is the COMBINATION, re-validated whole on every stack change.
# Records live in the runtime library (the commands write them); the
# service owns the blobs and the stack. Persona stays the standing
# first skin - the stack stands on base+persona ground.

ADAPTERS = {"stack": [], "deriving": False, "last_report": None,
            "probes": {}}


def adapters_dir(data_dir):
    d = os.path.join(data_dir, "adapters")
    os.makedirs(d, exist_ok=True)
    return d


def adapter_blob_path(data_dir, name):
    return os.path.join(adapters_dir(data_dir), f"{name}.pt")


def stack_state_path(data_dir):
    return os.path.join(adapters_dir(data_dir), "stack.json")


def save_stack_state(data_dir):
    with LOCK:
        stack = list(ADAPTERS["stack"])
    try:
        tmp = stack_state_path(data_dir) + ".tmp"
        with open(tmp, "w") as f:
            json.dump(stack, f)
        os.replace(tmp, stack_state_path(data_dir))
    except Exception as e:
        print(f"[adapters] stack persist failed: {e}", flush=True)


def load_dataset_convs(args, dataset_name):
    """(train, heldout) conversations from a registered dataset - jsonl
    rows in the persona shapes ({"user","assistant"} or
    {"messages":[...]}). The dataset's holdout_every policy splits;
    0 falls back to every 5th - a derivation must hold something out
    for its gate to mean anything."""
    for rec in load_registry_datasets(args.data_dir):
        if rec.get("name") != dataset_name:
            continue
        hn = int(rec.get("holdout_every") or 0) or 5
        train, hold = [], []
        i = 0
        for path in dataset_paths(dict(rec, format="jsonl")):
            try:
                with open(path) as f:
                    for ln in f:
                        ln = ln.strip()
                        if not ln:
                            continue
                        try:
                            o = json.loads(ln)
                        except Exception:
                            continue
                        if "messages" in o:
                            conv = {"messages": o["messages"]}
                        elif "user" in o and "assistant" in o:
                            conv = {"messages": [
                                {"role": "user", "content": str(o["user"])},
                                {"role": "assistant",
                                 "content": str(o["assistant"])}]}
                        else:
                            continue
                        (hold if i % hn == hn - 1 else train).append(conv)
                        i += 1
            except OSError:
                continue
        return train, hold
    return None, None


def load_base_scorer(args, base):
    """The adapter's base: 'pointer' -> a fresh copy of the serving
    user pointer's weights; a registered model name -> that record's
    backend + path, loaded fresh."""
    if base == "pointer":
        with LOCK:
            key = USER["pointer"]
        if not key:
            raise RuntimeError("no user pointer promoted - promote one "
                               "or name a registered model as base")
        return load_pointer_scorer(args, key), f"pointer:{key}"
    path = os.path.join(args.data_dir, "registry.json")
    try:
        with open(path) as f:
            reg = json.load(f)
    except Exception:
        reg = {}
    for m in reg.get("models", []):
        if m.get("name") == base:
            if m.get("backend") == "hf":
                return HFScorer(m.get("path")), f"model:{base}"
            return NanochatScorer(m.get("path")), f"model:{base}"
    raise RuntimeError(f"'{base}' is neither 'pointer' nor a "
                       "registered model")


def merge_adapter_blob(model, blob):
    """W += (alpha/rank) * B@A per target - the additive merge both
    persona and the stack use. Returns how many linears took it."""
    import torch
    scale = float(blob["alpha"]) / float(blob["rank"])
    applied = 0
    with torch.no_grad():
        for path, ab in blob["state"].items():
            try:
                w = model.get_submodule(path).weight
            except AttributeError:
                continue
            delta = (ab["B"].float() @ ab["A"].float()) * scale
            if delta.shape == w.shape:
                w.add_(delta.to(w.device, w.dtype))
                applied += 1
    return applied


def derive_named_adapter(args, spec):
    """Derive one named adapter: corpus from a registered dataset,
    base per spec, hook-LoRA training, the persona gate (min_gain on
    the subject's held-out loss, guard on standard loss). Saves the
    blob; never applies - apply is its own deliberate, gated act."""
    import torch
    cfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
    name = spec["name"]
    with LOCK:
        if ADAPTERS["deriving"] or PERSONA["deriving"]:
            return {"status": "err", "msg": "a derivation is already running"}
        ADAPTERS["deriving"] = True
    t0 = time.time()
    try:
        train, hold = load_dataset_convs(args, spec["dataset"])
        if train is None:
            return {"status": "err",
                    "msg": f"dataset '{spec['dataset']}' is not registered"}
        if len(train) < 4 or len(hold) < 1:
            return {"status": "err",
                    "msg": f"corpus too small ({len(train)} train / "
                           f"{len(hold)} held-out; need 4/1)"}
        scorer, base_ref = load_base_scorer(args, spec["base"])
        model, tokenizer = scorer_model(scorer), scorer.tokenizer
        device = model_device(model)
        seq_len = (scorer.meta["model_config"]["sequence_len"]
                   if scorer.meta else 2048)
        base_heldout = persona_eval(model, tokenizer, hold, device, seq_len)
        standard = load_standard_docs(args, limit=64)
        base_std = plain_lm_eval(model, tokenizer, standard[:24], device,
                                 seq_len)
        rank = int(spec.get("rank") or cfg["rank"])
        alpha = float(cfg["alpha"]) / float(cfg["rank"]) * rank
        scale = alpha / rank
        steps = int(spec.get("steps") or cfg["steps"])
        paths = lora_target_paths(model, spec.get("targets")
                                  or cfg["targets"])
        ab, hooks = {}, []
        for path in paths:
            try:
                lin = model.get_submodule(path)
            except AttributeError:
                continue
            A = torch.zeros(rank, lin.in_features, device=device,
                            dtype=torch.float32).normal_(0, 0.02
                                                         ).requires_grad_(True)
            Bm = torch.zeros(lin.out_features, rank, device=device,
                             dtype=torch.float32).requires_grad_(True)
            ab[path] = (A, Bm)

            def mk_hook(A=A, Bm=Bm):
                def hook(_mod, inputs, output):
                    x = inputs[0]
                    return output + (x.float() @ A.t() @ Bm.t()
                                     ).to(output.dtype) * scale
                return hook
            hooks.append(lin.register_forward_hook(mk_hook()))
        if not ab:
            return {"status": "err",
                    "msg": "no LoRA targets matched the base model"}
        params = [t for pair in ab.values() for t in pair]
        opt = torch.optim.AdamW(params, lr=float(cfg["lr"]))
        model.train()
        import random as _random
        for _step in range(steps):
            conv = train[_random.randrange(len(train))]
            try:
                ids, mask = render_masked(tokenizer, conv,
                                          min(int(seq_len), 1024))
            except Exception:
                continue
            if len(ids) < 3 or sum(mask) == 0:
                continue
            loss = masked_lm_loss_t(model, tokenizer, ids, mask, device)
            opt.zero_grad()
            loss.backward()
            opt.step()
        model.eval()
        adapted_heldout = persona_eval(model, tokenizer, hold, device,
                                       seq_len)
        adapted_std = plain_lm_eval(model, tokenizer, standard[:24],
                                    device, seq_len)
        for h in hooks:
            h.remove()
        gain_ok = (adapted_heldout is not None and base_heldout is not None
                   and adapted_heldout
                   <= base_heldout * (1 - float(cfg["min_gain"])))
        guard_ok = (adapted_std is None or base_std is None
                    or adapted_std <= base_std * (1 + float(cfg["guard"])))
        report = {
            "name": name, "dataset": spec["dataset"], "base": base_ref,
            "rank": rank, "alpha": alpha, "steps": steps,
            "targets": spec.get("targets") or cfg["targets"],
            "train_rows": len(train), "heldout_rows": len(hold),
            "heldout_base": base_heldout,
            "heldout_adapted": adapted_heldout,
            "std_base": base_std, "std_adapted": adapted_std,
            "gain_ok": gain_ok, "guard_ok": guard_ok,
            "seconds": int(time.time() - t0),
            "at": int(time.time() * 1000),
        }
        verdict = "accept" if (gain_ok and guard_ok) else "reject"
        append_metric(args.data_dir, dict(report, kind="adapter_derive",
                                          verdict=verdict))
        with LOCK:
            ADAPTERS["last_report"] = dict(report, verdict=verdict)
        print(f"[adapters] derive {name}: {verdict} {report}", flush=True)
        if verdict == "reject":
            return {"status": "err",
                    "msg": "derivation rejected by its gate", **report}
        torch.save({"state": {p: {"A": A.detach().cpu(),
                                  "B": B.detach().cpu()}
                              for p, (A, B) in ab.items()},
                    "rank": rank, "alpha": alpha, "meta": report},
                   adapter_blob_path(args.data_dir, name))
        del scorer
        try:
            torch.cuda.empty_cache()
        except Exception:
            pass
        return {"status": "ok", **report}
    finally:
        with LOCK:
            ADAPTERS["deriving"] = False


def apply_stack_gated(args, new_stack, reason):
    """Ruling 3, executed: rebuild the user scorer from a FRESH base +
    persona + every member merged (additive deltas commute; rebuilding
    beats subtracting in bf16), then gate the COMBINATION - every
    member's subject held-out loss with the full stack applied must
    still clear min_gain against the fresh bare base, and one standard
    -loss guard covers the whole stack. Pass -> swap serving; fail ->
    serving untouched, the numbers say which member broke."""
    import torch
    cfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
    with LOCK:
        key = USER["pointer"]
    if not key:
        return {"status": "err", "msg": "no user pointer promoted"}
    try:
        scorer = apply_persona(args, load_pointer_scorer(args, key))
    except Exception as e:
        return {"status": "err", "msg": f"base load failed: {e}"}
    if isinstance(scorer, StubScorer):
        return {"status": "err", "msg": "stub mode has no weights to stack"}
    model, tokenizer = scorer_model(scorer), scorer.tokenizer
    device = model_device(model)
    seq_len = (scorer.meta["model_config"]["sequence_len"]
               if scorer.meta else 2048)
    holds, blobs = {}, {}
    for name in new_stack:
        try:
            blobs[name] = torch.load(adapter_blob_path(args.data_dir, name),
                                     map_location="cpu", weights_only=False)
        except Exception as e:
            return {"status": "err", "msg": f"adapter '{name}' blob "
                                            f"unreadable: {e}"}
        _tr, hold = load_dataset_convs(
            args, blobs[name]["meta"].get("dataset", ""))
        holds[name] = hold or []
    bare = {name: persona_eval(model, tokenizer, holds[name], device,
                               seq_len) for name in new_stack}
    standard = load_standard_docs(args, limit=64)
    std_bare = plain_lm_eval(model, tokenizer, standard[:24], device,
                             seq_len)
    for name in new_stack:
        merge_adapter_blob(model, blobs[name])
    stacked = {name: persona_eval(model, tokenizer, holds[name], device,
                                  seq_len) for name in new_stack}
    std_stacked = plain_lm_eval(model, tokenizer, standard[:24], device,
                                seq_len)
    members = {}
    all_ok = True
    for name in new_stack:
        ok = (stacked[name] is None or bare[name] is None
              or stacked[name] <= bare[name] * (1 - float(cfg["min_gain"])))
        members[name] = {"bare": bare[name], "stacked": stacked[name],
                         "gain_ok": ok}
        all_ok = all_ok and ok
    guard_ok = (std_stacked is None or std_bare is None
                or std_stacked <= std_bare * (1 + float(cfg["guard"])))
    report = {"stack": list(new_stack), "members": members,
              "std_bare": std_bare, "std_stacked": std_stacked,
              "guard_ok": guard_ok, "reason": reason,
              "at": int(time.time() * 1000)}
    verdict = "accept" if (all_ok and guard_ok) else "reject"
    append_metric(args.data_dir, dict(report, kind="adapter_stack",
                                      verdict=verdict))
    with LOCK:
        ADAPTERS["last_report"] = dict(report, verdict=verdict)
    print(f"[adapters] stack {new_stack}: {verdict}", flush=True)
    if verdict == "reject":
        del model
        return {"status": "err",
                "msg": "stack rejected by the unit gate", **report}
    if new_stack:
        scorer.name = scorer.name + "+adp:" + ",".join(new_stack)
    with LOCK:
        USER_SLOT["scorer"] = scorer
        USER["name"] = "user:" + key + (
            "+lora" if "+lora" in getattr(scorer, "name", "") else "") + (
            "+adp" if new_stack else "")
        ADAPTERS["stack"] = list(new_stack)
        ADAPTERS["probes"] = {}
    save_stack_state(args.data_dir)
    return {"status": "ok", **report}


def apply_persona_and_stack(args, scorer):
    """The re-apply path (promote, rollback, restart): persona first,
    then the persisted stack merged UNGATED - the gate ran when the
    stack was formed; base movement is re-audited by the watchdog's
    stack probe on its own cadence."""
    scorer = apply_persona(args, scorer)
    if isinstance(scorer, StubScorer):
        return scorer
    try:
        with open(stack_state_path(args.data_dir)) as f:
            stack = json.load(f)
    except Exception:
        return scorer
    if not stack:
        return scorer
    import torch
    model = scorer_model(scorer)
    applied = []
    for name in stack:
        try:
            blob = torch.load(adapter_blob_path(args.data_dir, name),
                              map_location="cpu", weights_only=False)
            if merge_adapter_blob(model, blob):
                applied.append(name)
        except Exception as e:
            print(f"[adapters] re-apply of '{name}' failed: {e}",
                  flush=True)
    if applied:
        scorer.name = scorer.name + "+adp:" + ",".join(applied)
        with LOCK:
            ADAPTERS["stack"] = applied
        print(f"[adapters] stack re-applied: {applied}", flush=True)
    return scorer


def load_standard_docs(args, limit=512):
    """Standard pretraining data for the replay mix: the base-dir's own
    parquet shards when pyarrow can read them, else standard.txt in the
    data dir (one doc per line - the test harness path), else none."""
    docs = []
    try:
        import glob as _glob
        import pyarrow.parquet as pq
        for shard in sorted(_glob.glob(os.path.join(
                args.checkpoint, "base_data*", "*.parquet")))[:2]:
            tbl = pq.read_table(shard, columns=["text"])
            for v in tbl.column("text")[:limit - len(docs)]:
                docs.append(str(v))
            if len(docs) >= limit:
                break
    except Exception:
        pass
    if not docs:
        alt = os.path.join(args.data_dir, "standard.txt")
        if os.path.exists(alt):
            with open(alt) as f:
                docs = [ln.strip() for ln in f if ln.strip()][:limit]
    return docs


# ── S5: the posture solver and the delta trainer ────────────────────

TRAIN_TLS = threading.local()


def solve_posture(args, live):
    """Ruling 4: fits = free memory minus the margin (the serving
    model already resides, so 'free' is what remains beside it). The
    ladder on this branch: full -> lora(r) -> frozen; full-sharded
    waits for S7 placement, qlora for a quantization-dependency
    ruling; full stays nanochat-only until the ring learns full hf
    saves. MODEL_POSTURE= forces a rung, published; a forced rung
    that does not fit REFUSES training - loudly, with the arithmetic -
    rather than OOMing at step one. Unknown free memory is permissive
    and says so ('source': none)."""
    model = scorer_model(live)
    params = sum(p.numel() for p in model.parameters())
    try:
        bytes_per = next(model.parameters()).element_size()
    except StopIteration:
        bytes_per = 4
    weights_mb = int(params * bytes_per / 1048576)
    res = probe_resources(args.data_dir)
    gpus = res.get("gpus") or []
    if gpus:
        free_mb = gpus[0]["free_mb"]
        total_mb = gpus[0]["total_mb"]
        source = f"gpu{gpus[0]['index']}"
    else:
        free_mb = res.get("ram_free_mb")
        total_mb = free_mb
        source = "meminfo" if free_mb is not None else "none"
    headroom_pct = float(getattr(args, "headroom", "15") or 15)
    margin_mb = int((total_mb or 0) * headroom_pct / 100)
    # full: a deepcopy candidate + fp32 AdamW moments + grads
    need_full = weights_mb + int(params * 12 / 1048576)
    # lora: tiny A/B + backward activations through the frozen base
    need_lora = 500 + int(weights_mb * 0.05)
    backend = "hf" if isinstance(live, HFScorer) else "nanochat"
    arith = {"params": params, "weights_mb": weights_mb,
             "free_mb": free_mb, "margin_mb": margin_mb,
             "need_full_mb": need_full, "need_lora_mb": need_lora,
             "source": source, "backend": backend}

    def fits(need):
        return free_mb is None or need + margin_mb <= free_mb

    forced = str(getattr(args, "posture", "auto") or "auto")
    if forced != "auto":
        rung = forced.split("(", 1)[0]
        ok = {"full": backend == "nanochat" and fits(need_full),
              "lora": fits(need_lora),
              "frozen": True}.get(rung)
        if ok is None:
            return "refused", dict(
                arith, forced=forced,
                why=f"unknown posture '{forced}' (full|lora|frozen)")
        if not ok:
            need = need_full if rung == "full" else need_lora
            why = (f"forced '{forced}' does not fit: need {need}MB + "
                   f"{margin_mb}MB margin, free {free_mb}MB ({source})"
                   if backend != "hf" or rung != "full" else
                   "full is nanochat-only until the ring learns full "
                   "hf saves")
            return "refused", dict(arith, forced=forced, why=why)
        return rung, dict(arith, forced=forced)
    if backend == "nanochat" and fits(need_full):
        return "full", arith
    if fits(need_lora):
        return "lora", arith
    return "frozen", arith


def merge_ring_deltas(scorer, data_dir, upto=None):
    """Reconstruct merged CPT state from a delta ring: each delta blob
    was trained relative to the base plus every earlier merge, so
    merging chronologically reconstructs exactly. upto=<dir basename>
    stops after that checkpoint (a pointer's view); None merges all -
    the restart-resume path."""
    import glob as _g
    import torch
    model = scorer_model(scorer)
    merged = 0
    last = None
    for d in sorted(_g.glob(os.path.join(data_dir, "checkpoints", "cpt-*"))):
        f = os.path.join(d, "delta.pt")
        if not os.path.isfile(f):
            continue
        try:
            blob = torch.load(f, map_location="cpu", weights_only=False)
        except Exception as e:
            print(f"[service] delta {d} unreadable: {e}", flush=True)
            continue
        if merge_adapter_blob(model, blob):
            merged += 1
            last = os.path.basename(d)
        if upto and os.path.basename(d) == upto:
            break
    if merged and last:
        base = scorer.name.split(":cpt-", 1)[0]
        step = last.rsplit("-", 1)[-1].lstrip("0") or "0"
        scorer.name = f"{base}:cpt-{int(step)}"
        print(f"[service] {merged} ring delta(s) merged -> {scorer.name}",
              flush=True)
    return scorer


def prune_ring(args):
    """Ruling 5: the ring prunes by BYTE budget, with two floors that
    hold regardless - the protected user set never prunes, and neither
    does the newest entry (the ring is the restart-recovery path and
    must never empty). Full checkpoints and delta blobs meter the same
    budget."""
    ckroot = os.path.join(args.data_dir, "checkpoints")
    if not os.path.isdir(ckroot):
        return
    budget = float(getattr(args, "ring_gb", "100") or 100) * 1073741824

    def du(d):
        t = 0
        for base, _dd, files in os.walk(os.path.join(ckroot, d)):
            for n in files:
                try:
                    t += os.path.getsize(os.path.join(base, n))
                except OSError:
                    pass
        return t

    entries = sorted(e for e in os.listdir(ckroot)
                     if os.path.isdir(os.path.join(ckroot, e)))
    if not entries:
        return
    sizes = {e: du(e) for e in entries}
    with LOCK:
        protected = {v for v in (USER["pointer"], USER["last_good"],
                                 USER["ready"]) if v}
    total = sum(sizes.values())
    for old in entries[:-1]:
        if total <= budget:
            break
        if old in protected:
            continue
        shutil.rmtree(os.path.join(ckroot, old), ignore_errors=True)
        total -= sizes[old]


def trainer_lora_loop(args, live, arith):
    """The lora rung (S5): CPT as base + delta. The delta is hook-LoRA
    on the SERVING model's own weights - zero-copy - with the hooks
    gated per-thread (TRAIN_TLS): only the trainer thread flips its
    thread-local on for its steps and candidate evals, so serving
    threads never see the candidate. Same drain, same mix, same gates
    as the full rung. Promotion MERGES the delta into the serving
    weights (additive; a verdict generated mid-merge could straddle
    old and new weights once - the epsilon audit is the standing net)
    and rings the delta blob with its base ref: megabytes, not
    gigabytes. Reset zeroes the delta."""
    import torch
    import random as _r
    mix = parse_kv(args.mix, {"fresh": 0.25, "replay": 0.25,
                              "standard": 0.5})
    gate_cfg = parse_kv(args.gate, {"every": 50, "regress": 0.02,
                                    "fails": 3, "agree_slack": 0.05,
                                    "agree_n": 8})
    cfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
    model, tokenizer = scorer_model(live), live.tokenizer
    device = model_device(model)
    seq_len = min(int(live.meta["model_config"]["sequence_len"]
                      if live.meta else 2048), 2048)
    for p in model.parameters():
        p.requires_grad_(False)
    rank = int(cfg["rank"])
    scale = 2.0  # alpha = 2 * rank
    paths = lora_target_paths(model, cfg["targets"])
    ab, hooks = {}, []
    for path in paths:
        try:
            lin = model.get_submodule(path)
        except AttributeError:
            continue
        A = torch.zeros(rank, lin.in_features, device=device,
                        dtype=torch.float32).normal_(0, 0.02
                                                     ).requires_grad_(True)
        Bm = torch.zeros(lin.out_features, rank, device=device,
                         dtype=torch.float32).requires_grad_(True)
        ab[path] = (A, Bm)

        def mk_hook(A=A, Bm=Bm):
            def hook(_mod, inputs, output):
                if not getattr(TRAIN_TLS, "on", False):
                    return output
                x = inputs[0]
                return output + (x.float() @ A.t() @ Bm.t()
                                 ).to(output.dtype) * scale
            return hook
        hooks.append(lin.register_forward_hook(mk_hook()))
    if not ab:
        with LOCK:
            TRAINER["active"] = False
            TRAINER["posture"] = "frozen - no LoRA targets matched"
        return

    def fresh_opt():
        return torch.optim.AdamW([t for pr in ab.values() for t in pr],
                                 lr=float(args.lr))

    def zero_delta():
        with torch.no_grad():
            for A, Bm in ab.values():
                A.normal_(0, 0.02)
                Bm.zero_()

    opt = fresh_opt()
    replay_path = os.path.join(args.data_dir, "replay.jsonl")
    heldout_path = os.path.join(args.data_dir, "heldout.jsonl")
    pairs_path = os.path.join(args.data_dir, "heldout_pairs.jsonl")
    replay = []
    for path, dest in ((replay_path, replay), (heldout_path, HELDOUT_FRESH)):
        if os.path.exists(path):
            with open(path) as f:
                dest.extend(ln.strip() for ln in f if ln.strip())
    if os.path.exists(pairs_path) and not HELDOUT_PAIRS:
        with open(pairs_path) as f:
            for ln in f:
                ln = ln.strip()
                if ln:
                    try:
                        HELDOUT_PAIRS.append(json.loads(ln))
                    except Exception:
                        pass
    standard = load_standard_docs(args)
    anchor_docs = load_anchor_docs(args)
    if anchor_docs:
        heldout_std = anchor_docs[:24]
    else:
        heldout_std = standard[:24]
        standard = standard[24:]
    ds_pools = {}
    ds_holdout = {}

    def refresh_ds_pools():
        for rec in load_registry_datasets(args.data_dir):
            nm = rec.get("name")
            if not nm or nm in ("fresh", "replay", "standard"):
                continue
            if mix.get(nm, 0) <= 0:
                continue
            tr, ho = load_dataset_docs(rec)
            if tr:
                ds_pools[nm] = tr
            if ho:
                ds_holdout[nm] = ho

    refresh_ds_pools()
    with LOCK:
        TRAINER["active"] = True
        TRAINER["mix"] = mix
        TRAINER["standard_docs"] = len(standard)
        TRAINER["replay_size"] = len(replay)
        TRAINER["dataset_pools"] = {k: len(v) for k, v in ds_pools.items()}
        TRAINER["anchor"] = bool(anchor_docs)

    def chunks_of(docs):
        toks = []
        for d in docs:
            toks.extend(encode_doc(tokenizer, d))
        return [toks[i:i + seq_len + 1]
                for i in range(0, len(toks) - seq_len - 1, seq_len)]

    def delta_on(fn, *a):
        TRAIN_TLS.on = True
        try:
            return fn(*a)
        finally:
            TRAIN_TLS.on = False

    def eval_chunks(chs):
        if not chs:
            return None
        import torch as _t
        tot = 0.0
        with _t.no_grad():
            for c in chs[:16]:
                tot += float(plain_lm_loss_t(model, tokenizer, c, device))
        return tot / min(len(chs), 16)

    def agreement(n, as_candidate):
        pairs = HELDOUT_PAIRS[-int(n):]
        if not pairs:
            return None
        total = 0.0
        for pr in pairs:
            def gen():
                return live.generate_text(
                    [{"role": "user",
                      "content": salience_prompt("?", pr["input"], 0, 0)}],
                    max_tokens=64, temperature=0.01, top_k=1)
            completion = delta_on(gen) if as_candidate else gen()
            sal, _w, _p = parse_salience(completion)
            if sal is None:
                sal = 0.5
            total += min(1.0, abs(sal - float(pr["target"])))
        return round(1.0 - total / len(pairs), 4)

    heldout_std_chunks = chunks_of(heldout_std)
    live_std = eval_chunks(heldout_std_chunks)
    live_fresh = None
    live_agree = None
    fails = 0
    ema = None
    tick = 0
    print(f"[trainer] lora rung on {live.name}: rank={rank} "
          f"targets={len(ab)} mix={mix} gate={gate_cfg}", flush=True)
    while True:
        time.sleep(max(float(args.train_interval), 0.05))
        if EXPERIMENT["running"]:
            continue   # the bench borrows the time-share (ruling 7)
        tick += 1
        if tick % 30 == 1:
            refresh_ds_pools()
            with LOCK:
                TRAINER["dataset_pools"] = {k: len(v)
                                            for k, v in ds_pools.items()}
        with LOCK:
            fresh_in = list(TRAIN_Q)
            TRAIN_Q.clear()
        fresh_docs = []
        for entry in fresh_in:
            d = entry["doc"] if isinstance(entry, dict) else entry
            pair = entry.get("pair") if isinstance(entry, dict) else None
            if _r.random() < 0.1 and len(HELDOUT_FRESH) < 512:
                with LOCK:
                    HELDOUT_FRESH.append(d)
                with open(heldout_path, "a") as f:
                    f.write(d.replace("\n", " ") + "\n")
                if pair:
                    HELDOUT_PAIRS.append(pair)
                    with open(pairs_path, "a") as f:
                        f.write(json.dumps(pair) + "\n")
                    live_agree = None
                live_fresh = None
            else:
                fresh_docs.append(d)
                replay.append(d)
                with open(replay_path, "a") as f:
                    f.write(d.replace("\n", " ") + "\n")
        if len(replay) > 4096:
            _r.shuffle(replay)
            replay = replay[:4096]
            with open(replay_path, "w") as f:
                f.writelines(d + "\n" for d in replay)
        pools = dict(ds_pools)
        pools.update({"fresh": fresh_docs or replay, "replay": replay,
                      "standard": standard})
        avail = {k: p for k, p in pools.items() if p and mix.get(k, 0) > 0}
        if not avail:
            with LOCK:
                TRAINER["replay_size"] = len(replay)
            continue
        total_w = sum(mix[k] for k in avail)
        docs = []
        for k, pool in avail.items():
            n = max(1, round(6 * mix[k] / total_w))
            docs.extend(_r.choice(pool) for _ in range(n))
        chs = chunks_of(docs)
        if not chs:
            continue
        _r.shuffle(chs)
        try:
            TRAIN_TLS.on = True
            loss = plain_lm_loss_t(model, tokenizer, chs[0], device)
            opt.zero_grad()
            loss.backward()
            opt.step()
        except torch.cuda.OutOfMemoryError:
            torch.cuda.empty_cache()
            print("[trainer] lora step OOM - skipped", flush=True)
            continue
        finally:
            TRAIN_TLS.on = False
        lv = float(loss.detach())
        ema = lv if ema is None else 0.98 * ema + 0.02 * lv
        with LOCK:
            TRAINER["steps"] += 1
            TRAINER["loss_ema"] = round(ema, 4)
            TRAINER["replay_size"] = len(replay)
            steps = TRAINER["steps"]
        if steps % 10 == 0:
            append_metric(args.data_dir, {"kind": "loss", "step": steps,
                                          "loss": round(ema, 4)})
        if steps % int(gate_cfg["every"]) != 0:
            continue
        cand_std = delta_on(eval_chunks, heldout_std_chunks)
        std_ok = (cand_std is None or live_std is None
                  or cand_std <= live_std * (1 + gate_cfg["regress"]))
        cand_fresh = live_fresh2 = cand_agree = None
        ds_losses = {}
        for nm, docs2 in ds_holdout.items():
            dl = delta_on(eval_chunks, chunks_of(docs2[:24]))
            if dl is not None:
                ds_losses[nm] = dl
        if len(HELDOUT_PAIRS) >= 4:
            if live_agree is None:
                live_agree = agreement(gate_cfg["agree_n"], False)
            cand_agree = agreement(gate_cfg["agree_n"], True)
            learn_ok = (cand_agree is None or live_agree is None
                        or cand_agree >= live_agree - gate_cfg["agree_slack"])
        else:
            heldout_fresh_chunks = chunks_of(HELDOUT_FRESH[-64:])
            if live_fresh is None:
                live_fresh = eval_chunks(heldout_fresh_chunks)
            live_fresh2 = live_fresh
            cand_fresh = delta_on(eval_chunks, heldout_fresh_chunks)
            learn_ok = (cand_fresh is None or live_fresh is None
                        or cand_fresh <= live_fresh)
        verdict = "promote" if (std_ok and learn_ok) else "hold"
        gate_row = {"step": steps, "verdict": verdict,
                    "cand_std": cand_std, "live_std": live_std,
                    "cand_fresh": cand_fresh, "live_fresh": live_fresh2,
                    "cand_agree": cand_agree, "live_agree": live_agree,
                    "pairs": len(HELDOUT_PAIRS),
                    "datasets": ds_losses or None,
                    "posture": "lora"}
        print(f"[trainer] gate: {gate_row}", flush=True)
        with LOCK:
            TRAINER["gates"] += 1
            TRAINER["last_gate"] = gate_row
        append_metric(args.data_dir, dict(gate_row, kind="gate"))
        if verdict == "promote":
            saved_key = None
            try:
                ckdir = os.path.join(
                    args.data_dir, "checkpoints",
                    f"cpt-{time.strftime('%Y%m%d%H%M%S')}-{steps:06d}")
                os.makedirs(ckdir, exist_ok=True)
                torch.save({"state": {p: {"A": A.detach().cpu(),
                                          "B": B.detach().cpu()}
                                      for p, (A, B) in ab.items()},
                            "rank": rank, "alpha": scale * rank,
                            "base_ref": live.name,
                            "backend": arith.get("backend"),
                            "step": steps},
                           os.path.join(ckdir, "delta.pt"))
                saved_key = os.path.basename(ckdir)
            except Exception as e:
                print(f"[trainer] delta ring save failed "
                      f"(promoting anyway): {e}", flush=True)
            with torch.no_grad():
                for path, (A, Bm) in ab.items():
                    try:
                        w = model.get_submodule(path).weight
                    except AttributeError:
                        continue
                    w.add_(((Bm.float() @ A.float()) * scale
                            ).to(w.device, w.dtype))
            zero_delta()
            opt = fresh_opt()
            base = live.name.split("+", 1)[0].split(":cpt-", 1)[0]
            live.name = f"{base}:cpt-{steps}"
            with LOCK:
                STATE["promotions"] += 1
                TRAINER["promotions"] += 1
            set_soak(saved_key)
            prune_ring(args)
            live_std = eval_chunks(heldout_std_chunks)
            live_fresh = None
            live_agree = None
            fails = 0
            print(f"[trainer] PROMOTED (merge) -> {live.name}", flush=True)
        else:
            fails += 1
            with LOCK:
                TRAINER["fails"] = fails
            if fails >= int(gate_cfg["fails"]):
                zero_delta()
                opt = fresh_opt()
                fails = 0
                with LOCK:
                    TRAINER["resets"] += 1
                    TRAINER["fails"] = 0
                print("[trainer] lora candidate reset", flush=True)


# ── S6: the bench ───────────────────────────────────────────────────
# Recipes snap the bricks together (records in the runtime library,
# resolved by the commands and passed here whole - the service stays
# store-blind); an experiment runs each arm as a fresh base + bounded
# delta and scores with the gates' own instruments on eval material
# PINNED at run start (ruling 7). On one card the bench borrows the
# standing trainer's time-share: candidate steps pause, serving never
# does, and the borrow is published.

EXPERIMENT = {"running": False, "current": None, "last": None}


def run_experiment_arm(args, recipe, budget_steps, eval_sets, pairs):
    """One arm: fresh base, bare measurement, a bounded hook-LoRA delta
    trained on the recipe's mix, delta measurement, weights freed. The
    delta is always-on-hooks - this model is private to the arm, never
    serving."""
    import torch
    import random as _r
    scorer, base_ref = load_base_scorer(args, recipe.get("base") or "pointer")
    model, tokenizer = scorer_model(scorer), scorer.tokenizer
    device = model_device(model)
    seq_len = min(int(scorer.meta["model_config"]["sequence_len"]
                      if scorer.meta else 2048), 2048)
    for p in model.parameters():
        p.requires_grad_(False)

    def chunks_of(docs):
        toks = []
        for d in docs:
            toks.extend(encode_doc(tokenizer, d))
        return [toks[i:i + seq_len + 1]
                for i in range(0, len(toks) - seq_len - 1, seq_len)]

    eval_chunks = {nm: chunks_of(docs) for nm, docs in eval_sets.items()}

    def eval_one(chs):
        if not chs:
            return None
        tot = 0.0
        with torch.no_grad():
            for c in chs[:16]:
                tot += float(plain_lm_loss_t(model, tokenizer, c, device))
        return round(tot / min(len(chs), 16), 4)

    def agreement():
        if len(pairs) < 4:
            return None
        sel = pairs[-8:]
        total = 0.0
        for pr in sel:
            completion = scorer.generate_text(
                [{"role": "user",
                  "content": salience_prompt("?", pr["input"], 0, 0)}],
                max_tokens=64, temperature=0.01, top_k=1)
            sal, _w, _p = parse_salience(completion)
            if sal is None:
                sal = 0.5
            total += min(1.0, abs(sal - float(pr["target"])))
        return round(1.0 - total / len(sel), 4)

    def measure():
        return {"evals": {nm: eval_one(chs)
                          for nm, chs in eval_chunks.items()},
                "agreement": agreement()}

    before = measure()
    mix = parse_kv(recipe.get("mix", ""), {})
    pools = {}
    for rec in load_registry_datasets(args.data_dir):
        nm = rec.get("name")
        if nm and mix.get(nm, 0) > 0:
            tr, _ho = load_dataset_docs(rec)
            if tr:
                pools[nm] = tr
    cfg = parse_kv_mixed(getattr(args, "lora", ""), PERSONA_LORA_DEFAULTS)
    rank = int(cfg["rank"])
    scale = 2.0
    ab, hooks = {}, []
    for path in lora_target_paths(model, cfg["targets"]):
        try:
            lin = model.get_submodule(path)
        except AttributeError:
            continue
        A = torch.zeros(rank, lin.in_features, device=device,
                        dtype=torch.float32).normal_(0, 0.02
                                                     ).requires_grad_(True)
        Bm = torch.zeros(lin.out_features, rank, device=device,
                         dtype=torch.float32).requires_grad_(True)
        ab[path] = (A, Bm)

        def mk_hook(A=A, Bm=Bm):
            def hook(_m, inputs, output):
                x = inputs[0]
                return output + (x.float() @ A.t() @ Bm.t()
                                 ).to(output.dtype) * scale
            return hook
        hooks.append(lin.register_forward_hook(mk_hook()))
    steps_run = 0
    if ab and pools:
        opt = torch.optim.AdamW([t for pr2 in ab.values() for t in pr2],
                                lr=float(recipe.get("lr") or args.lr))
        total_w = sum(mix[k] for k in pools)
        model.train()
        for _s in range(int(budget_steps)):
            docs = []
            for k, pool in pools.items():
                n = max(1, round(4 * mix[k] / total_w))
                docs.extend(_r.choice(pool) for _ in range(n))
            chs = chunks_of(docs)
            if not chs:
                continue
            loss = plain_lm_loss_t(model, tokenizer, chs[0], device)
            opt.zero_grad()
            loss.backward()
            opt.step()
            steps_run += 1
        model.eval()
    after = measure()
    for h in hooks:
        h.remove()
    result = {"base": base_ref,
              "pools": {k: len(v) for k, v in pools.items()},
              "steps_run": steps_run, "before": before, "after": after}
    del scorer, model
    try:
        torch.cuda.empty_cache()
    except Exception:
        pass
    return result


def run_experiment_thread(args, spec):
    """One bench run: pin the eval material (ruling 7 - frozen at run
    start, hashes recorded so the report names exactly what it
    measured), run each arm, append the report to experiments.jsonl.
    The standing trainer's candidate steps pause for the duration -
    serving never does - and the borrow rides /status."""
    with LOCK:
        EXPERIMENT["running"] = True
        EXPERIMENT["current"] = {"name": spec["name"],
                                 "started": int(time.time() * 1000),
                                 "arms": [a for a, _r2 in spec["arms"]]}
        TRAINER["borrowed_by"] = spec["name"]
    report = {"name": spec["name"], "budget_steps": spec["budget_steps"],
              "bricks_changed": spec.get("bricks_changed"),
              "one_brick": spec.get("one_brick"),
              "started": int(time.time() * 1000)}
    try:
        eval_names = set()
        for _a, recipe in spec["arms"]:
            for nm in str(recipe.get("evals", "")).split(","):
                if nm.strip():
                    eval_names.add(nm.strip())
            for k in parse_kv(recipe.get("mix", ""), {}):
                eval_names.add(k)
        eval_sets = {}
        pinned = {}
        for rec in load_registry_datasets(args.data_dir):
            nm = rec.get("name")
            if nm in eval_names:
                tr, ho = load_dataset_docs(rec)
                docs = ho or tr[:24]
                if docs:
                    eval_sets[nm] = docs[:24]
                    pinned[nm] = rec.get("hash")
        anchor_docs = load_anchor_docs(args)
        std = (anchor_docs[:24] if anchor_docs
               else load_standard_docs(args, limit=64)[:24])
        if std:
            eval_sets["standard"] = std
            pinned["standard"] = "anchor" if anchor_docs else "base_data"
        pairs = list(HELDOUT_PAIRS)
        report["pinned"] = pinned
        arms_out = {}
        for arm_name, recipe in spec["arms"]:
            arms_out[arm_name] = run_experiment_arm(
                args, recipe, spec["budget_steps"], eval_sets, pairs)
        report["arms"] = arms_out
        report["status"] = "done"
    except Exception as e:
        import traceback
        traceback.print_exc()
        report["status"] = "error"
        report["error"] = str(e)[:500]
    report["finished"] = int(time.time() * 1000)
    try:
        with open(os.path.join(args.data_dir, "experiments.jsonl"),
                  "a") as f:
            f.write(json.dumps(report) + "\n")
    except Exception:
        pass
    append_metric(args.data_dir, {"kind": "experiment",
                                  "name": spec["name"],
                                  "status": report["status"]})
    with LOCK:
        EXPERIMENT["running"] = False
        EXPERIMENT["current"] = None
        EXPERIMENT["last"] = report
        TRAINER["borrowed_by"] = None
    print(f"[bench] {spec['name']}: {report['status']}", flush=True)


def trainer_real(args):
    """Continuous CPT on a candidate copy of the live model. Runs only
    once a NanochatScorer is live; steps forever at --train-interval."""
    import random
    import copy as _copy
    while True:
        with LOCK:
            live = STATE["slots"][STATE["live"]]
        if live is not None and not isinstance(live, StubScorer):
            break
        time.sleep(5)
    # S5: the posture solver rules how (and whether) the candidate
    # trains - ruling 4's arithmetic, published (rule 4). full is the
    # shipped deepcopy path below; lora is the delta rung; frozen and
    # refused end here with the numbers on status.
    posture, arith = solve_posture(args, live)
    with LOCK:
        TRAINER["posture"] = (posture if posture != "refused"
                              else f"refused: {arith.get('why')}")
        TRAINER["arithmetic"] = arith
    print(f"[trainer] posture: {posture} {arith}", flush=True)
    if posture in ("refused", "frozen"):
        with LOCK:
            TRAINER["active"] = False
        return
    if posture == "lora":
        return trainer_lora_loop(args, live, arith)
    import torch
    mix = parse_kv(args.mix, {"fresh": 0.25, "replay": 0.25, "standard": 0.5})
    gate_cfg = parse_kv(args.gate, {"every": 50, "regress": 0.02, "fails": 3,
                                    "agree_slack": 0.05, "agree_n": 8})
    seq_len = live.meta["model_config"]["sequence_len"] if live.meta else 2048
    seq_len = min(int(seq_len), 2048)
    device = live.engine.model.get_device()
    candidate = _copy.deepcopy(live.engine.model).to(device)
    candidate.train()
    opt = torch.optim.AdamW(candidate.parameters(), lr=float(args.lr))
    replay_path = os.path.join(args.data_dir, "replay.jsonl")
    heldout_path = os.path.join(args.data_dir, "heldout.jsonl")
    pairs_path = os.path.join(args.data_dir, "heldout_pairs.jsonl")
    replay = []
    for path, dest in ((replay_path, replay), (heldout_path, HELDOUT_FRESH)):
        if os.path.exists(path):
            with open(path) as f:
                dest.extend(ln.strip() for ln in f if ln.strip())
    if os.path.exists(pairs_path) and not HELDOUT_PAIRS:
        # (the user gate loop may have loaded them already)
        with open(pairs_path) as f:
            for ln in f:
                ln = ln.strip()
                if ln:
                    try:
                        HELDOUT_PAIRS.append(json.loads(ln))
                    except Exception:
                        pass
    standard = load_standard_docs(args)
    # ruling 2: a registered model's anchor replaces the held-out
    # standard sample - the gate reads through the manager. An
    # unregistered or unanchored checkpoint takes the legacy split,
    # byte-for-byte (R1).
    anchor_docs = load_anchor_docs(args)
    if anchor_docs:
        heldout_std = anchor_docs[:24]
    else:
        heldout_std = standard[:24]
        standard = standard[24:]
    # S2: registered dataset pools join the built-ins, weighted by
    # MODEL_MIX entries naming them. Nothing configured = exactly the
    # three built-in pools - today's behavior. Mixing a dataset in or
    # out is editing one weight.
    ds_pools = {}
    ds_holdout = {}

    def refresh_ds_pools():
        for rec in load_registry_datasets(args.data_dir):
            nm = rec.get("name")
            if not nm or nm in ("fresh", "replay", "standard"):
                continue
            if mix.get(nm, 0) <= 0:
                continue
            tr, ho = load_dataset_docs(rec)
            if tr:
                ds_pools[nm] = tr
            if ho:
                ds_holdout[nm] = ho

    refresh_ds_pools()
    with LOCK:
        TRAINER["active"] = True
        TRAINER["mix"] = mix
        TRAINER["standard_docs"] = len(standard)
        TRAINER["replay_size"] = len(replay)
        TRAINER["dataset_pools"] = {k: len(v) for k, v in ds_pools.items()}
        TRAINER["anchor"] = bool(anchor_docs)
    print(f"[trainer] live: candidate of {live.name} on {device}, "
          f"mix={mix} gate={gate_cfg} lr={args.lr} seq_len={seq_len}", flush=True)

    def chunks_of(docs):
        toks = []
        for d in docs:
            toks.extend(live.tokenizer.encode(d[:4000], prepend="<|bos|>"))
        out = []
        for i in range(0, len(toks) - seq_len - 1, seq_len):
            out.append(toks[i:i + seq_len + 1])
        return out

    @torch.no_grad()
    def eval_loss(model, chs):
        if not chs:
            return None
        was_training = model.training
        model.eval()
        tot = 0.0
        for c in chs[:16]:
            x = torch.tensor([c[:-1]], device=device)
            y = torch.tensor([c[1:]], device=device)
            tot += float(model(x, y))
        if was_training:
            model.train()
        return tot / min(len(chs), 16)

    def gen_agreement(model, n):
        """Generate verdicts on held-out pairs with THE serving prompt;
        agreement = 1 - mean |generated - frontier target|. This is the
        gate measuring the actual job, not a perplexity proxy."""
        from nanochat.engine import Engine as _Engine
        pairs = HELDOUT_PAIRS[-int(n):]
        if not pairs:
            return None
        was_training = model.training
        model.eval()
        eng = _Engine(model, live.tokenizer)
        total = 0.0
        for pr in pairs:
            conversation = {"messages": [
                {"role": "user", "content": salience_prompt("?", pr["input"], 0, 0)},
                {"role": "assistant", "content": ""},
            ]}
            ids = live.tokenizer.render_for_completion(conversation)
            results, _m2 = eng.generate_batch(ids, num_samples=1, max_tokens=64,
                                              temperature=0.01, top_k=1)
            sal, _why, _parsed = parse_salience(live.tokenizer.decode(results[0][len(ids):]))
            if sal is None:
                sal = 0.5
            total += min(1.0, abs(sal - float(pr["target"])))
        if was_training:
            model.train()
        return round(1.0 - total / len(pairs), 4)

    heldout_std_chunks = chunks_of(heldout_std)
    live_std = eval_loss(live.engine.model, heldout_std_chunks)
    live_fresh = None
    live_agree = None
    fails = 0
    ema = None
    tick = 0
    while True:
        time.sleep(max(float(args.train_interval), 0.05))
        if EXPERIMENT["running"]:
            continue   # the bench borrows the time-share (ruling 7)
        # streams grow and datasets register mid-flight: refresh the
        # registered pools at start and every ~30 ticks (registry.json
        # itself is mtime-cached inside the loader)
        tick += 1
        if tick % 30 == 1:
            refresh_ds_pools()
            with LOCK:
                TRAINER["dataset_pools"] = {
                    k: len(v) for k, v in ds_pools.items()}
        # drain fresh curriculum; 1-in-10 goes to held-out, never trained
        with LOCK:
            fresh_in = list(TRAIN_Q)
            TRAIN_Q.clear()
        fresh_docs = []
        for entry in fresh_in:
            d = entry["doc"] if isinstance(entry, dict) else entry
            pair = entry.get("pair") if isinstance(entry, dict) else None
            if random.random() < 0.1 and len(HELDOUT_FRESH) < 512:
                with LOCK:
                    HELDOUT_FRESH.append(d)
                with open(heldout_path, "a") as f:
                    f.write(d.replace("\n", " ") + "\n")
                if pair:
                    HELDOUT_PAIRS.append(pair)
                    with open(pairs_path, "a") as f:
                        f.write(json.dumps(pair) + "\n")
                    live_agree = None  # held-out pairs changed; re-baseline
                live_fresh = None
            else:
                fresh_docs.append(d)
                replay.append(d)
                with open(replay_path, "a") as f:
                    f.write(d.replace("\n", " ") + "\n")
        if len(replay) > 4096:  # reservoir cap
            random.shuffle(replay)
            replay = replay[:4096]
            with open(replay_path, "w") as f:
                f.writelines(d + "\n" for d in replay)
        pools = dict(ds_pools)
        pools.update({"fresh": fresh_docs or replay, "replay": replay,
                      "standard": standard})
        avail = {k: p for k, p in pools.items() if p and mix.get(k, 0) > 0}
        if not avail:
            with LOCK:
                TRAINER["replay_size"] = len(replay)
            continue
        total_w = sum(mix[k] for k in avail)
        docs = []
        for k, pool in avail.items():
            n = max(1, round(6 * mix[k] / total_w))
            docs.extend(random.choice(pool) for _ in range(n))
        chs = chunks_of(docs)
        if not chs:
            continue
        random.shuffle(chs)
        try:
            batch = chs[:2]
            x = torch.tensor([c[:-1] for c in batch], device=device)
            y = torch.tensor([c[1:] for c in batch], device=device)
            loss = candidate(x, y)
            opt.zero_grad()
            loss.backward()
            opt.step()
        except torch.cuda.OutOfMemoryError:
            torch.cuda.empty_cache()
            print("[trainer] step OOM - skipped, cache cleared", flush=True)
            continue
        lv = float(loss.detach())
        ema = lv if ema is None else 0.98 * ema + 0.02 * lv
        with LOCK:
            TRAINER["steps"] += 1
            TRAINER["loss_ema"] = round(ema, 4)
            TRAINER["replay_size"] = len(replay)
            steps = TRAINER["steps"]
        if steps % 10 == 0:
            append_metric(args.data_dir, {"kind": "loss", "step": steps,
                                          "loss": round(ema, 4)})
        if steps % int(gate_cfg["every"]) != 0:
            continue
        # ── the gate: forgetting guard on held-out standard data, and -
        # when held-out pairs exist - GENERATED-verdict agreement with
        # the frontier's labels (the actual job), else the loss proxy.
        cand_std = eval_loss(candidate, heldout_std_chunks)
        std_ok = (cand_std is None or live_std is None
                  or cand_std <= live_std * (1 + gate_cfg["regress"]))
        cand_fresh = live_fresh2 = cand_agree = None
        if len(HELDOUT_PAIRS) >= 4:
            if live_agree is None:
                live_agree = gen_agreement(live.engine.model, gate_cfg["agree_n"])
            cand_agree = gen_agreement(candidate, gate_cfg["agree_n"])
            learn_ok = (cand_agree is None or live_agree is None
                        or cand_agree >= live_agree - gate_cfg["agree_slack"])
        else:
            heldout_fresh_chunks = chunks_of(HELDOUT_FRESH[-64:])
            if live_fresh is None:
                live_fresh = eval_loss(live.engine.model, heldout_fresh_chunks)
            live_fresh2 = live_fresh
            cand_fresh = eval_loss(candidate, heldout_fresh_chunks)
            learn_ok = (cand_fresh is None or live_fresh is None
                        or cand_fresh <= live_fresh)
        verdict = "promote" if (std_ok and learn_ok) else "hold"
        # per-dataset held-out losses: REPORTED, not gating - the
        # bench's yardstick (ruling 7) accumulating before the bench
        # exists. The standing gates keep their meaning unchanged.
        ds_losses = {}
        for nm, docs in ds_holdout.items():
            dl = eval_loss(candidate, chunks_of(docs[:24]))
            if dl is not None:
                ds_losses[nm] = dl
        gate_row = {
            "step": steps, "verdict": verdict,
            "cand_std": cand_std, "live_std": live_std,
            "cand_fresh": cand_fresh, "live_fresh": live_fresh2,
            "cand_agree": cand_agree, "live_agree": live_agree,
            "pairs": len(HELDOUT_PAIRS),
            "datasets": ds_losses or None,
        }
        print(f"[trainer] gate: {gate_row}", flush=True)
        with LOCK:
            TRAINER["gates"] += 1
            TRAINER["last_gate"] = gate_row
        append_metric(args.data_dir, dict(gate_row, kind="gate"))
        if verdict == "promote":
            saved_key = None
            try:
                from nanochat.checkpoint_manager import save_checkpoint
                ckdir = os.path.join(args.data_dir, "checkpoints",
                                     f"cpt-{time.strftime('%Y%m%d%H%M%S')}-{steps:06d}")
                save_checkpoint(ckdir, steps,
                                {k: v.detach().cpu() for k, v in candidate.state_dict().items()},
                                None, {"model_config": dict(live.meta["model_config"])})
                saved_key = os.path.basename(ckdir)
                # ring pruning by byte budget (ruling 5), floors held
                prune_ring(args)
            except Exception as e:
                print(f"[trainer] ring save failed (promoting anyway): {e}", flush=True)
            promoted_model = _copy.deepcopy(candidate)
            promoted_model.eval()
            scorer = live.from_parts(
                promoted_model, live.tokenizer, live.meta,
                f"nanochat:cpt-{steps}", live.checkpoint)
            with LOCK:
                inactive = "B" if STATE["live"] == "A" else "A"
                STATE["slots"][inactive] = scorer
                STATE["live"] = inactive
                STATE["promotions"] += 1
                TRAINER["promotions"] += 1
            live = scorer
            # the new salience pointer starts its soak on the SLOW lane's
            # clock; an unsaved (memory-only) promotion can never qualify
            set_soak(saved_key)
            live_std = eval_loss(live.engine.model, heldout_std_chunks)
            live_fresh = None
            live_agree = None
            fails = 0
            print(f"[trainer] PROMOTED -> {scorer.name}", flush=True)
        else:
            fails += 1
            with LOCK:
                TRAINER["fails"] = fails
            if fails >= int(gate_cfg["fails"]):
                candidate.load_state_dict(live.engine.model.state_dict())
                opt = torch.optim.AdamW(candidate.parameters(), lr=float(args.lr))
                fails = 0
                with LOCK:
                    TRAINER["resets"] += 1
                    TRAINER["fails"] = 0
                print("[trainer] consecutive gate failures - candidate "
                      "reset to live weights", flush=True)


def trainer_loop(args):
    """Ingest drain: parse curriculum batches, render docs, feed the real
    trainer's queue (stub mode keeps the 5b skeleton markers)."""
    ingest = os.path.join(args.data_dir, "ingest")
    done = os.path.join(args.data_dir, "ingested")
    ckdir = os.path.join(args.data_dir, "checkpoints")
    while True:
        try:
            for name in sorted(os.listdir(ingest)):
                if not name.endswith(".jsonl"):
                    continue
                path = os.path.join(ingest, name)
                n = 0
                kinds = {}
                with open(path) as f:
                    for line in f:
                        line = line.strip()
                        if not line:
                            continue
                        n += 1
                        try:
                            k = json.loads(line).get("kind", "?")
                        except Exception:
                            k = "unparseable"
                        kinds[k] = kinds.get(k, 0) + 1
                shutil.move(path, os.path.join(done, name))
                docs = []
                with open(os.path.join(done, name)) as f:
                    for line in f:
                        line = line.strip()
                        if not line:
                            continue
                        try:
                            o = json.loads(line)
                            pair = None
                            if o.get("kind") == "salience_pair":
                                r2 = o.get("row") or {}
                                tgt = r2.get("frontier", r2.get("local"))
                                if r2.get("input") and tgt is not None:
                                    pair = {"input": str(r2["input"])[:600],
                                            "target": float(tgt)}
                            docs.append({"doc": render_sample(o), "pair": pair})
                        except Exception:
                            pass
                with LOCK:
                    TRAIN_Q.extend(docs)
                    TRAINER["fresh_pending"] = len(TRAIN_Q)
                    STATE["ingested_files"] += 1
                    STATE["ingested_samples"] += n
                    trainer_on = TRAINER["active"]
                print(f"[trainer] {name}: {n} samples {kinds} -> "
                      f"{'training queue' if trainer_on else 'queued (trainer idle)'}",
                      flush=True)
                if not trainer_on:
                    # stub mode keeps the 5b skeleton ring markers
                    stamp = time.strftime("%Y%m%d-%H%M%S")
                    mark = os.path.join(ckdir, f"checkpoint-{stamp}")
                    os.makedirs(mark, exist_ok=True)
                    with open(os.path.join(mark, "MARKER"), "w") as f:
                        f.write(f"skeleton checkpoint after {name} ({n} samples)\n")
                    with LOCK:
                        protected = {v for v in (USER["pointer"], USER["last_good"],
                                                 USER["ready"]) if v}
                    ring = sorted(os.listdir(ckdir))
                    for old in ring[:-5]:
                        if old in protected:
                            continue
                        shutil.rmtree(os.path.join(ckdir, old), ignore_errors=True)
        except Exception as e:
            print(f"[trainer] error: {e}", flush=True)
        time.sleep(3)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *a):
        pass  # own logging only

    def _json(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path != "/status":
            return self._json(404, {"status": "err", "msg": "unknown path"})
        # computed OUTSIDE the lock: probe_resources may shell out to
        # nvidia-smi, and /salience scoring shares this lock
        res_map = probe_resources(self.server.args.data_dir)
        reg_info = registry_info(self.server.args.data_dir)
        with LOCK:
            live = STATE["live"]
            scorer = STATE["slots"][live]
            if scorer is not None:
                mode = scorer.name
            elif STATE["loading"]:
                mode = "loading"
            else:
                mode = "waiting"
            ck = os.path.join(self.server.args.data_dir, "checkpoints")
            # stale_script: the file this process was started from has
            # been rewritten since (bootstrap ships a newer asset) - the
            # running code predates it. Bootstrap reads this and
            # converges: kill by the pid reported here, relaunch.
            try:
                stale = os.path.getmtime(os.path.abspath(__file__)) > START + 1
            except OSError:
                stale = False
            self._json(200, {
                "status": "ok",
                "mode": mode,
                "pid": os.getpid(),
                "stale_script": stale,
                "boot_error": STATE["boot_error"],
                "live_slot": live,
                "checkpoint": getattr(scorer, "checkpoint", self.server.args.checkpoint),
                "scored": STATE["scored"],
                "promotions": STATE["promotions"],
                "ingested_files": STATE["ingested_files"],
                "ingested_samples": STATE["ingested_samples"],
                "ingest_pending": len([
                    n for n in os.listdir(
                        os.path.join(self.server.args.data_dir, "ingest"))
                    if n.endswith(".jsonl")]),
                "checkpoints": sorted(os.listdir(ck)) if os.path.isdir(ck) else [],
                "uptime_s": int(time.time() - START),
                "resources": res_map,
                "registry": reg_info,
                "mint": MINT["last"],
                "experiment": {"running": EXPERIMENT["running"],
                               "current": EXPERIMENT["current"],
                               "last": EXPERIMENT["last"]},
                "trainer": dict(TRAINER),
                "user": dict(
                    USER,
                    soaking=SOAK["current"],
                    soak=(lambda s: {"since_s": int(time.time() - s["since"]),
                                     "verdicts": s["verdicts"]} if s else None)(
                        SOAK["rings"].get(SOAK["current"])),
                    serving=USER_SLOT["scorer"] is not None,
                ),
                "persona": dict(PERSONA),
                "adapters": dict(ADAPTERS),
            })

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length).decode() if length else ""
        if self.path == "/salience":
            try:
                req = json.loads(raw)
            except Exception:
                return self._json(400, {"status": "err", "msg": "body must be JSON"})
            t0 = time.time()
            with LOCK:
                live = STATE["live"]
                scorer = STATE["slots"][live]
            if scorer is None:
                msg = STATE["boot_error"] or "scorer still loading"
                return self._json(503, {"status": "err", "msg": msg})
            try:
                res = scorer.score(
                    req.get("perception") or {}, req.get("context") or {})
                sal, why, parsed = res if len(res) == 3 else (res[0], res[1], True)
            except Exception as e:
                return self._json(500, {"status": "err", "msg": f"scorer failed: {e}"})
            with LOCK:
                STATE["scored"] += 1
                cur = SOAK["current"]
                if cur and cur in SOAK["rings"]:
                    SOAK["rings"][cur]["verdicts"] += 1
            append_metric(self.server.args.data_dir,
                          {"kind": "verdict", "sal": round(float(sal), 3),
                           "parsed": bool(parsed)})
            return self._json(200, {
                "status": "ok",
                "salient": round(float(sal), 4),
                "parsed": bool(parsed),
                "reasoning": why,
                "pointer": f"{live}:{scorer.name}",
                "ms": int((time.time() - t0) * 1000),
            })
        if self.path == "/ingest":
            ingest = os.path.join(self.server.args.data_dir, "ingest")
            name = f"batch-{time.strftime('%Y%m%d-%H%M%S')}-{os.getpid()}.jsonl"
            with open(os.path.join(ingest, name), "w") as f:
                f.write(raw)
            return self._json(200, {"status": "ok", "queued": name})
        if self.path == "/derive":
            # S2: procedural transforms run HERE so the dialect keeps
            # one home - render_sample IS the render_dialect transform.
            # Synchronous: rendering is line-at-a-time stdlib work, and
            # it runs in stub mode too.
            try:
                req = json.loads(raw)
            except Exception:
                return self._json(400, {"status": "err",
                                        "msg": "body must be JSON"})
            spath = str(req.get("source_path") or "")
            out_name = str(req.get("name") or "")
            transform = str(req.get("transform") or "")
            if transform != "render_dialect":
                return self._json(400, {
                    "status": "err",
                    "msg": f"unknown transform '{transform}' - this "
                           "branch ships render_dialect only"})
            files = []
            if os.path.isfile(spath):
                files = [spath]
            elif os.path.isdir(spath):
                for base, _d, fs in os.walk(spath):
                    files.extend(os.path.join(base, n)
                                 for n in sorted(fs)
                                 if n.endswith(".jsonl"))
            if not files or not out_name:
                return self._json(400, {"status": "err",
                                        "msg": "need name and a jsonl "
                                        f"source (got '{spath}')"})
            outdir = os.path.join(self.server.args.data_dir,
                                  "datasets", out_name)
            os.makedirs(outdir, exist_ok=True)
            rows = 0
            outpath = os.path.join(outdir, "docs.txt")
            with open(outpath, "w") as f:
                for path in files:
                    with open(path) as sf:
                        for ln in sf:
                            ln = ln.strip()
                            if not ln:
                                continue
                            try:
                                doc = render_sample(json.loads(ln))
                            except Exception:
                                doc = ln
                            f.write(doc.replace("\n", " ") + "\n")
                            rows += 1
            return self._json(200, {"status": "ok", "rows": rows,
                                    "path": outpath})
        if self.path == "/experiment":
            # S6: run a bench experiment - recipes arrive RESOLVED (the
            # commands own the store; the service stays store-blind).
            try:
                req = json.loads(raw)
            except Exception:
                return self._json(400, {"status": "err",
                                        "msg": "body must be JSON"})
            if self.server.args.checkpoint == "stub":
                return self._json(409, {"status": "err",
                                        "msg": "stub mode has no weights "
                                        "to bench"})
            with LOCK:
                if EXPERIMENT["running"]:
                    return self._json(409, {
                        "status": "err",
                        "msg": "an experiment is already running",
                        "current": EXPERIMENT["current"]})
            name = str(req.get("name") or "")
            control = req.get("control")
            if not name or not isinstance(control, dict):
                return self._json(400, {"status": "err",
                                        "msg": "need name and a resolved "
                                        "control recipe"})
            arms = [("control", control)]
            if isinstance(req.get("variant"), dict):
                arms.append(("variant", req["variant"]))
            spec = {"name": name, "arms": arms,
                    "budget_steps": int(req.get("budget_steps") or 20),
                    "bricks_changed": req.get("bricks_changed"),
                    "one_brick": req.get("one_brick")}
            threading.Thread(target=run_experiment_thread,
                             args=(self.server.args, spec),
                             daemon=True).start()
            return self._json(200, {"status": "ok", "started": True,
                                    "name": name,
                                    "arms": [a for a, _r3 in arms]})
        if self.path == "/mint_anchor":
            # ruling 2: mint a self-sampled anchor for an import. Loads
            # its own scorer (never the serving slots); background
            # thread; progress rides /status.mint. Stub mode has no
            # torch to load with - refuse honestly.
            try:
                req = json.loads(raw)
            except Exception:
                return self._json(400, {"status": "err",
                                        "msg": "body must be JSON"})
            if self.server.args.checkpoint == "stub":
                return self._json(409, {"status": "err",
                                        "msg": "stub mode cannot mint - no "
                                        "model environment; configure a real "
                                        "checkpoint first"})
            path = str(req.get("path") or "")
            out_name = str(req.get("name") or "")
            n = int(req.get("n") or 200)
            if not path or not os.path.isdir(path) or not out_name:
                return self._json(400, {"status": "err",
                                        "msg": "need path (existing dir) "
                                        "and name"})
            with LOCK:
                if MINT["running"]:
                    return self._json(409, {"status": "err",
                                            "msg": "a mint is already "
                                            "running", "mint": MINT["last"]})
            mbackend = str(req.get("backend") or "nanochat")
            threading.Thread(target=mint_anchor_thread,
                             args=(self.server.args, path, out_name, n,
                                   mbackend),
                             daemon=True).start()
            return self._json(200, {"status": "ok", "started": True,
                                    "name": out_name, "n": n})
        if self.path == "/shutdown":
            # The GPU off-switch: answer, then exit cleanly. Everything
            # durable survives on disk (ring checkpoints, replay,
            # held-out sets, metrics, the user pointer); the in-memory
            # candidate's steps since its last promotion are the only
            # loss. Bootstrap relaunches on demand and resumes at the
            # newest ring checkpoint via make_scorer.
            self._json(200, {"status": "ok", "msg": "shutting down"})
            threading.Timer(0.3, lambda: os._exit(0)).start()
            return
        if self.path == "/promote":
            # double-buffer: load into the slot NOT being served, then swap.
            args2 = self.server.args
            if getattr(args2, "backend", "nanochat") == "hf":
                return self._json(409, {
                    "status": "err",
                    "msg": "the hf backend has no ring checkpoints to "
                           "promote yet - the delta trainer lands in S5"})
            newest = (newest_ring(args2.data_dir)
                      if args2.checkpoint != "stub" else newest_checkpoint(args2))
            try:
                scorer = (
                    StubScorer(tag=os.path.basename(newest) if newest else "stub")
                    if (newest is None or args2.checkpoint == "stub")
                    else NanochatScorer.from_ring(args2.checkpoint, newest)
                )
            except Exception as e:
                return self._json(500, {"status": "err", "msg": f"load failed: {e}"})
            with LOCK:
                inactive = "B" if STATE["live"] == "A" else "A"
                STATE["slots"][inactive] = scorer
                STATE["live"] = inactive
                STATE["promotions"] += 1
                live = STATE["live"]
            set_soak(soak_key_for(scorer))
            return self._json(200, {
                "status": "ok", "live_slot": live,
                "loaded": newest or "stub", "pointer": f"{live}:{scorer.name}",
            })
        if self.path == "/chat":
            # the USER pointer's face - never the salience slots
            try:
                req = json.loads(raw) if raw else {}
            except Exception:
                return self._json(400, {"status": "err", "msg": "body must be JSON"})
            with LOCK:
                scorer = USER_SLOT["scorer"]
                pointer = USER["name"]
            if scorer is None:
                return self._json(503, {
                    "status": "err",
                    "msg": "no user pointer promoted yet - the user gate "
                           "(soak + evals) has not advanced it"})
            t0 = time.time()
            messages = req.get("messages") or []
            if not messages and req.get("prompt"):
                messages = [{"role": "user", "content": str(req["prompt"])}]
            if not messages:
                return self._json(400, {"status": "err",
                                        "msg": "messages (or prompt) required"})
            if isinstance(scorer, StubScorer):
                last = str(messages[-1].get("content", ""))[:120]
                return self._json(200, {
                    "status": "ok",
                    "text": f"[stub user pointer] heard: {last}",
                    "pointer": pointer, "ms": int((time.time() - t0) * 1000)})
            # nanochat's chat template knows user/assistant only - fold
            # any system content into the first user turn there; hf
            # templates carry system natively, pass it through (S3)
            if isinstance(scorer, NanochatScorer):
                sys_txt = "\n".join(str(m.get("content", ""))
                                    for m in messages
                                    if m.get("role") == "system").strip()
                messages = [m for m in messages
                            if m.get("role") != "system"]
                if sys_txt and messages:
                    m0 = dict(messages[0])
                    m0["content"] = (f"[system]\n{sys_txt}\n\n"
                                     f"{m0.get('content', '')}")
                    messages = [m0] + messages[1:]
            try:
                max_tokens = min(int(req.get("max_tokens") or 256), 1024)
                temp = float(req.get("temperature") or 0.7)
                text = scorer.generate_text(list(messages),
                                            max_tokens=max_tokens,
                                            temperature=temp, top_k=50)
            except Exception as e:
                return self._json(500, {"status": "err",
                                        "msg": f"generation failed: {e}"})
            return self._json(200, {"status": "ok", "text": text,
                                    "pointer": pointer,
                                    "ms": int((time.time() - t0) * 1000)})
        if self.path == "/user_promote":
            ok, payload = do_user_promote(self.server.args, "manual")
            return self._json(200 if ok else 409, payload)
        if self.path == "/user_rollback":
            ok, payload = do_user_rollback(self.server.args, "manual")
            return self._json(200 if ok else 409, payload)
        if self.path == "/persona_rederive":
            # manual re-derivation - blocks through the whole training
            # run; the mind tab's button expects the full report back
            payload = derive_adapter(self.server.args, "manual")
            return self._json(200 if payload.get("status") == "ok" else 409, payload)
        if self.path == "/adapter_derive":
            # S4: a named adapter from a registered dataset - blocks
            # through the run like persona_rederive; the command
            # records the result in the runtime library
            try:
                req = json.loads(raw)
            except Exception:
                return self._json(400, {"status": "err",
                                        "msg": "body must be JSON"})
            spec = {"name": str(req.get("name") or ""),
                    "dataset": str(req.get("dataset") or ""),
                    "base": str(req.get("base") or "pointer"),
                    "targets": str(req.get("targets") or ""),
                    "rank": int(req.get("rank") or 0),
                    "steps": int(req.get("steps") or 0)}
            if not spec["name"] or not spec["dataset"]:
                return self._json(400, {"status": "err",
                                        "msg": "need name and dataset"})
            try:
                payload = derive_named_adapter(self.server.args, spec)
            except Exception as e:
                payload = {"status": "err", "msg": f"derive failed: {e}"}
            return self._json(200 if payload.get("status") == "ok"
                              else 409, payload)
        if self.path in ("/adapter_apply", "/adapter_unapply"):
            try:
                req = json.loads(raw)
            except Exception:
                return self._json(400, {"status": "err",
                                        "msg": "body must be JSON"})
            name = str(req.get("name") or "")
            if not name:
                return self._json(400, {"status": "err",
                                        "msg": "need name"})
            with LOCK:
                stack = list(ADAPTERS["stack"])
            if self.path == "/adapter_apply":
                if name in stack:
                    return self._json(409, {"status": "err",
                                            "msg": f"'{name}' is already "
                                            "in the stack"})
                new_stack = stack + [name]
            else:
                if name not in stack:
                    return self._json(409, {"status": "err",
                                            "msg": f"'{name}' is not in "
                                            "the stack"})
                new_stack = [n for n in stack if n != name]
            try:
                payload = apply_stack_gated(self.server.args, new_stack,
                                            self.path.lstrip("/"))
            except Exception as e:
                payload = {"status": "err", "msg": f"stack change "
                                                   f"failed: {e}"}
            return self._json(200 if payload.get("status") == "ok"
                              else 409, payload)
        return self._json(404, {"status": "err", "msg": "unknown path"})


def load_initial(args):
    """Load the scorer AFTER the port binds - a real checkpoint takes
    long enough (torch import + weights) that loading before serving
    made every honest launch look dead to the outside. And a load
    failure is NOT fatal: when bootstrap has kicked off base training,
    the checkpoint this service is waiting for may be hours away - so
    retry every 60s forever, and the service upgrades itself the moment
    weights appear. /status says `loading`, then `waiting` (with
    boot_error naming what was missing) between retries; /salience
    answers 503 throughout, which the executive treats as no-verdict."""
    while True:
        try:
            scorer = make_scorer(args.checkpoint, args.data_dir,
                                 getattr(args, "backend", "nanochat"))
            with LOCK:
                STATE["slots"]["A"] = scorer
                STATE["loading"] = False
                STATE["boot_error"] = None
            set_soak(soak_key_for(scorer))
            print(f"[service] scorer ready: {scorer.name}", flush=True)
            return
        except Exception as e:
            import traceback
            with LOCK:
                STATE["loading"] = False
                STATE["boot_error"] = f"{type(e).__name__}: {e}"
            traceback.print_exc()
            print("[service] scorer load failed; retrying in 60s "
                  "(training may still be producing the checkpoint)", flush=True)
            time.sleep(60)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=8077)
    ap.add_argument("--data-dir", default="runtime/agent/model")
    ap.add_argument("--checkpoint", default="stub",
                    help="'stub', a nanochat base directory, or (with "
                         "--backend hf) an HF model directory")
    ap.add_argument("--backend", default="nanochat",
                    choices=["nanochat", "hf"],
                    help="which backend serves the checkpoint (S3); "
                         "resolved from the model record by bootstrap")
    ap.add_argument("--posture", default="auto",
                    help="S5, ruling 4: auto (the solver decides) or a "
                         "forced rung (full|lora|frozen) - published, "
                         "and refused with the arithmetic when it "
                         "does not fit")
    ap.add_argument("--ring-gb", default="100",
                    help="S5, ruling 5: the ring's byte budget in GB; "
                         "the protected user set and the newest entry "
                         "never prune")
    ap.add_argument("--headroom", default="15",
                    help="S5, ruling 4: the solver's safety margin as "
                         "a percent of total memory")
    ap.add_argument("--train", default="on", choices=["on", "off"],
                    help="continuous CPT on a candidate of the live model")
    ap.add_argument("--mix", default="fresh=0.25,replay=0.25,standard=0.5",
                    help="batch mix ratios (owner call: the replay ratio)")
    ap.add_argument("--lr", default="2e-5")
    ap.add_argument("--gate", default="every=50,regress=0.02,fails=3",
                    help="gate cadence + thresholds (owner call)")
    ap.add_argument("--train-interval", default="10",
                    help="seconds between training steps")
    ap.add_argument("--user-gate",
                    default="mode=manual,soak_s=21600,verdicts=100,"
                            "agree=0.75,regress=0.05,check_s=300",
                    help="the user pointer's stricter gate (owner call): "
                         "mode=manual|auto, soak_s/verdicts on the fast "
                         "lane, agree floor, regress slack, check cadence")
    ap.add_argument("--lora",
                    default="mode=on,rank=8,alpha=16,lr=1e-3,steps=200,"
                            "slack=0.1,min_gain=0.01,guard=0.2,targets=c_q.c_v",
                    help="the personality adapter (8b, owner call): "
                         "mode=on|off, LoRA rank/alpha/lr/steps, probe "
                         "slack that triggers re-derivation, min held-out "
                         "gain + standard-loss guard for the derivation "
                         "gate, dot-separated targets")
    args = ap.parse_args()
    for sub in ("checkpoints", "ingest", "ingested"):
        os.makedirs(os.path.join(args.data_dir, sub), exist_ok=True)
    threading.Thread(target=load_initial, args=(args,), daemon=True).start()
    threading.Thread(target=trainer_loop, args=(args,), daemon=True).start()
    if args.train == "on" and args.checkpoint != "stub":
        threading.Thread(target=trainer_real, args=(args,), daemon=True).start()
    threading.Thread(target=restore_user_pointer, args=(args,), daemon=True).start()
    threading.Thread(target=user_gate_loop, args=(args,), daemon=True).start()
    srv = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    srv.args = args
    print(f"[service] serving on 127.0.0.1:{args.port} (scorer loading) "
          f"data={args.data_dir}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
