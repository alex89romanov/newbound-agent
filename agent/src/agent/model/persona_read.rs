use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        persona_read()
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

pub fn persona_read() -> DataObject {
// persona_read: the persona corpus (runtime/agent/model/persona/
// persona.jsonl) as parsed rows for the mind tab's editor. When the
// corpus is empty or absent, the SHIPPED DEFAULT SEED
// (data/agent/_ASSETS/persona-seed.jsonl) is returned instead with
// seed:true - nothing is written until the owner saves; adopting the
// default is a deliberate click, not a side effect.
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
fn valid_row(o: &DataObject) -> bool {
    (o.has("user") && o.has("assistant")) || o.has("messages")
}

let root = match agent_root() { Ok(r) => r, Err(e) => return err(e) };
let path = root.join("runtime").join("agent").join("model")
    .join("persona").join("persona.jsonl");
let mut text = String::new();
if path.exists() {
    match std::fs::read_to_string(&path) {
        Ok(t) => text = t,
        Err(e) => return err(format!("persona read failed: {}", e)),
    }
}
let mut seed = false;
if text.trim().is_empty() {
    let seedpath = DataStore::new().root.join("agent").join("_ASSETS")
        .join("persona-seed.jsonl");
    if let Ok(t) = std::fs::read_to_string(&seedpath) {
        text = t;
        seed = true;
    }
}
let mut rows = DataArray::new();
let mut invalid = 0;
for line in text.lines() {
    let line = line.trim();
    if line.is_empty() { continue; }
    match DataObject::try_from_string(line) {
        Ok(o) if valid_row(&o) => rows.push_object(o),
        _ => invalid += 1,
    }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("count", rows.len() as i64);
o.put_int("invalid", invalid);
o.put_boolean("seed", seed);
o.put_array("rows", rows);
o.put_string("path", &path.display().to_string());
o

}
