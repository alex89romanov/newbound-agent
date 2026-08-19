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

// agent-model-dataset_remove - unregister a dataset (spectrum S2,
// model_remove's twin). The record always goes; bytes only with
// purge:true AND only inside the managed datasets dir - added-in-place
// corpora are the user's files and are never deleted. Refuses while
// MODEL_MIX names the dataset (mix a pool out before removing it).
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
let mix = prop("MODEL_MIX", "");
for part in mix.split(',') {
    if let Some((k, _v)) = part.split_once('=') {
        if k.trim() == name_t {
            return err(format!("refusing: MODEL_MIX names '{}' - mix it out first", name_t));
        }
    }
}
let store = DataStore::new();
if !store.exists("runtime", "datasets") {
    return err("no datasets registered".to_string());
}
let mut rec = store.get_data("runtime", "datasets");
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
let mut idx: i64 = -1;
let mut path = String::new();
for i in 0..list.len() {
    if let Ok(m) = list.try_get_object(i) {
        if m.has("name") && m.get_string("name") == name_t {
            idx = i as i64;
            if m.has("path") { path = m.get_string("path"); }
            break;
        }
    }
}
if idx < 0 {
    return err(format!("dataset '{}' is not registered", name_t));
}
list.remove_property(idx as usize);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "datasets", rec);

let root = match store.root.canonicalize() {
    Ok(r) => match r.parent() {
        Some(p) => p.to_path_buf(),
        None => { return err("store root has no parent".to_string()); }
    },
    Err(e) => { return err(format!("store root: {}", e)); }
};
let managed = root.join("runtime").join("agent").join("model").join("datasets");
let mut purged = false;
let mut note = String::new();
if purge {
    let p = std::path::Path::new(&path);
    if p.starts_with(&managed) && p.is_dir() {
        let _ = std::fs::remove_dir_all(p);
        purged = true;
    } else {
        note = "bytes left in place - path is outside the managed datasets dir".to_string();
    }
}
let reg_path = match render_registry(&store) {
    Ok(p) => p,
    Err(e) => { return err(e); }
};
let _ = reg_path;
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("removed", &name_t);
o.put_boolean("purged", purged);
if !note.is_empty() { o.put_string("note", &note); }
o
