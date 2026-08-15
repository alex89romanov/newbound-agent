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
let mut g = DataStore::globals();
let mut ex = ensure_exec_state(&mut g);
let mut q = ex.get_array("queue");
q.push_object(perception.deep_copy());
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("queue_depth", q.len() as i64);
o.put_boolean("running", ex.get_boolean("running"));
o
