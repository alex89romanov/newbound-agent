use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "on"] {
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
        let arg_1: bool = o.get_boolean("on");
        adapter_apply(arg_0, arg_1)
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

pub fn adapter_apply(name: String, on: bool) -> DataObject {
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

// agent-model-adapter_apply / the stack door (spectrum S4, ruling 3):
// the service rebuilds the user scorer from a fresh base + persona +
// every member and gates the COMBINATION - per-member subject probes
// with the full stack applied, one standard-loss guard. Pass -> the
// stack serves; fail -> serving untouched and the numbers name the
// member that broke.
let name_t = name.trim().to_lowercase();
let store = DataStore::new();
if !runtime_has(&store, "adapters", &name_t) {
    return err(format!("adapter '{}' is not recorded - adapter_derive it first", name_t));
}
let url = format!("http://127.0.0.1:{}/{}",
                  prop("MODEL_SERVICE_PORT", "8077"),
                  if on { "adapter_apply" } else { "adapter_unapply" });
let mut mb = DataObject::new();
mb.put_string("name", &name_t);
let reply = match ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(600000))
    .build()
    .post(&url)
    .send_string(&mb.to_string()) {
    Ok(r) => r.into_string().unwrap_or_default(),
    Err(ureq::Error::Status(_, r)) => r.into_string().unwrap_or_default(),
    Err(_) => {
        return err("the service owns the stack - bootstrap it first".to_string());
    }
};
let rd = match DataObject::try_from_string(&reply) {
    Ok(d) => d,
    Err(_) => { return err(format!("service reply was not JSON: {}", reply)); }
};
let ok = rd.has("status") && rd.get_string("status") == "ok";
if ok {
    let mut rec = adapters_record(&store);
    let mut d = rec.get_object("data");
    if d.has("list") {
        let list = d.get_array("list");
        let idx = find_in(&list, &name_t);
        if idx >= 0 {
            if let Ok(mut m) = list.try_get_object(idx as usize) {
                m.put_boolean("applied", on);
            }
        }
        d.put_array("list", list);
    }
    rec.put_object("data", d);
    rec.put_int("time", time());
    store.set_data("runtime", "adapters", rec);
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("name", &name_t);
    o.put_boolean("applied", on);
    o.put_object("report", rd);
    return o;
}
let msg = if rd.has("msg") { rd.get_string("msg") } else { reply.clone() };
let mut o = err(format!("stack change refused: {}", msg));
o.put_object("report", rd);
o

}
