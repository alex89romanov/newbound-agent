use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        start()
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_object("a", ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn start() -> DataObject {
// start (docs/one-memory-cycle.md B2; understandingloop.md Phase 1): the
// executive loop, explicit and killable - it NEVER autostarts. The loop
// observes and, since Phase 1, ORIENTS: each perception drained from the
// queue is joined to the claims it touches via a direct call to
// agent:archivist:recall. No seam stands between them - the archivist is
// constitutive of the executive, not swappable, and the recall RESULT
// SHAPE is the contract (command dispatch is already store-late-bound).
// Decide/act arrive in later phases; nothing here calls an LLM or acts.
// Shared runtime state under one globals key. Idempotent; every field a
// later read touches is initialized here, so no command path can panic on
// a missing key.
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
        g.put_object("AGENT_EXECUTIVE", ex);
    }
    g.get_object("AGENT_EXECUTIVE")
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
                // orient (Phase 1): what does memory say about this?
                // Query = the envelope's kind + sensor + payload strings.
                // A broken or missing recall degrades to an empty context;
                // the loop itself must never die to a panic below it.
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
                // Sensor-bound claims (perception-contract section 3) ride
                // the envelope; surface the precise join next to recall's
                // fuzzy one.
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
                let looked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let cmd = Command::lookup("agent", "archivist", "recall");
                    let mut args = DataObject::new();
                    args.put_string("query", &qs);
                    args.put_string("domains", "");
                    args.put_int("limit", 3);
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
                ex.put_object("last_context", ctx);
            }
            q.remove_property(0);
        } else {
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

}
