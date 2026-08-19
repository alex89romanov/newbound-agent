use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "base", "mix", "posture", "steps", "lr", "evals", "notes"] {
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
        let arg_0: String = o.get_string("name");
        let arg_1: String = o.get_string("base");
        let arg_2: String = o.get_string("mix");
        let arg_3: String = o.get_string("posture");
        let arg_4: i64 = o.get_int("steps");
        let arg_5: String = o.get_string("lr");
        let arg_6: String = o.get_string("evals");
        let arg_7: String = o.get_string("notes");
        recipe_author(arg_0, arg_1, arg_2, arg_3, arg_4, arg_5, arg_6, arg_7)
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

pub fn recipe_author(name: String, base: String, mix: String, posture: String, steps: i64, lr: String, evals: String, notes: String) -> DataObject {
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

// agent-model-recipe_author - a recipe snaps the bricks together
// (spectrum S6): base x datasets-with-weights x posture x budget x
// lr x evals. A named, declarative record in the runtime library -
// pure declaration, no local paths, the lab's unit of composition.
let name_t = name.trim().to_lowercase();
let ok_name = !name_t.is_empty()
    && name_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("name must be lowercase [a-z0-9-_] (got '{}')", name));
}
let base_t = base.trim().to_string();
let mix_t = mix.trim().to_string();
if mix_t.is_empty() {
    return err("mix must name at least one registered dataset (name=weight,...)".to_string());
}
let posture_t = posture.trim().to_string();
if !["auto", "full", "lora", "frozen"].contains(&posture_t.as_str()) {
    return err(format!("posture must be auto|full|lora|frozen (got '{}')", posture));
}
if steps <= 0 {
    return err("steps (the budget) must be > 0".to_string());
}
let store = DataStore::new();
if let Some(e) = validate_recipe(&store, &base_t, &mix_t) {
    return err(e);
}
let mut list = runtime_list(&store, "recipes");
if find_named(&list, &name_t).is_some() {
    return err(format!("recipe '{}' already exists - recipe_clone or recipe_remove it", name_t));
}
let mut m = DataObject::new();
m.put_string("name", &name_t);
m.put_string("base", &base_t);
m.put_string("mix", &mix_t);
m.put_string("posture", &posture_t);
m.put_int("steps", steps);
m.put_string("lr", lr.trim());
m.put_string("evals", evals.trim());
m.put_string("notes", notes.trim());
m.put_int("at", time());
list.push_object(m.clone());
save_runtime_list(&store, "recipes", list);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_object("recipe", m);
o

}
