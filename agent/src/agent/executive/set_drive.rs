use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["acts_per_hour"] {
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
        let arg_0: i64 = o.get_int("acts_per_hour");
        set_drive(arg_0)
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

pub fn set_drive(acts_per_hour: i64) -> DataObject {
// set_drive: the budget dial (understandingloop.md Phase 4). Acts per
// hour; 0 turns initiative off entirely - the loop still observes and
// orients, it just never decides. Takes effect immediately (the next
// idle tick may act). Clamped to 0..=3600.
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
        ex.put_int("drive", 4);
        ex.put_int("next_act_time", 0);
        ex.put_int("acts_total", 0);
        ex.put_int("work_depth", 0);
        g.put_object("AGENT_EXECUTIVE", ex);
    }
    g.get_object("AGENT_EXECUTIVE")
}

let mut g = DataStore::globals();
let mut ex = ensure_exec_state(&mut g);
let d = if acts_per_hour < 0 { 0 } else if acts_per_hour > 3600 { 3600 } else { acts_per_hour };
ex.put_int("drive", d);
ex.put_int("next_act_time", 0);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("drive", d);
o

}
