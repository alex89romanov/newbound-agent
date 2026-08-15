use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
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
// start (docs/one-memory-cycle.md B2): the executive loop, explicit and
// killable - it NEVER autostarts (understandingloop.md, the spawn/drive
// lesson: observability before autonomy). The skeleton loop only observes:
// it drains the perception queue and keeps its phase and counters visible
// in state. Orient/decide/act arrive in later phases; nothing here calls
// an LLM or acts on anything.
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
