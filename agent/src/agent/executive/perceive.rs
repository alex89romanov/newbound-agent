use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
use crate::agent::llm::ask_llm::ask_llm;
use crate::agent::msg::put::put;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["perception"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: DataObject = o.get_object("perception");
        perceive(arg_0)
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

pub fn perceive(perception: DataObject) -> DataObject {
// perceive (docs/perception-contract.md section 3): validate the envelope,
// enqueue, return. Non-blocking, and never journaled - perceptions are
// sensory flow, not memory; what deserves history reaches the store as
// claims and returns through the code sensor. Shape is validated loudly;
// VOCABULARY is not: an unknown kind queues as an opaque low-salience
// perception, so a new sensor works before the executive learns its words.
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

for k in ["kind", "time", "sensor", "payload"] {
    if !perception.has(k) {
        let mut o = DataObject::new();
        o.put_string("status", "err");
        o.put_string("msg", &format!("envelope must carry '{}' (docs/perception-contract.md section 1)", k));
        return o;
    }
}
if perception.has("v") && perception.get_int("v") != 1 {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", "unsupported envelope version (this executive speaks v1)");
    return o;
}
// H1 (owner call 0, ruled 2026-08-19): the executive records utterances
// on perceive - by KIND, never by sensor, so any acoustic sensor's
// transcripts join the one message universe with no sensor-specific
// code here (the layering rule). The sensor id rides as provenance, a
// tag. Recording is best-effort beside the queue, never a gate on it:
// a store hiccup must not cost a perception.
if perception.get_string("kind") == "acoustic_event" {
    if let Ok(p) = perception.try_get_object("payload") {
        if p.try_get_string("event").ok().as_deref() == Some("transcript") {
            let text = p.try_get_string("text").unwrap_or_default();
            if !text.trim().is_empty() {
                let entity = p.try_get_string("entity").unwrap_or_default();
                let sensor = perception.get_string("sensor");
                let _ = put("speaker".to_string(), "room".to_string(), text,
                            entity, sensor, String::new());
            }
        }
    }
}

let mut g = DataStore::globals();
let mut ex = ensure_exec_state(&mut g);
let mut q = ex.get_array("queue");
q.push_object(perception.deep_copy());
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("queue_depth", q.len() as i64);
o.put_boolean("running", ex.get_boolean("running"));
o

}
