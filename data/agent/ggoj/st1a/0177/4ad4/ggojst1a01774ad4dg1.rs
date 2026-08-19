// agent-model-model_remove - unregister a model (spectrum S1). The
// record always goes; bytes only with purge:true AND only inside the
// managed weights dir - a model imported in place references the
// user's own files, which this command never deletes. Refuses on the
// record MODEL= currently names.
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

let name_t = name.trim().to_lowercase();
if prop("MODEL", "") == name_t {
    return err(format!("refusing: MODEL= currently names '{}' - point MODEL= elsewhere first", name_t));
}
let store = DataStore::new();
if !store.exists("runtime", "models") {
    return err("no models registered".to_string());
}
let mut rec = store.get_data("runtime", "models");
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let mut idx: i64 = -1;
let mut path = String::new();
for i in 0..list.len() {
    if let Ok(m) = list.try_get_object(i) {
        if m.has("name") && m.get_string("name") == name_t {
            idx = i as i64;
            if m.has("path") { path = m.get_string("path"); }
            break;
        }
    }
}
if idx < 0 {
    return err(format!("model '{}' is not registered", name_t));
}
list.remove_property(idx as usize);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "models", rec);

let root = match store.root.canonicalize() {
    Ok(r) => match r.parent() {
        Some(p) => p.to_path_buf(),
        None => { return err("store root has no parent".to_string()); }
    },
    Err(e) => { return err(format!("store root: {}", e)); }
};
let modeldir = root.join("runtime").join("agent").join("model");
let mut purged = false;
let mut note = String::new();
if purge {
    let weights = modeldir.join("weights");
    let p = std::path::Path::new(&path);
    if p.starts_with(&weights) && p.is_dir() {
        let _ = std::fs::remove_dir_all(p);
        purged = true;
    } else {
        note = "bytes left in place - path is outside the managed weights dir".to_string();
    }
}

// re-render registry.json (the service's pickup signal is mtime)
let _ = std::fs::create_dir_all(&modeldir);
let d2 = store.get_data("runtime", "models").get_object("data");
let mut reg = DataObject::new();
reg.put_array("models", if d2.has("list") { d2.get_array("list") } else { DataArray::new() });
reg.put_array("datasets", if store.exists("runtime", "datasets") {
    let dd = store.get_data("runtime", "datasets").get_object("data");
    if dd.has("list") { dd.get_array("list") } else { DataArray::new() }
} else { DataArray::new() });
reg.put_int("rendered_at", time());
if let Err(e) = std::fs::write(modeldir.join("registry.json"), reg.to_string()) {
    return err(format!("registry render failed: {}", e));
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("removed", &name_t);
o.put_boolean("purged", purged);
if !note.is_empty() { o.put_string("note", &note); }
o
