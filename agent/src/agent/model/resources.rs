use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::system_call::system_call;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        resources()
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

pub fn resources() -> DataObject {
// agent-model-resources - the resource map (spectrum S1): GPUs and
// disk, from the service's /status when it answers (torch's view),
// from nvidia-smi directly when it doesn't - so the map is never
// simply absent. Empty gpus on a CPU-only box is an answer, not an
// error. First customers: the S5 posture solver, ring byte-budget
// warnings, birth-run sizing.
fn prop(key: &str, dflt: &str) -> String {
    (|| -> Option<String> {
        let s = DataStore::globals().try_get_object("system").ok()?;
        let a = s.try_get_object("apps").ok()?;
        let g = a.try_get_object("agent").ok()?;
        let r = g.try_get_object("runtime").ok()?;
        match r.try_get_string(key) {
            Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
            _ => None,
        }
    })().unwrap_or_else(|| dflt.to_string())
}

let status_url = format!("http://127.0.0.1:{}/status",
                         prop("MODEL_SERVICE_PORT", "8077"));
let mut o = DataObject::new();
o.put_string("status", "ok");

// host facts (one probe, two consumers - S1's solver reads these as
// data, H4's system sensor emits their threshold-crossings as
// perceptions; the map is grown here so a second probe never exists)
{
    let mut host = DataObject::new();
    if let Ok(mi) = std::fs::read_to_string("/proc/meminfo") {
        let grab = |key: &str| -> i64 {
            mi.lines().find(|l| l.starts_with(key))
              .and_then(|l| l.split_whitespace().nth(1))
              .and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) / 1024
        };
        host.put_int("mem_total_mb", grab("MemTotal:"));
        host.put_int("mem_avail_mb", grab("MemAvailable:"));
    }
    if let Ok(la) = std::fs::read_to_string("/proc/loadavg") {
        if let Some(l1) = la.split_whitespace().next()
                .and_then(|v| v.parse::<f64>().ok()) {
            host.put_float("load1", l1);
        }
    }
    host.put_int("cpus", std::thread::available_parallelism()
        .map(|n| n.get() as i64).unwrap_or(0));
    // service liveness is an observation the sensor coalesces into
    // up/down transitions; the probe just states it
    let alive = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_millis(800))
        .build().get(&status_url).call().is_ok();
    host.put_boolean("service_alive", alive);
    o.put_object("host", host);
}

// primary: the service's own view (torch's numbers)
if let Ok(r) = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(2500))
    .build()
    .get(&status_url)
    .call() {
    if let Ok(t) = r.into_string() {
        if let Ok(d) = DataObject::try_from_string(&t) {
            if let Ok(res) = d.try_get_object("resources") {
                o.put_object("resources", res);
                o.put_string("via", "service");
                return o;
            }
        }
    }
}

// fallback: nvidia-smi directly
let mut gpus = DataArray::new();
let mut x = DataArray::new();
x.push_string("bash");
x.push_string("-c");
x.push_string("nvidia-smi --query-gpu=index,name,memory.total,memory.free --format=csv,noheader,nounits 2>/dev/null");
let r = system_call(x);
if r.has("out") {
    for ln in r.get_string("out").lines() {
        let parts: Vec<String> = ln.split(',').map(|p| p.trim().to_string()).collect();
        if parts.len() >= 4 {
            if let (Ok(i), Ok(t), Ok(fr)) = (parts[0].parse::<i64>(),
                                             parts[2].parse::<i64>(),
                                             parts[3].parse::<i64>()) {
                let mut g = DataObject::new();
                g.put_int("index", i);
                g.put_string("name", &parts[1]);
                g.put_int("total_mb", t);
                g.put_int("free_mb", fr);
                gpus.push_object(g);
            }
        }
    }
}

// disk: free space where the model subsystem's bytes live
let mut res = DataObject::new();
res.put_array("gpus", gpus);
let root = DataStore::new().root.canonicalize().ok()
    .and_then(|r| r.parent().map(|p| p.to_path_buf()));
if let Some(root) = root {
    let modeldir = root.join("runtime").join("agent").join("model");
    let mut x2 = DataArray::new();
    x2.push_string("bash");
    x2.push_string("-c");
    x2.push_string(&format!("df -k --output=avail '{}' 2>/dev/null | tail -1",
                            modeldir.display()));
    let r2 = system_call(x2);
    if r2.has("out") {
        if let Ok(kb) = r2.get_string("out").trim().parse::<i64>() {
            res.put_float("disk_free_gb",
                          (kb as f64 / 1048576.0 * 10.0).round() / 10.0);
        }
    }
}
o.put_object("resources", res);
o.put_string("via", "nvidia-smi");
o

}
