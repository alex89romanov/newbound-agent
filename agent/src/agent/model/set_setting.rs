use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["key", "value"] {
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
        let arg_0: String = o.get_string("key");
        let arg_1: String = o.get_string("value");
        set_setting(arg_0, arg_1)
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

pub fn set_setting(key: String, value: String) -> DataObject {
// set_setting: change one agent setting from the app UI. Updates the
// LIVE runtime globals (keys the code reads per-call - SALIENCE, the
// LLM arm - take effect immediately) AND rewrites botd.properties in
// place, preserving every other line, so the change survives restart.
// Empty value = remove the line and the live key (revert to default).
// Secret-looking keys (KEY/TOKEN/SECRET) are refused - edit the file
// yourself for those. Key names are validated; values must be one
// line.
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

let _ = prop;
let key = key.trim().to_string();
if key.is_empty() || !key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
    return err(format!("key must be UPPER_SNAKE ([A-Z0-9_]): {:?}", key));
}
if key.contains("KEY") || key.contains("TOKEN") || key.contains("SECRET") {
    return err("secret-looking keys are not editable from the UI - edit botd.properties directly".to_string());
}
let value = value.trim().to_string();
if value.contains('\n') || value.contains('\r') {
    return err("value must be a single line".to_string());
}
// live globals
{
    let g = DataStore::globals();
    if let Ok(s) = g.try_get_object("system") {
        if let Ok(a) = s.try_get_object("apps") {
            if let Ok(ag) = a.try_get_object("agent") {
                if let Ok(mut r) = ag.try_get_object("runtime") {
                    if value.is_empty() {
                        if r.has(&key) { r.remove_property(&key); }
                    } else {
                        r.put_string(&key, &value);
                    }
                }
            }
        }
    }
}
// the file
let root = match agent_root() { Ok(r) => r, Err(e) => return err(e) };
let botd = root.join("runtime").join("agent").join("botd.properties");
let text = std::fs::read_to_string(&botd).unwrap_or_default();
let mut out: Vec<String> = Vec::new();
let mut replaced = false;
for line in text.lines() {
    let t = line.trim();
    let is_key = !t.starts_with('#') && t.find('=')
        .map(|eq| t[..eq].trim() == key).unwrap_or(false);
    if is_key {
        if !value.is_empty() && !replaced {
            out.push(format!("{}={}", key, value));
        }
        replaced = true;
    } else {
        out.push(line.to_string());
    }
}
if !replaced && !value.is_empty() {
    out.push(format!("{}={}", key, value));
}
let mut content = out.join("\n");
if !content.is_empty() { content.push('\n'); }
if let Err(e) = std::fs::write(&botd, &content) {
    return err(format!("could not write {}: {}", botd.display(), e));
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("key", &key);
o.put_string("action", if value.is_empty() { "removed" } else if replaced { "updated" } else { "added" });
o

}
