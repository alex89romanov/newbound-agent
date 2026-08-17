// start (understandingloop.md Phases 1-5b): the executive loop, explicit
// and killable - it NEVER autostarts. Observe: drain perceptions.
// Orient: bind + recall, then the salience verdict from the resident
// model service - ON or OFF in settings (SALIENCE=on in
// botd.properties; owner's simplification, 2026-08-16: nothing
// pluggable, the agent.model.salience client is called directly,
// in-crate). The judge is ensured EAGERLY: when salience is on,
// agent.model.bootstrap fires in the background the moment start is
// called - never lazily behind a perception (owner, 2026-08-16: a
// multi-hour install must not wait for something to need it).
// Bootstrap is idempotent - a no-op probe when everything is already
// up. While the service isn't answering, the loop runs degraded - no
// verdict, nothing else changes. Verdicts are
// STEERING as of Phase 7 (owner go-ahead, 2026-08-17): the verdict is
// computed FIRST, on bound-claims-only context - the same conditions
// the agreement gate measures under - and then steers orientation.
// Below SALIENCE_BANDS low= the perception takes the FAST PATH: no
// recall, no escalation exposure, but the epsilon audit still applies,
// so the fast lane never goes unobserved. Above high= it earns DEEP
// orientation (deep= recall limit instead of 3). Between, exactly the
// old behavior. SALIENCE_STEER=off (live key) reverts to
// record-don't-act. Uncertain verdicts (0.35..=0.65,
// owner-blessed) escalate to the frontier and the disagreement lands in
// the runtime salience log - curriculum feedstock. A deterministic 5%
// epsilon-audit samples confident verdicts so the local judge never
// drifts unobserved. One frontier call per 5s at most (band escalations
// dropped in between are counted): tick-rate orientation must never
// become a cost explosion. Decide/Act (Phase 4) unchanged: idle-only,
// drive-budgeted, decay-only writes, attributed. Shared runtime state
// under one globals key. Idempotent; every field a later read touches is
// initialized here, so no command path can panic on a missing key.
// (Keep every executive command's copy of this identical.)
fn ensure_exec_state(g: &mut DataObject) -> DataObject {
    if !g.has("AGENT_EXECUTIVE") {
        let mut ex = DataObject::new();
        ex.put_boolean("running", false);
        ex.put_string("phase", "stopped");
        ex.put_array("queue", DataArray::new());
        ex.put_int("perceived_total", 0);
        ex.put_int("started", 0);
        ex.put_string("last_kind", "");
        ex.put_int("last_time", 0);
        ex.put_int("drive", 4);
        ex.put_int("next_act_time", 0);
        ex.put_int("acts_total", 0);
        ex.put_int("work_depth", 0);
        ex.put_int("salience_calls", 0);
        ex.put_int("escalations", 0);
        ex.put_int("audits", 0);
        ex.put_int("esc_dropped", 0);
        ex.put_int("disagreements", 0);
        ex.put_int("last_frontier_time", 0);
        g.put_object("AGENT_EXECUTIVE", ex);
    }
    g.get_object("AGENT_EXECUTIVE")
}

