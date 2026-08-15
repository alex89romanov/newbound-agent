// status: sensor observability - running flag, cursor, emissions, and
// what the last emitted perception looked like (label + bound claims).
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
let st = ensure_sensor_state(&mut g);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_boolean("running", st.get_boolean("running"));
o.put_int("cursor", st.get_int("cursor"));
o.put_int("emitted_total", st.get_int("emitted_total"));
o.put_int("started", st.get_int("started"));
o.put_string("last_label", &st.get_string("last_label"));
o.put_int("last_bound", st.get_int("last_bound"));
o
