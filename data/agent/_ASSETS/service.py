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
    POST /salience  {perception, context} -> {salient, reasoning, pointer, ms}
    GET  /status    -> mode, live slot, counters, ingest/checkpoint state
    POST /ingest    (JSONL body) -> queued into the ingest directory
    POST /promote   load newest checkpoint into the INACTIVE slot, swap live

The live pointer is double-buffered from day one: /promote loads into the
slot not being served and swaps under a lock, so serving never waits on a
load.

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
    """{salient, reasoning} out of a model reply, salvaging a bare
    number; (None, why) when nothing parseable is there."""
    s0, e0 = completion.find("{"), completion.rfind("}")
    if 0 <= s0 < e0:
        try:
            d = json.loads(completion[s0:e0 + 1])
            sal = float(d.get("salient"))
            why = str(d.get("reasoning") or "")[:300]
            return max(0.0, min(1.0, sal)), (why or "model gave no reasoning")
        except Exception:
            pass
    m = re.search(r"(?<![\w.])(?:0?\.\d+|[01](?:\.\d+)?)(?![\w.])", completion)
    if m:
        return (max(0.0, min(1.0, float(m.group(0)))),
                f"unparseable reply, salvaged number: {completion[:120]!r}")
    return None, f"unparseable reply: {completion[:120]!r}"


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

    def score(self, perception, context):
        payload = perception.get("payload") or {}
        text = " ".join(str(v) for v in payload.values() if isinstance(v, str))[:600]
        prompt = salience_prompt(perception.get("kind", "?"), text,
                                 context.get("matched") or 0, context.get("bound") or 0)
        conversation = {"messages": [
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": ""},
        ]}
        ids = self.tokenizer.render_for_completion(conversation)
        results, _masks = self.engine.generate_batch(
            ids, num_samples=1, max_tokens=96, temperature=0.2, top_k=50)
        completion = self.tokenizer.decode(results[0][len(ids):])
        sal, why = parse_salience(completion)
        if sal is None:
            return 0.5, "defaulting uncertain - " + why
        return sal, why


def make_scorer(checkpoint):
    if checkpoint == "stub":
        return StubScorer()
    return NanochatScorer(checkpoint)


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
            r = o.get("row") or {}
            target = r.get("frontier", r.get("local", ""))
            why = r.get("frontier_why", "")
            return (f"PERCEPTION: {r.get('input', '')}\n"
                    f"SALIENCE VERDICT: {target}\nREASONING: {why}")
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


def trainer_real(args):
    """Continuous CPT on a candidate copy of the live model. Runs only
    once a NanochatScorer is live; steps forever at --train-interval."""
    import random
    import copy as _copy
    while True:
        with LOCK:
            live = STATE["slots"][STATE["live"]]
        if live is not None and live.name.startswith("nanochat"):
            break
        time.sleep(5)
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
    if os.path.exists(pairs_path):
        with open(pairs_path) as f:
            for ln in f:
                ln = ln.strip()
                if ln:
                    try:
                        HELDOUT_PAIRS.append(json.loads(ln))
                    except Exception:
                        pass
    standard = load_standard_docs(args)
    heldout_std = standard[:24]
    standard = standard[24:]
    with LOCK:
        TRAINER["active"] = True
        TRAINER["mix"] = mix
        TRAINER["standard_docs"] = len(standard)
        TRAINER["replay_size"] = len(replay)
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
            sal, _why = parse_salience(live.tokenizer.decode(results[0][len(ids):]))
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
    while True:
        time.sleep(max(float(args.train_interval), 0.05))
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
        pools = {"fresh": fresh_docs or replay, "replay": replay, "standard": standard}
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
        gate_row = {
            "step": steps, "verdict": verdict,
            "cand_std": cand_std, "live_std": live_std,
            "cand_fresh": cand_fresh, "live_fresh": live_fresh2,
            "cand_agree": cand_agree, "live_agree": live_agree,
            "pairs": len(HELDOUT_PAIRS),
        }
        print(f"[trainer] gate: {gate_row}", flush=True)
        with LOCK:
            TRAINER["gates"] += 1
            TRAINER["last_gate"] = gate_row
        append_metric(args.data_dir, dict(gate_row, kind="gate"))
        if verdict == "promote":
            try:
                from nanochat.checkpoint_manager import save_checkpoint
                ckdir = os.path.join(args.data_dir, "checkpoints", f"cpt-{steps:06d}")
                save_checkpoint(ckdir, steps,
                                {k: v.detach().cpu() for k, v in candidate.state_dict().items()},
                                None, {"model_config": dict(live.meta["model_config"])})
                ring = sorted(os.listdir(os.path.join(args.data_dir, "checkpoints")))
                for old in ring[:-5]:
                    shutil.rmtree(os.path.join(args.data_dir, "checkpoints", old),
                                  ignore_errors=True)
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
                    ring = sorted(os.listdir(ckdir))
                    for old in ring[:-5]:
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
                "trainer": dict(TRAINER),
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
                sal, why = scorer.score(
                    req.get("perception") or {}, req.get("context") or {})
            except Exception as e:
                return self._json(500, {"status": "err", "msg": f"scorer failed: {e}"})
            with LOCK:
                STATE["scored"] += 1
            append_metric(self.server.args.data_dir,
                          {"kind": "verdict", "sal": round(float(sal), 3)})
            return self._json(200, {
                "status": "ok",
                "salient": round(float(sal), 4),
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
        if self.path == "/promote":
            # double-buffer: load into the slot NOT being served, then swap.
            newest = newest_checkpoint(self.server.args)
            try:
                scorer = (
                    StubScorer(tag=os.path.basename(newest))
                    if (newest is None or self.server.args.checkpoint == "stub")
                    else NanochatScorer(newest)
                )
            except Exception as e:
                return self._json(500, {"status": "err", "msg": f"load failed: {e}"})
            with LOCK:
                inactive = "B" if STATE["live"] == "A" else "A"
                STATE["slots"][inactive] = scorer
                STATE["live"] = inactive
                STATE["promotions"] += 1
                live = STATE["live"]
            return self._json(200, {
                "status": "ok", "live_slot": live,
                "loaded": newest or "stub", "pointer": f"{live}:{scorer.name}",
            })
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
            scorer = make_scorer(args.checkpoint)
            with LOCK:
                STATE["slots"]["A"] = scorer
                STATE["loading"] = False
                STATE["boot_error"] = None
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
                    help="'stub' or a nanochat base directory")
    ap.add_argument("--train", default="on", choices=["on", "off"],
                    help="continuous CPT on a candidate of the live model")
    ap.add_argument("--mix", default="fresh=0.25,replay=0.25,standard=0.5",
                    help="batch mix ratios (owner call: the replay ratio)")
    ap.add_argument("--lr", default="2e-5")
    ap.add_argument("--gate", default="every=50,regress=0.02,fails=3",
                    help="gate cadence + thresholds (owner call)")
    ap.add_argument("--train-interval", default="10",
                    help="seconds between training steps")
    args = ap.parse_args()
    for sub in ("checkpoints", "ingest", "ingested"):
        os.makedirs(os.path.join(args.data_dir, sub), exist_ok=True)
    threading.Thread(target=load_initial, args=(args,), daemon=True).start()
    threading.Thread(target=trainer_loop, args=(args,), daemon=True).start()
    if args.train == "on" and args.checkpoint != "stub":
        threading.Thread(target=trainer_real, args=(args,), daemon=True).start()
    srv = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    srv.args = args
    print(f"[service] serving on 127.0.0.1:{args.port} (scorer loading) "
          f"data={args.data_dir}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
