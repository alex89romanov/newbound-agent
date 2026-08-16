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
load. The trainer runs as a SKELETON in 5b: it drains the ingest
directory, logs what it would step on, and rotates the checkpoint ring -
actual replay-buffered CPT stepping (and the owner's replay-ratio and
gate-threshold calls) arrive with Phase 6.
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
STATE = {
    "slots": {"A": None, "B": None},
    "live": "A",
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
        self.name = f"nanochat:{source}"

    def score(self, perception, context):
        payload = perception.get("payload") or {}
        text = " ".join(str(v) for v in payload.values() if isinstance(v, str))[:600]
        prompt = (
            "You judge salience for an autonomous agent's perception stream.\n"
            f"PERCEPTION kind={perception.get('kind', '?')}: {text}\n"
            f"CONTEXT: {int(context.get('matched') or 0)} recalled and "
            f"{int(context.get('bound') or 0)} bound memory claims.\n"
            "How much does this perception matter to the agent's "
            "understanding of its environment, from 0.0 (noise) to 1.0 "
            "(critical)? Reply with ONLY a JSON object: "
            '{"salient": <0.0-1.0>, "reasoning": "<one sentence>"}')
        conversation = {"messages": [
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": ""},
        ]}
        ids = self.tokenizer.render_for_completion(conversation)
        results, _masks = self.engine.generate_batch(
            ids, num_samples=1, max_tokens=96, temperature=0.2, top_k=50)
        completion = self.tokenizer.decode(results[0][len(ids):])
        # Parse {salient, reasoning}; fall back to the first number found.
        s0, e0 = completion.find("{"), completion.rfind("}")
        if 0 <= s0 < e0:
            try:
                d = json.loads(completion[s0:e0 + 1])
                sal = float(d.get("salient"))
                why = str(d.get("reasoning") or "")[:300]
                return max(0.0, min(1.0, sal)), why or "model gave no reasoning"
            except Exception:
                pass
        m = re.search(r"(?<![\w.])(?:0?\.\d+|[01](?:\.\d+)?)(?![\w.])", completion)
        if m:
            return max(0.0, min(1.0, float(m.group(0)))), \
                f"unparseable reply, salvaged number: {completion[:120]!r}"
        return 0.5, f"unparseable reply, defaulting uncertain: {completion[:120]!r}"


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


def trainer_loop(args):
    """The trainer skeleton: drain ingest, log, rotate the ring."""
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
                print(
                    f"[trainer] {name}: would step on {n} samples {kinds} "
                    "(replay-buffered mixing arrives in Phase 6)",
                    flush=True,
                )
                shutil.move(path, os.path.join(done, name))
                with LOCK:
                    STATE["ingested_files"] += 1
                    STATE["ingested_samples"] += n
                # ring: a stub checkpoint marker per drained batch, last 5 kept
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
            ck = os.path.join(self.server.args.data_dir, "checkpoints")
            self._json(200, {
                "status": "ok",
                "mode": scorer.name if scorer else "empty",
                "live_slot": live,
                "checkpoint": getattr(scorer, "checkpoint", "stub"),
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
                return self._json(503, {"status": "err", "msg": "no scorer loaded"})
            try:
                sal, why = scorer.score(
                    req.get("perception") or {}, req.get("context") or {})
            except Exception as e:
                return self._json(500, {"status": "err", "msg": f"scorer failed: {e}"})
            with LOCK:
                STATE["scored"] += 1
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


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=8077)
    ap.add_argument("--data-dir", default="runtime/agent/model")
    ap.add_argument("--checkpoint", default="stub",
                    help="'stub' or a nanochat checkpoint path")
    args = ap.parse_args()
    for sub in ("checkpoints", "ingest", "ingested"):
        os.makedirs(os.path.join(args.data_dir, sub), exist_ok=True)
    STATE["slots"]["A"] = make_scorer(args.checkpoint)
    threading.Thread(target=trainer_loop, args=(args,), daemon=True).start()
    srv = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    srv.args = args
    print(f"[service] serving on 127.0.0.1:{args.port} "
          f"mode={STATE['slots']['A'].name} data={args.data_dir}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
