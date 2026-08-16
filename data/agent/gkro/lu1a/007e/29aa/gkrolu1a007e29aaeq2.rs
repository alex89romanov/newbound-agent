// salience: the executive's judge, called DIRECTLY (in-crate, no
// dispatch) when SALIENCE=on in botd.properties - on or off in
// settings, nothing pluggable (owner's simplification, 2026-08-16).
// POSTs {perception, context} to the resident service's /salience and
// returns its {salient, reasoning, pointer, ms}. A short timeout keeps
// a hung service from stalling the tick; any failure is a plain err the
// executive treats as no-verdict - and its cue to fire bootstrap.
fn prop(key: &str, dflt: &str) -> String {
    // Settings live in runtime/agent/botd.properties like everything else.
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
fn service_url() -> String {
    format!("http://127.0.0.1:{}", prop("MODEL_SERVICE_PORT", "8077"))
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
