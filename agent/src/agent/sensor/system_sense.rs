use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
use crate::agent::model::resources::resources;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        system_sense()
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

pub fn system_sense() -> DataObject {
// agent-sensor-system_sense - the system sensor's one sweep (harvest
// H4; contract kind system_state). Agent-native: it joins the built-in
// family, which is what makes reading the resource map directly legal
// (the sensor-layering rule) - ONE probe, two consumers: the posture
// solver reads agent-model-resources as data; this sweep emits its
// THRESHOLD-CROSSINGS as perceptions. Coalescing is the sensor's first
// responsibility: a state is emitted when it CHANGES BAND, never per
// sweep - a full disk is one perception, a full disk staying full is
// zero. Payloads are observations ("disk_free_gb crossed below 5"),
// never conclusions ("disk will fill by Friday" is a claim the
// executive may infer). Callable deliberately (this command); the
// tailer loop calls it on its own cadence so no second loop exists.
fn band(metric: &str, v: f64) -> i64 {
    // band edges per metric; a crossing = an emission. 0 is nominal.
    match metric {
        "disk_free_gb"  => if v < 2.0 { 2 } else if v < 10.0 { 1 } else { 0 },
        "mem_avail_pct" => if v < 5.0 { 2 } else if v < 15.0 { 1 } else { 0 },
        "load_per_cpu"  => if v > 2.0 { 2 } else if v > 1.0 { 1 } else { 0 },
        "gpu_free_pct"  => if v < 5.0 { 2 } else if v < 20.0 { 1 } else { 0 },
        _ => 0,
    }
}
let mut g = DataStore::globals();
let mut st = if g.has("AGENT_SENSOR_SYSTEM") { g.get_object("AGENT_SENSOR_SYSTEM") }
    else {
        let mut s = DataObject::new();
        s.put_object("bands", DataObject::new());
        s.put_int("emitted_total", 0);
        s.put_int("last_sweep", 0);
        g.put_object("AGENT_SENSOR_SYSTEM", s.deep_copy());
        s
    };
let res = resources();
let mut samples: Vec<(String, f64, String)> = Vec::new(); // (metric, value, detail)
if res.has("resources") {
    let r = res.get_object("resources");
    if r.has("disk_free_gb") {
        samples.push(("disk_free_gb".to_string(), r.get_float("disk_free_gb"), String::new()));
    }
    if r.has("gpus") {
        let gpus = r.get_array("gpus");
        for i in 0..gpus.len() {
            if let Ok(gpu) = gpus.try_get_object(i) {
                if gpu.has("total_mb") && gpu.has("free_mb") && gpu.get_int("total_mb") > 0 {
                    let pct = 100.0 * gpu.get_int("free_mb") as f64 / gpu.get_int("total_mb") as f64;
                    samples.push((format!("gpu{}_free_pct", gpu.get_int("index")), pct,
                        if gpu.has("name") { gpu.get_string("name") } else { String::new() }));
                }
            }
        }
    }
}
if res.has("host") {
    let h = res.get_object("host");
    if h.has("mem_total_mb") && h.has("mem_avail_mb") && h.get_int("mem_total_mb") > 0 {
        samples.push(("mem_avail_pct".to_string(),
            100.0 * h.get_int("mem_avail_mb") as f64 / h.get_int("mem_total_mb") as f64,
            String::new()));
    }
    if h.has("load1") && h.has("cpus") && h.get_int("cpus") > 0 {
        samples.push(("load_per_cpu".to_string(),
            h.get_float("load1") / h.get_int("cpus") as f64, String::new()));
    }
    // service liveness: a boolean band of its own - up/down transitions
    if h.has("service_alive") {
        samples.push(("service_alive".to_string(),
            if h.get_boolean("service_alive") { 1.0 } else { 0.0 }, String::new()));
    }
}

let mut bands = st.get_object("bands");
let mut emitted = 0i64;
let now = time();
for (metric, value, detail) in &samples {
    // the band key: gpuN_free_pct all share the gpu_free_pct edges
    let edge_key = if metric.starts_with("gpu") { "gpu_free_pct" }
        else { metric.as_str() };
    let b = if edge_key == "service_alive" { if *value > 0.5 { 0 } else { 2 } }
        else { band(edge_key, *value) };
    let prev = if bands.has(metric) { bands.get_int(metric) } else { -1 };
    // first sweep seeds silently: startup is not an event
    if prev >= 0 && b != prev {
        let mut p = DataObject::new();
        p.put_int("v", 1);
        p.put_string("kind", "system_state");
        p.put_int("time", now);
        p.put_string("sensor", "system");
        let mut pl = DataObject::new();
        pl.put_string("metric", metric);
        pl.put_float("value", (*value * 100.0).round() / 100.0);
        pl.put_int("band", b);
        pl.put_int("prev_band", prev);
        if !detail.is_empty() { pl.put_string("detail", detail); }
        p.put_object("payload", pl);
        p.put_array("claims", DataArray::new());
        // worsening outranks recovery; both are worth noticing
        p.put_float("salience_hint", if b > prev { 0.5 + 0.2 * b as f64 } else { 0.3 });
        let sent = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let cmd = Command::lookup("agent", "executive", "perceive");
            let mut args = DataObject::new();
            args.put_object("perception", p.deep_copy());
            cmd.execute(args)
        }));
        if sent.is_ok() { emitted += 1; }
    }
    bands.put_int(metric, b);
}
st.put_object("bands", bands.deep_copy());
st.put_int("emitted_total", st.get_int("emitted_total") + emitted);
st.put_int("last_sweep", now);
g.put_object("AGENT_SENSOR_SYSTEM", st.deep_copy());

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("sampled", samples.len() as i64);
o.put_int("emitted", emitted);
o.put_object("bands", bands);
o

}
