// status: the observable half of observability-before-autonomy - current
// phase, queue depth, counters, and the last perception seen.
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
o
