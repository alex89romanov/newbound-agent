// service_status: GET /status on the resident model service - mode
// (stub|nanochat), live slot, counters, ingest and checkpoint-ring
// state. The dashboard's window into the serving half.
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

let url = format!("{}/status", service_url());
let resp = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(1500))
    .build()
    .get(&url)
    .call();
match resp {
    Ok(r) => match r.into_string() {
        Ok(t) => match DataObject::try_from_string(&t) {
            Ok(d) => d,
            Err(_) => err(format!("model service answered non-JSON at {}", url)),
        },
        Err(e) => err(format!("model service read failed: {}", e)),
    },
    Err(e) => err(format!("model service unreachable at {}: {}", url, e)),
}
