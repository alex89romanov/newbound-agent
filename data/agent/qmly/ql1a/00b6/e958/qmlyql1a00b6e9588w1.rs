// train_status: the base-training run's live state for the app UI -
// whether train.pid is alive, whether train_done exists in the
// checkpoint dir, and the tail of train.log (where base_train prints
// its step/ETA lines). Read-only.
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

let root = match agent_root() { Ok(r) => r, Err(e) => return err(e) };
let modeldir = root.join("runtime").join("agent").join("model");
let mut o = DataObject::new();
o.put_string("status", "ok");
let mut running = false;
let mut pid = String::new();
if let Ok(p) = std::fs::read_to_string(modeldir.join("train.pid")) {
    pid = p.trim().to_string();
    if !pid.is_empty() && std::path::Path::new(&format!("/proc/{}", pid)).exists() {
        running = true;
    }
}
o.put_boolean("running", running);
o.put_string("pid", &pid);
let checkpoint = prop("MODEL_CHECKPOINT", "stub");
o.put_boolean("done", checkpoint != "stub"
    && std::path::Path::new(&checkpoint).join("train_done").exists());
let mut lines = DataArray::new();
if let Ok(log) = std::fs::read_to_string(modeldir.join("train.log")) {
    let all: Vec<&str> = log.lines().collect();
    let start = all.len().saturating_sub(25);
    for l in &all[start..] {
        let mut l = l.to_string();
        if l.chars().count() > 220 { l = l.chars().take(220).collect(); }
        lines.push_string(&l);
    }
}
o.put_array("log", lines);
o
