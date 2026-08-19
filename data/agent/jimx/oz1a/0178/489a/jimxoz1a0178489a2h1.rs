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

// agent-model-dataset_snapshot - pin a frozen, hashed cut of a stream
// (or any dataset) as a new record with lineage (spectrum S2, ruling
// 10: the bench and SFT runs pin; the live loop rides streams). Bytes
// are COPIED into the managed datasets dir; the source stays a stream.
fn copy_tree(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    if src.is_file() {
        if let Some(par) = dst.parent() {
            std::fs::create_dir_all(par).map_err(|e| e.to_string())?;
        }
        std::fs::copy(src, dst).map_err(|e| e.to_string())?;
        return Ok(());
    }
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    if let Ok(rd) = std::fs::read_dir(src) {
        for e in rd.flatten() {
            let p = e.path();
            let t = dst.join(e.file_name());
            if p.is_dir() {
                copy_tree(&p, &t)?;
            } else {
                std::fs::copy(&p, &t).map_err(|e2| e2.to_string())?;
            }
        }
    }
    Ok(())
}

let name_t = name.trim().to_lowercase();
let snap_t = snapshot_name.trim().to_lowercase();
let ok_name = !snap_t.is_empty()
    && !["fresh", "replay", "standard", "stub"].contains(&snap_t.as_str())
    && snap_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("snapshot_name must be lowercase [a-z0-9-_] and not a built-in pool name (got '{}')", snapshot_name));
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
if find_ds(&list, &snap_t).is_some() {
    return err(format!("'{}' is already registered", snap_t));
}
let fmt = if src_rec.has("format") { src_rec.get_string("format") } else { "jsonl".to_string() };
if fmt == "parquet" {
    return err("parquet snapshots are not supported on this branch".to_string());
}
let src_path = if src_rec.has("path") { src_rec.get_string("path") } else { String::new() };
let holdout = if src_rec.has("holdout_every") { src_rec.get_int("holdout_every") } else { 0 };
let kind = if src_rec.has("kind") { src_rec.get_string("kind") } else { "cpt".to_string() };

let root = match store.root.canonicalize() {
    Ok(r) => match r.parent() {
        Some(p) => p.to_path_buf(),
        None => { return err("store root has no parent".to_string()); }
    },
    Err(e) => { return err(format!("store root: {}", e)); }
};
let dest = root.join("runtime").join("agent").join("model").join("datasets").join(&snap_t);
if dest.exists() {
    return err(format!("{} already exists on disk", dest.display()));
}
let sp = std::path::Path::new(&src_path);
let target = if sp.is_file() {
    dest.join(sp.file_name().map(|n| n.to_os_string()).unwrap_or_default())
} else {
    dest.clone()
};
if let Err(e) = copy_tree(sp, &target) {
    let _ = std::fs::remove_dir_all(&dest);
    return err(format!("copy failed: {}", e));
}
let files = collect_files(&dest, &format!(".{}", fmt));
let rows = count_rows(&files);
let hash = hash_files(&files);

let mut m = DataObject::new();
m.put_string("name", &snap_t);
m.put_string("kind", &kind);
m.put_string("format", &fmt);
m.put_string("path", &dest.display().to_string());
m.put_int("rows", rows);
m.put_string("hash", &hash);
m.put_int("holdout_every", holdout);
m.put_string("mode", "snapshot");
m.put_string("lineage", &format!("snapshot:{}@{}rows", name_t, rows));
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
o.put_string("snapshot", &snap_t);
o.put_string("of", &name_t);
o.put_int("rows", rows);
o.put_string("hash", &hash);
o.put_string("path", &dest.display().to_string());
o.put_string("registry", &reg_path);
o
