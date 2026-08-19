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
fn runtime_list(store: &DataStore, id: &str) -> DataArray {
    if store.exists("runtime", id) {
        let d = store.get_data("runtime", id).get_object("data");
        if d.has("list") { return d.get_array("list"); }
    }
    DataArray::new()
}
fn find_named(list: &DataArray, name: &str) -> Option<DataObject> {
    for i in 0..list.len() {
        if let Ok(m) = list.try_get_object(i) {
            if m.has("name") && m.get_string("name") == name {
                return Some(m);
            }
        }
    }
    None
}
fn save_runtime_list(store: &DataStore, id: &str, list: DataArray) {
    let mut rec = if store.exists("runtime", id) {
        store.get_data("runtime", id)
    } else {
        let mut r = DataObject::new();
        r.put_string("id", id);
        r.put_string("username", "system");
        r.put_array("readers", DataArray::new());
        r.put_array("writers", DataArray::new());
        r.put_object("data", DataObject::new());
        r
    };
    let mut d = rec.get_object("data");
    d.put_array("list", list);
    rec.put_object("data", d);
    rec.put_int("time", time());
    store.set_data("runtime", id, rec);
}
fn validate_recipe(store: &DataStore, base: &str, mix: &str) -> Option<String> {
    if base != "pointer" {
        let models = runtime_list(store, "models");
        if find_named(&models, base).is_none() {
            return Some(format!("base '{}' is neither 'pointer' nor a registered model", base));
        }
    }
    let datasets = runtime_list(store, "datasets");
    for part in mix.split(',') {
        if let Some((k, _v)) = part.split_once('=') {
            let k = k.trim();
            if !k.is_empty() && find_named(&datasets, k).is_none() {
                return Some(format!("mix names '{}', which is not a registered dataset", k));
            }
        }
    }
    None
}

// agent-model-recipe_remove (spectrum S6).
let name_t = name.trim().to_lowercase();
let store = DataStore::new();
let mut list = runtime_list(&store, "recipes");
let mut idx: i64 = -1;
for i in 0..list.len() {
    if let Ok(m) = list.try_get_object(i) {
        if m.has("name") && m.get_string("name") == name_t {
            idx = i as i64;
            break;
        }
    }
}
if idx < 0 {
    return err(format!("recipe '{}' is not registered", name_t));
}
list.remove_property(idx as usize);
save_runtime_list(&store, "recipes", list);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("removed", &name_t);
o
