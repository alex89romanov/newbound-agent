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
