use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["perception", "context"] {
        if !o.has(p) {
            let mut e = DataObject::new();
            e.put_string("status", "err");
            e.put_string("msg", &format!("missing required parameter: {}", p));
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", e);
            return result_obj;
        }
    }
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let arg_0: DataObject = o.get_object("perception");
        let arg_1: DataObject = o.get_object("context");
        salience(arg_0, arg_1)
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

pub fn salience(perception: DataObject, context: DataObject) -> DataObject {
// salience: SALIENCE_CTL's filler - a thin client on the resident model
// service (tools/model-service/service.py, one process: trainer +
// server, live/gated pointers inside). The service answers from
// whatever its live pointer holds: the stub scorer anywhere, a nanochat
// checkpoint on the owner's hardware - the zero-executive-change test
// is exactly that swap. A short timeout keeps a hung service from
// stalling the executive's tick; any error is a plain err result, which
// the executive treats as no-verdict and degrades like an unset seam.
fn service_url() -> String {
    // MODEL_SERVICE_URL in runtime/agent/botd.properties; the default
    // matches service.py's default port.
    (|| -> Option<String> {
        let s = DataStore::globals().try_get_object("system").ok()?;
        let a = s.try_get_object("apps").ok()?;
        let g = a.try_get_object("agent").ok()?;
        let r = g.try_get_object("runtime").ok()?;
        match r.try_get_string("MODEL_SERVICE_URL") {
            Ok(v) if !v.trim().is_empty() => Some(v.trim().trim_end_matches('/').to_string()),
            _ => None,
        }
    })().unwrap_or_else(|| "http://127.0.0.1:8077".to_string())
}
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}

let mut body = DataObject::new();
body.put_object("perception", perception.deep_copy());
body.put_object("context", context.deep_copy());
let url = format!("{}/salience", service_url());
let resp = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(1500))
    .build()
    .post(&url)
    .set("Content-Type", "application/json")
    .send_string(&body.to_string());
match resp {
    Ok(r) => match r.into_string() {
        Ok(t) => match DataObject::try_from_string(&t) {
            Ok(mut d) => {
                if !d.has("status") { d.put_string("status", "ok"); }
                d
            }
            Err(_) => err(format!("model service answered non-JSON at {}", url)),
        },
        Err(e) => err(format!("model service read failed: {}", e)),
    },
    Err(e) => err(format!("model service unreachable at {}: {}", url, e)),
}

}
