use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "source", "kind", "format", "holdout_every", "mode"] {
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
        let arg_1: String = o.get_string("source");
        let arg_2: String = o.get_string("kind");
        let arg_3: String = o.get_string("format");
        let arg_4: i64 = o.get_int("holdout_every");
        let arg_5: String = o.get_string("mode");
        dataset_add(arg_0, arg_1, arg_2, arg_3, arg_4, arg_5)
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

pub fn dataset_add(name: String, source: String, kind: String, format: String, holdout_every: i64, mode: String) -> DataObject {
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

// agent-model-dataset_add - the dataset door (spectrum S2, rulings 1,
// 9, 10): register a local corpus in place. Records land in the
// runtime library's datasets record; the bytes stay where they are;
// holdout_every is a READ-TIME policy (every Nth row reserved, never
// trained - the persona split pattern), so the user's files are never
// rewritten. Hub datasets come through this same door later.
let name_t = name.trim().to_lowercase();
let ok_name = !name_t.is_empty()
    && !["fresh", "replay", "standard", "stub"].contains(&name_t.as_str())
    && name_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("name must be lowercase [a-z0-9-_], non-empty, and not a built-in pool name (got '{}')", name));
}
let kind_t = kind.trim().to_string();
if !["cpt", "sft", "eval", "persona", "anchor"].contains(&kind_t.as_str()) {
    return err(format!("kind must be cpt|sft|eval|persona|anchor (got '{}')", kind));
}
let mode_t = mode.trim().to_string();
if !["stream", "snapshot"].contains(&mode_t.as_str()) {
    return err(format!("mode must be stream or snapshot (got '{}')", mode));
}
let mut format_t = format.trim().to_string();
if !["auto", "jsonl", "txt", "parquet"].contains(&format_t.as_str()) {
    return err(format!("format must be auto|jsonl|txt|parquet (got '{}')", format));
}
if holdout_every < 0 {
    return err("holdout_every must be >= 0 (0 = no held-out split)".to_string());
}
let src = match std::path::Path::new(source.trim()).canonicalize() {
    Ok(p) => p,
    Err(_) => { return err(format!("source must exist (got '{}')", source)); }
};
if format_t == "auto" {
    format_t = String::new();
    for f in ["jsonl", "txt", "parquet"] {
        if !collect_files(&src, &format!(".{}", f)).is_empty() {
            format_t = f.to_string();
            break;
        }
    }
    if format_t.is_empty() {
        return err(format!("no .jsonl/.txt/.parquet files under {}", src.display()));
    }
}
let files = collect_files(&src, &format!(".{}", format_t));
if files.is_empty() {
    return err(format!("no .{} files under {}", format_t, src.display()));
}
let rows = if format_t == "parquet" { -1 } else { count_rows(&files) };
let hash = if mode_t == "stream" { "rolling".to_string() } else { hash_files(&files) };

let store = DataStore::new();
let mut rec = ds_record(&store);
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
if find_ds(&list, &name_t).is_some() {
    return err(format!("dataset '{}' is already registered", name_t));
}
let mut m = DataObject::new();
m.put_string("name", &name_t);
m.put_string("kind", &kind_t);
m.put_string("format", &format_t);
m.put_string("path", &src.display().to_string());
m.put_int("rows", rows);
m.put_string("hash", &hash);
m.put_int("holdout_every", holdout_every);
m.put_string("mode", &mode_t);
m.put_string("lineage", "added");
m.put_string("provenance", source.trim());
m.put_int("at", time());
list.push_object(m);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "datasets", rec);

// a model waiting on this anchor gets its status flipped (ruling 2)
let mut anchored: Vec<String> = Vec::new();
if kind_t == "anchor" && store.exists("runtime", "models") {
    let mut mrec = store.get_data("runtime", "models");
    let md = mrec.get_object("data");
    if md.has("list") {
        let mlist = md.get_array("list");
        for i in 0..mlist.len() {
            if let Ok(mut mm) = mlist.try_get_object(i) {
                if mm.has("anchor")
                    && (mm.get_string("anchor") == name_t
                        || (mm.get_string("anchor") == "mint"
                            && mm.has("name")
                            && format!("{}-anchor", mm.get_string("name")) == name_t)) {
                    mm.put_string("anchor", &name_t);
                    mm.put_string("anchor_status", "named");
                    anchored.push(mm.get_string("name"));
                }
            }
        }
    }
    mrec.put_int("time", time());
    store.set_data("runtime", "models", mrec);
}

let reg_path = match render_registry(&store) {
    Ok(p) => p,
    Err(e) => { return err(e); }
};
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("name", &name_t);
o.put_string("kind", &kind_t);
o.put_string("format", &format_t);
o.put_int("rows", rows);
o.put_string("hash", &hash);
o.put_string("mode", &mode_t);
o.put_int("files", files.len() as i64);
if !anchored.is_empty() {
    let mut a = DataArray::new();
    for n in &anchored { a.push_string(n); }
    o.put_array("anchored_models", a);
}
o.put_string("registry", &reg_path);
o

}
