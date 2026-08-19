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

// agent-model-adapter_derive - the killer feature's door (spectrum S4,
// charter): a purpose-built adapter derived from any registered
// dataset against the serving pointer's base (or a registered model),
// gated exactly as persona is (min_gain on the subject's held-out
// loss, standard-loss guard). The service trains and gates; this
// command validates the names, blocks through the run, and records
// the result - report verbatim - in the runtime library. Deriving
// never applies: apply is its own deliberate, unit-gated act.
let name_t = name.trim().to_lowercase();
let ok_name = !name_t.is_empty()
    && name_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("name must be lowercase [a-z0-9-_] (got '{}')", name));
}
let dataset_t = dataset.trim().to_lowercase();
let base_t = base.trim().to_string();
let store = DataStore::new();
if runtime_has(&store, "adapters", &name_t) {
    return err(format!("adapter '{}' is already recorded - adapter_delete it first", name_t));
}
if !runtime_has(&store, "datasets", &dataset_t) {
    return err(format!("dataset '{}' is not registered - dataset_add it first", dataset_t));
}
if base_t != "pointer" && !runtime_has(&store, "models", &base_t) {
    return err(format!("base must be 'pointer' or a registered model (got '{}')", base_t));
}
if rank < 0 || steps < 0 {
    return err("rank and steps must be >= 0 (0 = the USER_LORA default)".to_string());
}

let url = format!("http://127.0.0.1:{}/adapter_derive",
                  prop("MODEL_SERVICE_PORT", "8077"));
let mut mb = DataObject::new();
mb.put_string("name", &name_t);
mb.put_string("dataset", &dataset_t);
mb.put_string("base", &base_t);
mb.put_string("targets", targets.trim());
mb.put_int("rank", rank);
mb.put_int("steps", steps);
let reply = match ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(600000))
    .build()
    .post(&url)
    .send_string(&mb.to_string()) {
    Ok(r) => r.into_string().unwrap_or_default(),
    Err(ureq::Error::Status(_, r)) => r.into_string().unwrap_or_default(),
    Err(_) => {
        return err("the service derives adapters - bootstrap it first".to_string());
    }
};
let rd = match DataObject::try_from_string(&reply) {
    Ok(d) => d,
    Err(_) => { return err(format!("service reply was not JSON: {}", reply)); }
};
if !rd.has("status") || rd.get_string("status") != "ok" {
    let msg = if rd.has("msg") { rd.get_string("msg") } else { reply.clone() };
    let mut o = err(format!("derivation did not pass: {}", msg));
    o.put_object("report", rd);
    return o;
}

// the record: the service's report, verbatim, plus bookkeeping
let root = match store.root.canonicalize() {
    Ok(r) => match r.parent() {
        Some(p) => p.to_path_buf(),
        None => { return err("store root has no parent".to_string()); }
    },
    Err(e) => { return err(format!("store root: {}", e)); }
};
let blob = root.join("runtime").join("agent").join("model")
    .join("adapters").join(format!("{}.pt", name_t));
let mut rec = adapters_record(&store);
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let mut m = DataObject::new();
m.put_string("name", &name_t);
m.put_string("dataset", &dataset_t);
m.put_string("base", &base_t);
m.put_string("path", &blob.display().to_string());
m.put_boolean("applied", false);
m.put_object("report", rd.clone());
m.put_int("at", time());
list.push_object(m);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "adapters", rec);

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("name", &name_t);
o.put_object("report", rd);
o
