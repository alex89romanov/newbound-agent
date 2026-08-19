use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use crate::agent::llm::ask_llm::ask_llm;
use flowlang::flowlang::system::time::time;
use crate::agent::archivist::epistemic_work::epistemic_work;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        wonder()
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

pub fn wonder() -> DataObject {
// agent-archivist-wonder - idea generation from gaps (harvest H5).
// Gathers what the system KNOWS about its own incompleteness
// procedurally - the work queue's counts, the banks' sizes, the
// unbuilt charters - and asks the frontier for open QUESTIONS, not
// answers. Wonderings land in a runtime record (user data), capped
// and deduped: a list the owner can pick from, prune, or ignore on
// the mind tab. Never memory, never a claim - a wondering that
// matters graduates by the owner acting on it, not by any automatic
// promotion.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
let store = DataStore::new();
let work = epistemic_work();
let (stale, review, unpromoted) = (
    if work.has("stale") { work.get_int("stale") } else { 0 },
    if work.has("review") { work.get_int("review") } else { 0 },
    if work.has("unpromoted") { work.get_int("unpromoted") } else { 0 });
let mut banks = String::new();
if store.exists("runtime", "datasets") {
    let d = store.get_data("runtime", "datasets").get_object("data");
    if d.has("list") {
        let list = d.get_array("list");
        for i in 0..list.len() {
            if let Ok(m) = list.try_get_object(i) {
                banks.push_str(&format!("{}={} ", m.get_string("name"),
                    if m.has("rows") { m.get_int("rows") } else { 0 }));
            }
        }
    }
}
let prompt = format!(
    "You are an autonomous agent taking stock of its own gaps. Facts about your current state:\n- memory work queue: {} stale, {} awaiting review, {} unpromoted subject claims\n- training banks (rows): {}\n- chartered but unbuilt: the camera sensor (visual_event is reserved in the perception contract); hollis's calibration self-reporting\n- your senses: store journals, room audio, system state\nGenerate 1 to 2 OPEN QUESTIONS worth the owner's attention - genuine wonderings about what to build, verify, or watch next. Not tasks, not answers: questions that would teach something if pursued.\nReply with ONLY a JSON array, no fences: [\"<question>\", ...]",
    stale, review, unpromoted, banks.trim());
let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    ask_llm(prompt, Data::DNull)
})).unwrap_or_else(|_| "ERROR: ask_llm panicked".to_string());
if reply.starts_with("ERROR") {
    return err(format!("the frontier arm failed: {}", reply.chars().take(200).collect::<String>()));
}
let parsed = reply.find('[').and_then(|s0| reply.rfind(']').map(|e0| (s0, e0)))
    .filter(|(s0, e0)| e0 > s0)
    .and_then(|(s0, e0)| DataObject::try_from_string(&format!("{{\"a\":{}}}", &reply[s0..=e0])).ok())
    .and_then(|w| w.try_get_array("a").ok());
let qs = match parsed {
    Some(l) => l,
    None => {
        let mut o = DataObject::new();
        o.put_string("status", "ok");
        o.put_boolean("unparseable", true);
        o.put_int("added", 0);
        return o;
    }
};
let mut rec = if store.exists("runtime", "wonderings") {
    store.get_data("runtime", "wonderings")
} else {
    let mut r = DataObject::new();
    r.put_string("id", "wonderings");
    r.put_string("username", "system");
    r.put_array("readers", DataArray::new());
    r.put_array("writers", DataArray::new());
    r.put_object("data", DataObject::new());
    r
};
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let mut added = 0i64;
let now = time();
for i in 0..qs.len() {
    if let Ok(q) = qs.try_get_string(i) {
        let qt = q.trim().to_string();
        if qt.is_empty() { continue; }
        let mut dup = false;
        for j in 0..list.len() {
            if let Ok(w) = list.try_get_object(j) {
                if w.has("q") && w.get_string("q") == qt { dup = true; break; }
            }
        }
        if dup { continue; }
        let mut w = DataObject::new();
        w.put_string("q", &qt);
        w.put_int("t", now);
        w.put_string("author", "wonder");
        list.push_object(w);
        added += 1;
    }
}
while list.len() > 20 { list.remove_property(0); }
d.put_array("list", list.deep_copy());
rec.put_object("data", d);
rec.put_int("time", now);
store.set_data("runtime", "wonderings", rec);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("added", added);
o.put_int("held", list.len() as i64);
o

}
