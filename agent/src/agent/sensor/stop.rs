use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        stop()
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

pub fn stop() -> DataObject {
// stop: flip the flag; the tailer exits at its next sweep.
// Sensor runtime state under one globals key (the executive's pattern).
// The cursor is runtime state for now - it resets to `now` on start, so a
// restart never replays history; a persisted cursor record arrives with
// the sensor-state work the contract reserves for it.
fn ensure_sensor_state(g: &mut DataObject) -> DataObject {
    if !g.has("AGENT_SENSOR_STORE") {
        let mut st = DataObject::new();
        st.put_boolean("running", false);
        st.put_int("cursor", 0);
        st.put_int("emitted_total", 0);
        st.put_int("started", 0);
        st.put_string("last_label", "");
        st.put_int("last_bound", 0);
        g.put_object("AGENT_SENSOR_STORE", st);
    }
    g.get_object("AGENT_SENSOR_STORE")
}

let mut g = DataStore::globals();
let mut st = ensure_sensor_state(&mut g);
let was = st.get_boolean("running");
st.put_boolean("running", false);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_boolean("was_running", was);
o

}
