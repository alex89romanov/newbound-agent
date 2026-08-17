use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
use ndata::dataarray::DataArray;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        get_settings()
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

pub fn get_settings() -> DataObject {
// get_settings: the agent's effective configuration for the app UI.
// Known keys come back with value, default, whether explicitly set,
// and a `takes` hint (when a change takes effect). Any OTHER key
// explicitly present in botd.properties rides along too, with keys
// that look secret (KEY/TOKEN/SECRET) masked and locked. Read-only -
// writes go through set_setting.
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

let known: &[(&str, &str, &str)] = &[
    ("SALIENCE", "off", "immediately (service launch: next bootstrap)"),
    ("SALIENCE_STEER", "on", "immediately"),
    ("SALIENCE_BANDS", "low=0.2,high=0.8,deep=6", "immediately"),
    ("LLM", "VLLM", "next frontier call"),
    ("VLLM_URL", "", "next frontier call"),
    ("VLLM_MODEL", "", "next frontier call"),
    ("MODEL_CHECKPOINT", "stub", "next service relaunch"),
    ("MODEL_SERVICE_PORT", "8077", "next service relaunch"),
    ("MODEL_TRAIN", "on", "next service relaunch"),
    ("MODEL_MIX", "fresh=0.25,replay=0.25,standard=0.5", "next service relaunch"),
    ("MODEL_TRAIN_LR", "2e-5", "next service relaunch"),
    ("MODEL_GATE", "every=50,regress=0.02,fails=3", "next service relaunch"),
    ("MODEL_TRAIN_INTERVAL", "10", "next service relaunch"),
    ("USER_GATE", "mode=manual,soak_s=21600,verdicts=100,agree=0.75,regress=0.05,check_s=300", "next service relaunch"),
    ("USER_LORA", "mode=on,rank=8,alpha=16,lr=1e-3,steps=200,slack=0.1,min_gain=0.01,guard=0.2,targets=c_q.c_v", "next service relaunch"),
    ("NANOCHAT_TRAIN_ARGS", "--depth=20 --device-batch-size=8 --window-pattern=L", "next base training"),
    ("NANOCHAT_REPO", "https://github.com/karpathy/nanochat.git", "next env install"),
    ("NANOCHAT_DIST", "", "next base training"),
];
fn is_secret(k: &str) -> bool {
    k.contains("KEY") || k.contains("TOKEN") || k.contains("SECRET")
}
let mut set_keys: Vec<(String, String)> = Vec::new();
let root = match agent_root() { Ok(r) => r, Err(e) => return err(e) };
let botd = root.join("runtime").join("agent").join("botd.properties");
if let Ok(text) = std::fs::read_to_string(&botd) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(eq) = line.find('=') {
            set_keys.push((line[..eq].trim().to_string(), line[eq + 1..].trim().to_string()));
        }
    }
}
let mut list = DataArray::new();
for (k, dflt, takes) in known {
    let explicit = set_keys.iter().find(|(sk, _)| sk == k);
    let mut row = DataObject::new();
    row.put_string("key", k);
    row.put_string("default", dflt);
    row.put_string("takes", takes);
    row.put_boolean("set", explicit.is_some());
    let val = prop(k, dflt);
    if is_secret(k) {
        row.put_string("value", if explicit.is_some() { "••••••" } else { "" });
        row.put_boolean("locked", true);
    } else {
        row.put_string("value", &val);
        row.put_boolean("locked", false);
    }
    list.push_object(row);
}
for (k, v) in &set_keys {
    if known.iter().any(|(kk, _, _)| kk == k) { continue; }
    let mut row = DataObject::new();
    row.put_string("key", k);
    row.put_string("default", "");
    row.put_string("takes", "");
    row.put_boolean("set", true);
    let secret = is_secret(k);
    row.put_string("value", if secret { "••••••" } else { v });
    row.put_boolean("locked", secret);
    list.push_object(row);
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("path", &botd.display().to_string());
o.put_array("settings", list);
o

}
