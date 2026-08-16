// promote_pointer: POST /promote on the resident service - load the
// newest ring checkpoint into the inactive slot and swap the live
// pointer. The manual counterpart of the trainer's gated promotions.
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
fn agent_root() -> Result<std::path::PathBuf, String> {
    let root = DataStore::new().root;
    let root = root.canonicalize().map_err(|e| format!("store root: {}", e))?;
    Ok(root.parent().ok_or("store root has no parent")?.to_path_buf())
}

let _ = agent_root;
let url = format!("http://127.0.0.1:{}/promote", prop("MODEL_SERVICE_PORT", "8077"));
let resp = ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(4000))
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
    Err(e) => err(format!("model service unreachable at {}: {}", url, e)),
}
