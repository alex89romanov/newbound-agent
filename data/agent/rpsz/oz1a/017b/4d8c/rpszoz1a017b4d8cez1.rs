fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
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
fn adapters_record(store: &DataStore) -> DataObject {
    if store.exists("runtime", "adapters") {
        store.get_data("runtime", "adapters")
    } else {
        let mut r = DataObject::new();
        r.put_string("id", "adapters");
        r.put_string("username", "system");
        r.put_array("readers", DataArray::new());
        r.put_array("writers", DataArray::new());
        r.put_object("data", DataObject::new());
        r
    }
}
fn find_in(list: &DataArray, name: &str) -> i64 {
    for i in 0..list.len() {
        if let Ok(m) = list.try_get_object(i) {
            if m.has("name") && m.get_string("name") == name {
                return i as i64;
            }
        }
    }
    -1
}
fn runtime_has(store: &DataStore, rec: &str, name: &str) -> bool {
    if !store.exists("runtime", rec) { return false; }
    let d = store.get_data("runtime", rec).get_object("data");
    if !d.has("list") { return false; }
    find_in(&d.get_array("list"), name) >= 0
}

// agent-model-adapter_delete - remove an adapter record AND its blob
// (blobs only ever live in the managed adapters dir). Refuses while
// the adapter is applied - unapply is the deliberate act that changes
// serving, delete only cleans shelves.
let name_t = name.trim().to_lowercase();
let store = DataStore::new();
if !store.exists("runtime", "adapters") {
    return err("no adapters recorded".to_string());
}
let mut rec = store.get_data("runtime", "adapters");
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let idx = find_in(&list, &name_t);
if idx < 0 {
    return err(format!("adapter '{}' is not recorded", name_t));
}
let mut path = String::new();
if let Ok(m) = list.try_get_object(idx as usize) {
    if m.has("applied") && m.get_boolean("applied") {
        return err(format!("refusing: '{}' is applied - adapter_unapply it first", name_t));
    }
    if m.has("path") { path = m.get_string("path"); }
}
list.remove_property(idx as usize);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "adapters", rec);
let mut purged = false;
if !path.is_empty() {
    let p = std::path::Path::new(&path);
    if p.is_file() {
        purged = std::fs::remove_file(p).is_ok();
    }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("removed", &name_t);
o.put_boolean("blob_deleted", purged);
o
