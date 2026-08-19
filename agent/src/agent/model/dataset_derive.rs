use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "out_name", "transform"] {
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
        let arg_1: String = o.get_string("out_name");
        let arg_2: String = o.get_string("transform");
        dataset_derive(arg_0, arg_1, arg_2)
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

pub fn dataset_derive(name: String, out_name: String, transform: String) -> DataObject {
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn collect_files(root: &std::path::Path, ext: &str) -> Vec<std::path::PathBuf> {
    fn walkdir(d: &std::path::Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            let mut es: Vec<std::path::PathBuf> = rd.flatten().map(|e| e.path()).collect();
            es.sort();
            for p in es {
                if p.is_dir() {
                    walkdir(&p, ext, out);
                } else if p.display().to_string().ends_with(ext) {
                    out.push(p);
                }
            }
        }
    }
    let mut out = Vec::new();
    if root.is_file() {
        if root.display().to_string().ends_with(ext) { out.push(root.to_path_buf()); }
        return out;
    }
    walkdir(root, ext, &mut out);
    out
}
fn hash_files(files: &Vec<std::path::PathBuf>) -> String {
    use std::io::Read;
    let mut h: u64 = 0xcbf29ce484222325;
    for p in files {
        if let Ok(mut f) = std::fs::File::open(p) {
            let mut buf = [0u8; 1048576];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        for b in &buf[..n] {
                            h ^= *b as u64;
                            h = h.wrapping_mul(0x100000001b3);
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
    format!("{:016x}", h)
}
fn count_rows(files: &Vec<std::path::PathBuf>) -> i64 {
    use std::io::BufRead;
    let mut n = 0i64;
    for p in files {
        if let Ok(f) = std::fs::File::open(p) {
            for ln in std::io::BufReader::new(f).lines().flatten() {
                if !ln.trim().is_empty() { n += 1; }
            }
        }
    }
    n
}
fn ds_record(store: &DataStore) -> DataObject {
    if store.exists("runtime", "datasets") {
        store.get_data("runtime", "datasets")
    } else {
        let mut r = DataObject::new();
        r.put_string("id", "datasets");
        r.put_string("username", "system");
        r.put_array("readers", DataArray::new());
        r.put_array("writers", DataArray::new());
        r.put_object("data", DataObject::new());
        r
    }
}
fn find_ds(list: &DataArray, name: &str) -> Option<DataObject> {
    for i in 0..list.len() {
        if let Ok(m) = list.try_get_object(i) {
            if m.has("name") && m.get_string("name") == name {
                return Some(m);
            }
        }
    }
    None
}
fn render_registry(store: &DataStore) -> Result<String, String> {
    let root = store.root.canonicalize().map_err(|e| format!("store root: {}", e))?;
    let root = match root.parent() {
        Some(p) => p.to_path_buf(),
        None => { return Err("store root has no parent".to_string()); }
    };
    let modeldir = root.join("runtime").join("agent").join("model");
    let _ = std::fs::create_dir_all(&modeldir);
    let mut reg = DataObject::new();
    reg.put_array("models", if store.exists("runtime", "models") {
        let d = store.get_data("runtime", "models").get_object("data");
        if d.has("list") { d.get_array("list") } else { DataArray::new() }
    } else { DataArray::new() });
    reg.put_array("datasets", if store.exists("runtime", "datasets") {
        let d = store.get_data("runtime", "datasets").get_object("data");
        if d.has("list") { d.get_array("list") } else { DataArray::new() }
    } else { DataArray::new() });
    reg.put_int("rendered_at", time());
    let p = modeldir.join("registry.json");
    std::fs::write(&p, reg.to_string()).map_err(|e| format!("registry render failed: {}", e))?;
    Ok(p.display().to_string())
}

// agent-model-dataset_derive - synthetic data is a dataset operation
// (spectrum S2, ruling 10). This branch ships the procedural
// transform only: render_dialect, which runs IN THE SERVICE so the
// serving dialect keeps one home (render_sample is the transform).
// The derived record carries lineage: source, transform, provenance.
// Model-driven generation arrives with its governed spender later.
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

let name_t = name.trim().to_lowercase();
let out_t = out_name.trim().to_lowercase();
let transform_t = transform.trim().to_string();
if transform_t != "render_dialect" {
    return err(format!("this branch ships one transform: render_dialect (got '{}')", transform));
}
let ok_name = !out_t.is_empty()
    && !["fresh", "replay", "standard", "stub"].contains(&out_t.as_str())
    && out_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("out_name must be lowercase [a-z0-9-_] and not a built-in pool name (got '{}')", out_name));
}
let store = DataStore::new();
if !store.exists("runtime", "datasets") {
    return err("no datasets registered".to_string());
}
let mut rec = store.get_data("runtime", "datasets");
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let src_rec = match find_ds(&list, &name_t) {
    Some(m) => m,
    None => { return err(format!("dataset '{}' is not registered", name_t)); }
};
if find_ds(&list, &out_t).is_some() {
    return err(format!("'{}' is already registered", out_t));
}
let src_path = if src_rec.has("path") { src_rec.get_string("path") } else { String::new() };
let holdout = if src_rec.has("holdout_every") { src_rec.get_int("holdout_every") } else { 0 };

let url = format!("http://127.0.0.1:{}/derive", prop("MODEL_SERVICE_PORT", "8077"));
let mut mb = DataObject::new();
mb.put_string("source_path", &src_path);
mb.put_string("name", &out_t);
mb.put_string("transform", &transform_t);
let reply = match ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(120000))
    .build()
    .post(&url)
    .send_string(&mb.to_string()) {
    Ok(r) => match r.into_string() {
        Ok(t) => t,
        Err(e) => { return err(format!("unreadable service reply: {}", e)); }
    },
    Err(_) => {
        return err("the service renders transforms (one dialect home) - bootstrap it first; stub mode serves /derive too".to_string());
    }
};
let rd = match DataObject::try_from_string(&reply) {
    Ok(d2) => d2,
    Err(_) => { return err(format!("service reply was not JSON: {}", reply)); }
};
if !rd.has("rows") {
    let msg = if rd.has("msg") { rd.get_string("msg") } else { reply };
    return err(format!("derive failed: {}", msg));
}
// the record's path is computed HERE, absolutely, from the store root
// - the service may run with a relative --data-dir and its reply path
// then only resolves from its own cwd
let root = match store.root.canonicalize() {
    Ok(r) => match r.parent() {
        Some(p) => p.to_path_buf(),
        None => { return err("store root has no parent".to_string()); }
    },
    Err(e) => { return err(format!("store root: {}", e)); }
};
let out_dir = root.join("runtime").join("agent").join("model")
    .join("datasets").join(&out_t);
let files = collect_files(&out_dir, ".txt");
if files.is_empty() {
    return err(format!("derive reported rows but nothing at {}", out_dir.display()));
}
let rows = count_rows(&files);
let hash = hash_files(&files);

let mut m = DataObject::new();
m.put_string("name", &out_t);
m.put_string("kind", "cpt");
m.put_string("format", "txt");
m.put_string("path", &out_dir.display().to_string());
m.put_int("rows", rows);
m.put_string("hash", &hash);
m.put_int("holdout_every", holdout);
m.put_string("mode", "snapshot");
m.put_string("lineage", &format!("derived:render_dialect:{}", name_t));
m.put_string("provenance", &src_path);
m.put_int("at", time());
list.push_object(m);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "datasets", rec);
let reg_path = match render_registry(&store) {
    Ok(p) => p,
    Err(e) => { return err(e); }
};
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("derived", &out_t);
o.put_string("from", &name_t);
o.put_string("transform", &transform_t);
o.put_int("rows", rows);
o.put_string("hash", &hash);
o.put_string("registry", &reg_path);
o

}
