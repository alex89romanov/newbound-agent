use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        service_stop()
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

pub fn service_stop() -> DataObject {
// service_stop: the GPU off-switch. POST /shutdown on the resident
// service - it answers, then exits cleanly, freeing ALL of the agent's
// GPU memory (serving model, training candidate, optimizer). Durable
// state survives on disk: ring checkpoints, replay reservoir, held-out
// sets, metrics. The only loss is the candidate's steps since its last
// promotion. Resume = agent-model-bootstrap (or the mind tab's
// button): the relaunched service loads the NEWEST RING CHECKPOINT, so
// promoted CPT progress carries across the off/on cycle. To keep it
// off across an executive restart, set SALIENCE=off first (live key).
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
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
let url = format!("http://127.0.0.1:{}/shutdown", prop("MODEL_SERVICE_PORT", "8077"));
let resp = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(3000))
    .build()
    .post(&url)
    .set("Content-Type", "application/json")
    .send_string("{}");
match resp {
    Ok(r) => match r.into_string() {
        Ok(_) => {
            std::thread::sleep(std::time::Duration::from_millis(800));
            let mut o = DataObject::new();
            o.put_string("status", "ok");
            o.put_string("msg", "service stopped - GPU released; resume with bootstrap");
            o
        }
        Err(e) => err(format!("service read failed: {}", e)),
    },
    Err(e) => err(format!("model service unreachable at {} (already stopped?): {}", url, e)),
}

}
