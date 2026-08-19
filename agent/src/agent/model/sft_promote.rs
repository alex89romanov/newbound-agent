use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["checkpoint"] {
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
        let arg_0: String = o.get_string("checkpoint");
        sft_promote(arg_0)
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

pub fn sft_promote(checkpoint: String) -> DataObject {
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

// agent-model-sft_promote - the deliberate act (spectrum S8): put an
// accepted sft-* ring candidate on the FAST lane for its soak. From
// there the shipped machinery rules: soak_s + verdicts + agreement
// qualify it READY, /user_promote advances the user pointer, persona
// and the adapter stack re-apply on top, rollback is one step back.
let key = checkpoint.trim().to_string();
if !key.starts_with("sft-") {
    return err(format!("'{}' is not an sft-* ring candidate (cpt-* checkpoints promote through the trainer's own gate)", key));
}
let url = format!("http://127.0.0.1:{}/promote",
                  prop("MODEL_SERVICE_PORT", "8077"));
let mut body = DataObject::new();
body.put_string("checkpoint", &key);
let reply = match ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(300000))
    .build()
    .post(&url)
    .send_string(&body.to_string()) {
    Ok(r) => r.into_string().unwrap_or_default(),
    Err(e) => {
        let es = e.to_string();
        if es.contains("404") {
            return err(format!("no ring entry '{}' - did the SFT gate accept?", key));
        }
        return err(format!("promote failed: {}", es.chars().take(200).collect::<String>()));
    }
};
let rd = match DataObject::try_from_string(&reply) {
    Ok(d) => d,
    Err(_) => { return err(format!("service reply was not JSON: {}", reply)); }
};
if !rd.has("pointer") {
    let msg = if rd.has("msg") { rd.get_string("msg") } else { reply };
    return err(format!("promote refused: {}", msg));
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("pointer", &rd.get_string("pointer"));
o.put_string("soaking", &key);
o.put_string("next", "the fast lane soaks it; the user gate qualifies READY; agent-model-user_promote advances /chat");
o

}
