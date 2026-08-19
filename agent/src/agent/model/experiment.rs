use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "control", "variant", "budget_steps"] {
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
        let arg_1: String = o.get_string("control");
        let arg_2: String = o.get_string("variant");
        let arg_3: i64 = o.get_int("budget_steps");
        experiment(arg_0, arg_1, arg_2, arg_3)
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

pub fn experiment(name: String, control: String, variant: String, budget_steps: i64) -> DataObject {
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

// agent-model-experiment - the bench run (spectrum S6, ruling 7).
// Resolves the control (and variant) recipes from the store, diffs
// their bricks for the one-brick discipline - WARNS when more than
// one moved, records honestly, never refuses - and hands the resolved
// recipes to the service, which pins eval material at start, borrows
// the trainer's time-share (candidate steps pause, serving never
// does), runs the arms, and appends the report to experiments.jsonl.
let name_t = name.trim().to_lowercase();
if name_t.is_empty() {
    return err("name the experiment".to_string());
}
let store = DataStore::new();
let list = runtime_list(&store, "recipes");
let control_t = control.trim().to_lowercase();
let ctl_rec = match find_named(&list, &control_t) {
    Some(m) => m,
    None => { return err(format!("control recipe '{}' is not registered", control_t)); }
};
let variant_t = variant.trim().to_lowercase();
let mut var_rec: Option<DataObject> = None;
if !variant_t.is_empty() {
    var_rec = match find_named(&list, &variant_t) {
        Some(m) => Some(m),
        None => { return err(format!("variant recipe '{}' is not registered", variant_t)); }
    };
}
if budget_steps <= 0 {
    return err("budget_steps must be > 0".to_string());
}
// the one-brick diff: which axes moved between control and variant
let mut bricks = DataArray::new();
let mut warning = String::new();
let mut one_brick = true;
if let Some(vr) = &var_rec {
    for key in ["base", "mix", "posture", "lr", "evals"] {
        let a = if ctl_rec.has(key) { ctl_rec.get_string(key) } else { String::new() };
        let b = if vr.has(key) { vr.get_string(key) } else { String::new() };
        if a != b { bricks.push_string(key); }
    }
    let ca = if ctl_rec.has("steps") { ctl_rec.get_int("steps") } else { 0 };
    let cb = if vr.has("steps") { vr.get_int("steps") } else { 0 };
    if ca != cb { bricks.push_string("steps"); }
    let n = bricks.len();
    one_brick = n == 1;
    if n == 0 {
        warning = "control and variant are IDENTICAL - this measures run-to-run noise, which is honest but probably not what you meant".to_string();
    } else if n > 1 {
        let mut moved: Vec<String> = Vec::new();
        for i in 0..bricks.len() {
            moved.push(bricks.get_string(i));
        }
        warning = format!(
            "{} bricks moved ({}) - attribution will be confounded; the report records all of them honestly",
            n, moved.join(", "));
    }
}
let url = format!("http://127.0.0.1:{}/experiment",
                  prop("MODEL_SERVICE_PORT", "8077"));
let mut body = DataObject::new();
body.put_string("name", &name_t);
body.put_object("control", ctl_rec);
if let Some(vr) = var_rec {
    body.put_object("variant", vr);
}
body.put_int("budget_steps", budget_steps);
body.put_array("bricks_changed", bricks);
body.put_boolean("one_brick", one_brick);
let reply = match ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(10000))
    .build()
    .post(&url)
    .send_string(&body.to_string()) {
    Ok(r) => r.into_string().unwrap_or_default(),
    Err(e) => {
        let es = e.to_string();
        if es.contains("409") {
            return err("the service is busy (stub mode, or an experiment already running) - check agent-model-experiments".to_string());
        }
        return err("the service is not answering - bootstrap it first".to_string());
    }
};
let rd = match DataObject::try_from_string(&reply) {
    Ok(d) => d,
    Err(_) => { return err(format!("service reply was not JSON: {}", reply)); }
};
if !rd.has("started") {
    let msg = if rd.has("msg") { rd.get_string("msg") } else { reply };
    return err(format!("experiment refused: {}", msg));
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("name", &name_t);
o.put_boolean("started", true);
o.put_boolean("one_brick", one_brick);
if !warning.is_empty() { o.put_string("warning", &warning); }
o.put_string("watch", "agent-model-experiments reports progress and the finished report");
o

}
