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


def newest_ring(data_dir):
    """Newest gate-promoted checkpoint dir (cpt-*, must hold weights)."""
    import glob as _glob
    dirs = sorted(d for d in _glob.glob(os.path.join(data_dir, "checkpoints", "cpt-*"))
                  if _glob.glob(os.path.join(d, "model_*.pt")))
    return dirs[-1] if dirs else None


def make_scorer(checkpoint, data_dir):
    if checkpoint == "stub":
        return StubScorer()
    ring = newest_ring(data_dir)
    if ring:
        try:
            return NanochatScorer.from_ring(checkpoint, ring)
        except Exception as e:
            print(f"[service] ring load failed ({e}); falling back to base dir", flush=True)
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
        return NanochatScorer(args.checkpoint)
    ring_dir = os.path.join(args.data_dir, "checkpoints", key)
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
        conversation = {"messages": [
            {"role": "user", "content": salience_prompt("?", pr["input"], 0, 0)},
            {"role": "assistant", "content": ""},
        ]}
        ids = scorer.tokenizer.render_for_completion(conversation)
        results, _m = scorer.engine.generate_batch(
            ids, num_samples=1, max_tokens=64, temperature=0.01, top_k=1)
        sal, _why = parse_salience(scorer.tokenizer.decode(results[0][len(ids):]))
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
        scorer = apply_persona(args, load_pointer_scorer(args, key))
    except Exception as e:
        return False, {"status": "err", "msg": f"user pointer load failed: {e}"}
    with LOCK:
        prev = USER["pointer"]
        USER_SLOT["scorer"] = scorer
        USER["last_good"] = prev
        USER["pointer"] = key
        USER["name"] = ("user:" + key
                        + ("+lora" if getattr(scorer, "name", "").endswith("+lora") else ""))
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
        scorer = apply_persona(args, load_pointer_scorer(args, target))
    except Exception as e:
        return False, {"status": "err", "msg": f"rollback load failed: {e}"}
    with LOCK:
        USER_SLOT["scorer"] = scorer
        USER["pointer"] = target
        USER["name"] = ("user:" + target
                        + ("+lora" if getattr(scorer, "name", "").endswith("+lora") else ""))
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
                        probe = persona_eval(user_scorer.engine.model,
                                             user_scorer.tokenizer, _ho,
                                             user_scorer.engine.model.get_device(),
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
    scorer = apply_persona(args, scorer)
    with LOCK:
        USER_SLOT["scorer"] = scorer
        for k in ("pointer", "name", "promoted_at", "eval", "last_good"):
            USER[k] = snap.get(k)
        USER["promotions"] = int(snap.get("promotions") or 0)
        USER["rollbacks"] = int(snap.get("rollbacks") or 0)
        if getattr(scorer, "name", "").endswith("+lora"):
            USER["name"] = f"user:{key}+lora"
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
    (c_q.c_v.attn_proj.c_fc.mlp_proj -> attention and MLP linears)."""
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
                ids, mask = tokenizer.render_conversation(
                    conv, max_tokens=min(int(seq_len), 1024))
            except Exception:
                continue
            if len(ids) < 3 or sum(mask[1:]) == 0:
                continue
            x = torch.tensor([ids[:-1]], device=device)
            y = torch.tensor([[t if mask[i + 1] == 1 else -1
                               for i, t in enumerate(ids[1:])]], device=device)
            tot += float(model(x, y))
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
        toks.extend(tokenizer.encode(d[:4000], prepend="<|bos|>"))
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
            x = torch.tensor([c[:-1]], device=device)
            y = torch.tensor([c[1:]], device=device)
            tot += float(model(x, y))
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
        model = scorer.engine.model
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
    model, tokenizer = scorer.engine.model, scorer.tokenizer
    device = model.get_device()
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
            ids, mask = tokenizer.render_conversation(
                conv, max_tokens=min(int(seq_len), 1024))
        except Exception:
            continue
        if len(ids) < 3 or sum(mask[1:]) == 0:
            continue
        x = torch.tensor([ids[:-1]], device=device)
        y = torch.tensor([[t if mask[i + 1] == 1 else -1
                           for i, t in enumerate(ids[1:])]], device=device)
        loss = model(x, y)
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
    # swap serving: a fresh base + the new adapter merged
    fresh = apply_persona(args, load_pointer_scorer(args, key))
    with LOCK:
        USER_SLOT["scorer"] = fresh
        USER["name"] = f"user:{key}+lora"
        PERSONA["rederivations"] += 1
        PERSONA["baseline"] = adapted_heldout
        PERSONA["probe"] = adapted_heldout
    return {"status": "ok", **report}


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
            saved_key = None
            try:
                from nanochat.checkpoint_manager import save_checkpoint
                ckdir = os.path.join(args.data_dir, "checkpoints",
                                     f"cpt-{time.strftime('%Y%m%d%H%M%S')}-{steps:06d}")
                save_checkpoint(ckdir, steps,
                                {k: v.detach().cpu() for k, v in candidate.state_dict().items()},
                                None, {"model_config": dict(live.meta["model_config"])})
                saved_key = os.path.basename(ckdir)
                # ring pruning - but never the user pointer's checkpoints
                with LOCK:
                    protected = {v for v in (USER["pointer"], USER["last_good"],
                                             USER["ready"]) if v}
                ring = sorted(os.listdir(os.path.join(args.data_dir, "checkpoints")))
                for old in ring[:-5]:
                    if old in protected:
                        continue
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
                "user": dict(
                    USER,
                    soaking=SOAK["current"],
                    soak=(lambda s: {"since_s": int(time.time() - s["since"]),
                                     "verdicts": s["verdicts"]} if s else None)(
                        SOAK["rings"].get(SOAK["current"])),
                    serving=USER_SLOT["scorer"] is not None,
                ),
                "persona": dict(PERSONA),
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
                cur = SOAK["current"]
                if cur and cur in SOAK["rings"]:
                    SOAK["rings"][cur]["verdicts"] += 1
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
            # any system content into the first user turn
            sys_txt = "\n".join(str(m.get("content", "")) for m in messages
                                if m.get("role") == "system").strip()
            messages = [m for m in messages if m.get("role") != "system"]
            if sys_txt and messages:
                m0 = dict(messages[0])
                m0["content"] = f"[system]\n{sys_txt}\n\n{m0.get('content', '')}"
                messages = [m0] + messages[1:]
            try:
                conversation = {"messages": list(messages)
                                + [{"role": "assistant", "content": ""}]}
                ids = scorer.tokenizer.render_for_completion(conversation)
                max_tokens = min(int(req.get("max_tokens") or 256), 1024)
                temp = float(req.get("temperature") or 0.7)
                results, _m = scorer.engine.generate_batch(
                    ids, num_samples=1, max_tokens=max_tokens,
                    temperature=temp, top_k=50)
                text = scorer.tokenizer.decode(results[0][len(ids):])
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
            scorer = make_scorer(args.checkpoint, args.data_dir)
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