fn as_float(o: &DataObject, k: &str) -> f64 {
    if !o.has(k) { return -1.0; }
    match o.get_property(k) {
        Data::DFloat(f) => f,
        Data::DInt(i) => i as f64,
        _ => -1.0,
    }
}
fn salience_on() -> bool {
    (|| -> Option<String> {
        let s = DataStore::globals().try_get_object("system").ok()?;
        let a = s.try_get_object("apps").ok()?;
        let g = a.try_get_object("agent").ok()?;
        let r = g.try_get_object("runtime").ok()?;
        r.try_get_string("SALIENCE").ok()
    })().map(|v| {
        let v = v.trim().to_lowercase();
        v == "on" || v == "true" || v == "1" || v == "yes"
    }).unwrap_or(false)
}
fn steer_cfg() -> (bool, f64, f64, i64) {
    // SALIENCE_STEER=off disables steering live. SALIENCE_BANDS tunes
    // it: low=0.2,high=0.8,deep=6. The escalation band (0.35..=0.65)
    // is separate and owner-blessed; keep low/high outside it.
    fn rprop(key: &str) -> Option<String> {
        let s = DataStore::globals().try_get_object("system").ok()?;
        let a = s.try_get_object("apps").ok()?;
        let g = a.try_get_object("agent").ok()?;
        let r = g.try_get_object("runtime").ok()?;
        match r.try_get_string(key) {
            Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
            _ => None,
        }
    }
    let on = rprop("SALIENCE_STEER").map(|v| v.to_lowercase() != "off").unwrap_or(true);
    let mut low = 0.2f64;
    let mut high = 0.8f64;
    let mut deep = 6i64;
    if let Some(spec) = rprop("SALIENCE_BANDS") {
        for part in spec.split(',') {
            if let Some(eq) = part.find('=') {
                let k = part[..eq].trim();
                let v = part[eq + 1..].trim();
                if k == "low" { if let Ok(f) = v.parse() { low = f; } }
                else if k == "high" { if let Ok(f) = v.parse() { high = f; } }
                else if k == "deep" { if let Ok(i) = v.parse() { deep = i; } }
            }
        }
    }
    (on, low, high, deep)
}
// The escalation log: runtime-library record, instance-owned like
// archivist_queue, capped, unjournaled (so the store sensor never
// perceives its own audit trail - no feedback channel).
fn log_salience_row(row: DataObject) {
    let store = DataStore::new();
    let mut rec = if store.exists("runtime", "salience_log") {
        store.get_data("runtime", "salience_log")
    } else {
        let mut r = DataObject::new();
        r.put_string("id", "salience_log");
        r.put_string("username", "system");
        r.put_array("readers", DataArray::new());
        r.put_array("writers", DataArray::new());
        r.put_object("data", DataObject::new());
        r
    };
    let mut d = rec.get_object("data");
    let mut rows = if d.has("rows") { d.get_array("rows") } else { DataArray::new() };
    rows.push_object(row);
    while rows.len() > 1000 { rows.remove_property(0); }
    d.put_array("rows", rows);
    rec.put_object("data", d);
    rec.put_int("time", time());
    store.set_data("runtime", "salience_log", rec);
}

let mut g = DataStore::globals();
let mut ex = ensure_exec_state(&mut g);
if ex.get_boolean("running") {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_boolean("already_running", true);
    return o;
}
ex.put_boolean("running", true);
ex.put_string("phase", "idle");
ex.put_int("started", time());
// Ensure the judge NOW, not when a perception happens to need it:
// bootstrap in the background (install if configured, launch if down,
// no-op if answering). The loop below never triggers installs.
if salience_on() {
    std::thread::spawn(|| {
        let _ = std::panic::catch_unwind(|| {
            crate::agent::model::bootstrap::bootstrap()
        });
    });
}

