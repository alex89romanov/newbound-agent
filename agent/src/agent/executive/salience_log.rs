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
        salience_log()
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

pub fn salience_log() -> DataObject {
// salience_log: read the escalation/audit log (runtime, instance-owned,
// capped 1000) - the curriculum feedstock and the owner's audit surface.
// Returns the last 10 rows and totals computed from the rows themselves.
let store = DataStore::new();
let mut o = DataObject::new();
o.put_string("status", "ok");
if !store.exists("runtime", "salience_log") {
    o.put_int("total", 0);
    o.put_array("rows", DataArray::new());
    return o;
}
let d = store.get_data("runtime", "salience_log").get_object("data");
let rows = if d.has("rows") { d.get_array("rows") } else { DataArray::new() };
let mut esc = 0i64;
let mut aud = 0i64;
let mut dis = 0i64;
for i in 0..rows.len() {
    if let Ok(r) = rows.try_get_object(i) {
        if r.has("kind") && r.get_string("kind") == "escalation" { esc += 1; } else { aud += 1; }
        if r.has("disagree") && r.get_boolean("disagree") { dis += 1; }
    }
}
let mut last = DataArray::new();
let start = if rows.len() > 10 { rows.len() - 10 } else { 0 };
for i in start..rows.len() {
    if let Ok(r) = rows.try_get_object(i) { last.push_object(r.deep_copy()); }
}
o.put_int("total", rows.len() as i64);
o.put_int("escalations", esc);
o.put_int("audits", aud);
o.put_int("disagreements", dis);
o.put_array("rows", last);
o

}
