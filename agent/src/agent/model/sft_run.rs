use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "dataset", "base", "rank", "steps"] {
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
        let arg_1: String = o.get_string("dataset");
        let arg_2: String = o.get_string("base");
        let arg_3: i64 = o.get_int("rank");
        let arg_4: i64 = o.get_int("steps");
        sft_run(arg_0, arg_1, arg_2, arg_3, arg_4)
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

pub fn sft_run(name: String, dataset: String, base: String, rank: i64, steps: i64) -> DataObject {
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

// agent-model-sft_run - SFT joins the loop (spectrum S8; gate design
// in docs/spectrum-s8.md, written before this code). Validates the
// dataset (sft/persona kind) and base against the registry, then
// hands the run to the service: bounded masked-conversation delta,
// three-instrument gate (subject gain, forgetting guard, agreement
// non-regression), accept -> an sft-* ring candidate that resume
// never auto-merges - agent-model-sft_promote puts it on the fast
// lane for its soak, and from there the shipped user gate rules.
let name_t = name.trim().to_lowercase();
let ok_name = !name_t.is_empty()
    && name_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("name must be lowercase [a-z0-9-_] (got '{}')", name));
}
let dataset_t = dataset.trim().to_lowercase();
let store = DataStore::new();
let datasets = runtime_list(&store, "datasets");
let drec = match find_named(&datasets, &dataset_t) {
    Some(m) => m,
    None => { return err(format!("dataset '{}' is not registered", dataset_t)); }
};
let kind = if drec.has("kind") { drec.get_string("kind") } else { String::new() };
if kind != "sft" && kind != "persona" {
    return err(format!("dataset '{}' is kind '{}' - SFT trains on conversation datasets (sft or persona)", dataset_t, kind));
}
let base_t = base.trim().to_string();
if base_t != "pointer" {
    let models = runtime_list(&store, "models");
    if find_named(&models, &base_t).is_none() {
        return err(format!("base '{}' is neither 'pointer' nor a registered model", base_t));
    }
}
if rank <= 0 || steps <= 0 {
    return err("rank and steps must be > 0".to_string());
}
let url = format!("http://127.0.0.1:{}/sft_run",
                  prop("MODEL_SERVICE_PORT", "8077"));
let mut body = DataObject::new();
body.put_string("name", &name_t);
body.put_string("dataset", &dataset_t);
body.put_string("base", &base_t);
body.put_int("rank", rank);
body.put_int("steps", steps);
let reply = match ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(10000))
    .build()
    .post(&url)
    .send_string(&body.to_string()) {
    Ok(r) => r.into_string().unwrap_or_default(),
    Err(e) => {
        if e.to_string().contains("409") {
            return err("the service is busy (stub mode, or the time-share is borrowed) - check agent-model-service_status".to_string());
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
    return err(format!("sft_run refused: {}", msg));
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("name", &name_t);
o.put_boolean("started", true);
o.put_string("watch", "service_status carries /status.sft; the report lands in agent-model-experiments; an accepted candidate appears as an sft-* checkpoint for sft_promote");
o

}
