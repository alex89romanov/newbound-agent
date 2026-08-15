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
