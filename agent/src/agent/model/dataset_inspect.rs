use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "peek", "verify"] {
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
        let arg_1: i64 = o.get_int("peek");
        let arg_2: bool = o.get_boolean("verify");
        dataset_inspect(arg_0, arg_1, arg_2)
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

pub fn dataset_inspect(name: String, peek: i64, verify: bool) -> DataObject {
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

// agent-model-dataset_inspect - peek rows and (optionally) re-verify
// hash + row count against the record (spectrum S2). On a snapshot a
// hash mismatch is a tamper alarm; on a stream drift is the point and
// the recorded hash stays "rolling".
let name_t = name.trim().to_lowercase();
let store = DataStore::new();
if !store.exists("runtime", "datasets") {
    return err("no datasets registered".to_string());
}
let d = store.get_data("runtime", "datasets").get_object("data");
let list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let m = match find_ds(&list, &name_t) {
    Some(m) => m,
    None => { return err(format!("dataset '{}' is not registered", name_t)); }
};
let fmt = if m.has("format") { m.get_string("format") } else { "jsonl".to_string() };
let path = if m.has("path") { m.get_string("path") } else { String::new() };
let files = collect_files(std::path::Path::new(&path), &format!(".{}", fmt));

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_object("dataset", m.clone());
o.put_int("files", files.len() as i64);

if peek > 0 && !files.is_empty() && fmt != "parquet" {
    use std::io::BufRead;
    let mut rows = DataArray::new();
    let cap = std::cmp::min(peek, 20) as usize;
    if let Ok(f) = std::fs::File::open(&files[0]) {
        for ln in std::io::BufReader::new(f).lines().flatten() {
            let t = ln.trim().to_string();
            if t.is_empty() { continue; }
            rows.push_string(&t.chars().take(200).collect::<String>());
            if rows.len() >= cap { break; }
        }
    }
    o.put_array("peek", rows);
}
if verify {
    let rows_now = if fmt == "parquet" { -1 } else { count_rows(&files) };
    let hash_now = hash_files(&files);
    let recorded = if m.has("hash") { m.get_string("hash") } else { String::new() };
    o.put_int("rows_now", rows_now);
    o.put_string("hash_now", &hash_now);
    if recorded == "rolling" {
        o.put_string("hash_match", "rolling (a stream - drift is the point)");
    } else if recorded == hash_now {
        o.put_string("hash_match", "ok");
    } else {
        o.put_string("hash_match", "MISMATCH - the bytes changed since registration");
    }
}
o

}
