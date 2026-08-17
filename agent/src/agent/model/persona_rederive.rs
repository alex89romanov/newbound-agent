use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        persona_rederive()
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

pub fn persona_rederive() -> DataObject {
// persona_rederive: POST /persona_rederive - derive (or re-derive)
// the personality adapter from persona/persona.jsonl against the user
// pointer's CURRENT base (Phase 8b). Blocks through the training run
// and returns the full derivation report; the gate (held-out persona
// gain + standard-loss guard) can reject it, which leaves the serving
// pointer untouched. The probe normally triggers this on its own -
// this command is the deliberate manual version.
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

let url = format!("http://127.0.0.1:{}/persona_rederive", prop("MODEL_SERVICE_PORT", "8077"));
let resp = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(1800000))
    .build()
    .post(&url)
    .set("Content-Type", "application/json")
    .send_string("{}");
match resp {
    Ok(r) => match r.into_string() {
        Ok(t) => match DataObject::try_from_string(&t) {
            Ok(d) => d,
            Err(_) => err(format!("service answered non-JSON at {}", url)),
        },
        Err(e) => err(format!("service read failed: {}", e)),
    },
    Err(ureq::Error::Status(_code, r)) => match r.into_string() {
        Ok(t) => match DataObject::try_from_string(&t) {
            Ok(d) => d,
            Err(_) => err("derivation refused".to_string()),
        },
        Err(e) => err(format!("service read failed: {}", e)),
    },
    Err(e) => err(format!("model service unreachable at {}: {}", url, e)),
}

}
