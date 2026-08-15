// status: the observable half of observability-before-autonomy - phase,
// queue depth, counters, the last perception's orientation, and (Phase
// 4) the initiative picture: drive budget, epistemic work depth, act
// count, time to the next allowed act, and the last act WITH its
// attribution (kind, claim, home, why, action, before/after).
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
o
