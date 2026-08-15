// set_drive: the budget dial (understandingloop.md Phase 4). Acts per
// hour; 0 turns initiative off entirely - the loop still observes and
// orients, it just never decides. Takes effect immediately (the next
// idle tick may act). Clamped to 0..=3600.
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
let mut ex = ensure_exec_state(&mut g);
let d = if acts_per_hour < 0 { 0 } else if acts_per_hour > 3600 { 3600 } else { acts_per_hour };
ex.put_int("drive", d);
ex.put_int("next_act_time", 0);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("drive", d);
o
