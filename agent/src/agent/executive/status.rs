use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
use crate::agent::llm::ask_llm::ask_llm;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        status()
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

pub fn status() -> DataObject {
// status: the observable half - phase, queue, counters, orientation,
// initiative, and (Phase 5a) the salience picture: verdict counters,
// escalations vs audits vs cooldown drops, disagreement count.
// Shared runtime state under one globals key. Idempotent; every field a
// later read touches is initialized here, so no command path can panic on
// a missing key. (Keep every executive command's copy of this identical.)
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

let mut g = DataStore::globals();
let ex = ensure_exec_state(&mut g);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_boolean("running", ex.get_boolean("running"));
o.put_string("phase", &ex.get_string("phase"));
o.put_int("queue_depth", ex.get_array("queue").len() as i64);
o.put_int("perceived_total", ex.get_int("perceived_total"));
o.put_string("last_kind", &ex.get_string("last_kind"));
o.put_int("last_time", ex.get_int("last_time"));
o.put_int("started", ex.get_int("started"));
if ex.has("last_context") {
    o.put_object("last_context", ex.get_object("last_context"));
}
o.put_int("drive", if ex.has("drive") { ex.get_int("drive") } else { 4 });
o.put_int("acts_total", if ex.has("acts_total") { ex.get_int("acts_total") } else { 0 });
o.put_int("work_depth", if ex.has("work_depth") { ex.get_int("work_depth") } else { 0 });
let next_at = if ex.has("next_act_time") { ex.get_int("next_act_time") } else { 0 };
let now = time();
o.put_int("next_act_in_ms", if next_at > now { next_at - now } else { 0 });
if ex.has("last_act") {
    o.put_object("last_act", ex.get_object("last_act"));
}
for k in ["salience_calls", "escalations", "audits", "esc_dropped", "disagreements"] {
    o.put_int(k, if ex.has(k) { ex.get_int(k) } else { 0 });
}
o

}