std::thread::spawn(move || {
    let g = DataStore::globals();
    loop {
        let ex = g.get_object("AGENT_EXECUTIVE");
        if !ex.get_boolean("running") { break; }
        let mut ex = ex;
        let mut q = ex.get_array("queue");
        if q.len() > 0 {
            ex.put_string("phase", "observing");
            if let Ok(p) = q.try_get_object(0) {
                ex.put_int("perceived_total", ex.get_int("perceived_total") + 1);
                if p.has("kind") { ex.put_string("last_kind", &p.get_string("kind")); }
                if p.has("time") { ex.put_int("last_time", p.get_int("time")); }
                ex.put_string("phase", "orienting");
                let mut qparts: Vec<String> = Vec::new();
                if p.has("kind") { qparts.push(p.get_string("kind")); }
                if p.has("sensor") { qparts.push(p.get_string("sensor")); }
                if let Ok(pl) = p.try_get_object("payload") {
                    for k in pl.clone().keys() {
                        if let Ok(v) = pl.try_get_string(&k) { qparts.push(v); }
                    }
                }
                let mut qs = qparts.join(" ");
                if qs.chars().count() > 240 { qs = qs.chars().take(240).collect(); }
                let mut ctx = DataObject::new();
                ctx.put_string("query", &qs);
                ctx.put_int("matched", 0);
                if p.has("claims") {
                    if let Ok(bc) = p.try_get_array("claims") {
                        ctx.put_int("bound", bc.len() as i64);
                        if bc.len() > 0 {
                            if let Ok(b0) = bc.try_get_object(0) {
                                if b0.has("claim") { ctx.put_string("bound_top", &b0.get_string("claim")); }
                                if b0.has("stale") { ctx.put_boolean("bound_top_stale", b0.get_boolean("stale")); }
                            }
                        }
                    }
                }
                // ── salience first (Phases 5-7): the verdict is computed
                // on bound-claims-only context - the same conditions the
                // agreement gate measures under - then STEERS how much
                // orientation this perception earns. ────────────────────
                let mut recall_limit: i64 = 3;
                if salience_on() {
                    let judged = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        salience(p.deep_copy(), ctx.deep_copy())
                    }));
                    if let Ok(a) = judged {
                        let sal = as_float(&a, "salient");
                        if sal >= 0.0 {
                            let n = if ex.has("salience_calls") { ex.get_int("salience_calls") } else { 0 };
                            ex.put_int("salience_calls", n + 1);
                            ctx.put_float("salience", sal);
                            if a.has("reasoning") { ctx.put_string("salience_why", &a.get_string("reasoning")); }
                            let (steer_on, lo, hi, deep) = steer_cfg();
                            if steer_on {
                                if sal < lo {
                                    // the fast path: noise earns no recall
                                    ctx.put_string("steer", "fast");
                                    recall_limit = 0;
                                    let c = if ex.has("fast_skips") { ex.get_int("fast_skips") } else { 0 };
                                    ex.put_int("fast_skips", c + 1);
                                } else if sal > hi {
                                    // deep orientation: critical earns more memory
                                    ctx.put_string("steer", "deep");
                                    recall_limit = deep;
                                    let c = if ex.has("deep_orients") { ex.get_int("deep_orients") } else { 0 };
                                    ex.put_int("deep_orients", c + 1);
                                } else {
                                    ctx.put_string("steer", "normal");
                                }
                            }
                            let band = sal >= 0.35 && sal <= 0.65;
                            // Deterministic epsilon: hash of query +
                            // perception time - reproducible sampling,
                            // no rand dependency.
                            let eps = {
                                let mut h: u64 = 0xcbf29ce484222325;
                                for b in qs.as_bytes() {
                                    h ^= *b as u64;
                                    h = h.wrapping_mul(0x100000001b3);
                                }
                                let pt = if p.has("time") { p.get_int("time") } else { 0 };
                                h ^= pt as u64;
                                (h % 100) < 5
                            };
                            if band || eps {
                                let now2 = time();
                                let lf = if ex.has("last_frontier_time") { ex.get_int("last_frontier_time") } else { 0 };
                                if now2 - lf >= 5000 {
                                    ex.put_int("last_frontier_time", now2);
                                    let fprompt = format!(
                                        "You are the salience auditor for an autonomous agent's perception stream.\nPERCEPTION: {}\nCONTEXT: recall matched {} claims.\nThe resident model rated salience {:.2}.\nIndependently rate how much this perception matters to the agent's understanding of its environment, from 0.0 (noise) to 1.0 (critical).\nReply with ONLY a JSON object: {{\"salient\": <0.0-1.0>, \"reasoning\": \"<one sentence>\"}}",
                                        qs, ctx.get_int("matched"), sal);
                                    let fres = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        ask_llm(fprompt, Data::DNull)
                                    }));
                                    if let Ok(resp) = fres {
                                        if !resp.starts_with("ERROR") {
                                            if let (Some(s0), Some(e0)) = (resp.find('{'), resp.rfind('}')) {
                                                if e0 > s0 {
                                                    if let Ok(fd) = DataObject::try_from_string(&resp[s0..=e0]) {
                                                        let fsal = as_float(&fd, "salient");
                                                        if fsal >= 0.0 {
                                                            let disagree = (sal - fsal).abs() > 0.25;
                                                            let mut row = DataObject::new();
                                                            row.put_int("time", now2);
                                                            row.put_string("kind", if band { "escalation" } else { "audit" });
                                                            row.put_string("input", &qs);
                                                            row.put_float("local", sal);
                                                            row.put_float("frontier", fsal);
                                                            if fd.has("reasoning") { row.put_string("frontier_why", &fd.get_string("reasoning")); }
                                                            row.put_boolean("disagree", disagree);
                                                            log_salience_row(row);
                                                            let k = if band { "escalations" } else { "audits" };
                                                            let c = if ex.has(k) { ex.get_int(k) } else { 0 };
                                                            ex.put_int(k, c + 1);
                                                            if disagree {
                                                                let dcount = if ex.has("disagreements") { ex.get_int("disagreements") } else { 0 };
                                                                ex.put_int("disagreements", dcount + 1);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if band {
                                    let dropped = if ex.has("esc_dropped") { ex.get_int("esc_dropped") } else { 0 };
                                    ex.put_int("esc_dropped", dropped + 1);
                                }
                            }
                        }
                        // sal < 0: no verdict - the service is down or
                        // still bootstrapping. Degrade in place; the
                        // eager bootstrap at start owns recovery.
                    }
                }
                // orient: recall at the steered depth (fast path skips)
                if recall_limit > 0 {
                    let looked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let cmd = Command::lookup("agent", "archivist", "recall");
                        let mut args = DataObject::new();
                        args.put_string("query", &qs);
                        args.put_string("domains", "");
                        args.put_int("limit", recall_limit);
                        cmd.execute(args)
                    }));
                    if let Ok(Ok(r)) = looked {
                        if r.has("a") {
                            let a = r.get_object("a");
                            if a.has("status") && a.get_string("status") == "ok" {
                                ctx.put_int("matched", a.get_int("matched"));
                                let cl = a.get_array("claims");
                                if cl.len() > 0 {
                                    let top = cl.get_object(0);
                                    if top.has("claim") { ctx.put_string("top_claim", &top.get_string("claim")); }
                                    if top.has("home") { ctx.put_string("top_home", &top.get_string("home")); }
                                    if top.has("stale") { ctx.put_boolean("top_stale", top.get_boolean("stale")); }
                                }
                            }
                        }
                    }
                }
                // per-sensor accounting - sensor-AGNOSTIC by design
                // (owner's rule: agent never knows which sensors exist;
                // rows are keyed by the envelope's own sensor field)
                {
                    let sname = if p.has("sensor") { p.get_string("sensor") } else { "unknown".to_string() };
                    let mut sc = if ex.has("sensor_counts") { ex.get_object("sensor_counts") } else {
                        let o2 = DataObject::new();
                        ex.put_object("sensor_counts", o2.clone());
                        o2
                    };
                    let mut row = if sc.has(&sname) { sc.get_object(&sname) } else { DataObject::new() };
                    row.put_int("count", (if row.has("count") { row.get_int("count") } else { 0 }) + 1);
                    row.put_int("last_time", time());
                    if ctx.has("steer") {
                        let st = ctx.get_string("steer");
                        if st == "fast" {
                            row.put_int("fast", (if row.has("fast") { row.get_int("fast") } else { 0 }) + 1);
                        } else if st == "deep" {
                            row.put_int("deep", (if row.has("deep") { row.get_int("deep") } else { 0 }) + 1);
                        }
                    }
                    sc.put_object(&sname, row);
                }
                ex.put_object("last_context", ctx);
            }
            q.remove_property(0);
        } else {
            let now = time();
            let drive = if ex.has("drive") { ex.get_int("drive") } else { 4 };
            let next_at = if ex.has("next_act_time") { ex.get_int("next_act_time") } else { 0 };
            if drive > 0 && now >= next_at {
                ex.put_string("phase", "deciding");
                ex.put_int("next_act_time", now + 3600000 / drive);
                let worked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let cmd = Command::lookup("agent", "archivist", "epistemic_work");
                    cmd.execute(DataObject::new())
                }));
                if let Ok(Ok(r)) = worked {
                    if r.has("a") {
                        let a = r.get_object("a");
                        if a.has("total") { ex.put_int("work_depth", a.get_int("total")); }
                        if a.has("items") && a.get_array("items").len() > 0 {
                            let it = a.get_array("items").get_object(0);
                            let kind = it.get_string("kind");
                            let mut act = DataObject::new();
                            act.put_string("kind", &kind);
                            act.put_string("claim", &it.get_string("claim"));
                            act.put_string("home", &format!("{}.{}", it.get_string("lib"), it.get_string("domain")));
                            act.put_string("why", &it.get_string("why"));
                            act.put_int("time", now);
                            if kind == "stale" {
                                ex.put_string("phase", "acting");
                                let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let cmd = Command::lookup("agent", "archivist", "decay");
                                    let mut args = DataObject::new();
                                    args.put_string("lib", &it.get_string("lib"));
                                    args.put_string("domain", &it.get_string("domain"));
                                    args.put_string("claim", &it.get_string("claim"));
                                    args.put_string("author", "executive");
                                    cmd.execute(args)
                                }));
                                let mut action = "act_failed".to_string();
                                if let Ok(Ok(rr)) = done {
                                    if rr.has("a") {
                                        let aa = rr.get_object("a");
                                        if aa.has("action") { action = aa.get_string("action"); }
                                        if aa.has("before") { act.put_string("before", &aa.get_string("before")); }
                                        if aa.has("after") { act.put_string("after", &aa.get_string("after")); }
                                    }
                                }
                                act.put_string("action", &action);
                            } else {
                                act.put_string("action", "surfaced");
                            }
                            ex.put_object("last_act", act);
                            let n = if ex.has("acts_total") { ex.get_int("acts_total") } else { 0 };
                            ex.put_int("acts_total", n + 1);
                        }
                    }
                }
            }
            ex.put_string("phase", "idle");
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    let mut ex = g.get_object("AGENT_EXECUTIVE");
    ex.put_string("phase", "stopped");
});

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_boolean("already_running", false);
o
