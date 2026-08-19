use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "source", "backend", "anchor"] {
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
        let arg_2: String = o.get_string("backend");
        let arg_3: String = o.get_string("anchor");
        import(arg_0, arg_1, arg_2, arg_3)
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

pub fn import(name: String, source: String, backend: String, anchor: String) -> DataObject {
// agent-model-import - the deliberate door (spectrum charter S1,
// ruling 1: records in the runtime library, bytes referenced in place;
// ruling 9: acquisition only through this door). Local paths only on
// this branch - the hub form (hf:org/repo@revision) errors clearly
// until the fetch stage lands. anchor: mint | none | <dataset name>
// (ruling 2: imports mint a self-sampled anchor by default; the mint
// itself wires to the service endpoint later in S1 - until then it
// records as mint_pending).
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
fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}
fn walk(dir: &std::path::Path, base: &std::path::Path, h: &mut u64, n: &mut i64) {
    let mut entries: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() { entries.push(e.path()); }
    }
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, base, h, n);
        } else if let Ok(md) = p.metadata() {
            let rel = p.strip_prefix(base).map(|r| r.display().to_string()).unwrap_or_default();
            fnv(h, rel.as_bytes());
            fnv(h, md.len().to_string().as_bytes());
            *n += 1;
        }
    }
}

let name_t = name.trim().to_lowercase();
let ok_name = !name_t.is_empty() && name_t != "stub"
    && name_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("name must be lowercase [a-z0-9-_], non-empty, not 'stub' (got '{}')", name));
}
let backend_t = backend.trim().to_string();
if backend_t != "nanochat" && backend_t != "hf" {
    return err(format!("backend must be nanochat or hf (got '{}')", backend));
}
let anchor_t = anchor.trim().to_string();
if anchor_t.is_empty() {
    return err("anchor must be mint, none, or a dataset name".to_string());
}
let source_t = source.trim().to_string();
if source_t.starts_with("hf:") {
    return err("hub fetch lands later on this branch - pass a local path for now".to_string());
}
let src = match std::path::Path::new(&source_t).canonicalize() {
    Ok(p) => p,
    Err(_) => { return err(format!("source must be an existing directory (got '{}')", source_t)); }
};
if !src.is_dir() {
    return err(format!("source must be a directory (got '{}')", source_t));
}

// backend-shape validation - refuse at the door, not at serving time
if backend_t == "nanochat" {
    let has_ckpt = ["base_checkpoints", "chatsft_checkpoints", "chatrl_checkpoints"]
        .iter().any(|d| src.join(d).is_dir()) || src.join("train_done").exists();
    if !src.join("tokenizer").is_dir() || !has_ckpt {
        return err(format!("{} is not a nanochat base dir (needs tokenizer/ plus base_checkpoints/ or a sibling phase dir, or train_done)", src.display()));
    }
} else if !src.join("config.json").is_file() {
    return err(format!("{} is not an HF model dir (no config.json)", src.display()));
}

// footprint - cheap and best-effort, from the metadata files only
let mut context_len = 0i64;
let mut dtype = String::new();
if backend_t == "hf" {
    if let Ok(cfg) = std::fs::read_to_string(src.join("config.json")) {
        if let Ok(c) = DataObject::try_from_string(&cfg) {
            if let Ok(m) = c.try_get_int("max_position_embeddings") { context_len = m; }
            if let Ok(d) = c.try_get_string("torch_dtype") { dtype = d; }
        }
    }
} else {
    dtype = "bf16".to_string();
    'outer: for ckdir in ["chatrl_checkpoints", "chatsft_checkpoints", "base_checkpoints"] {
        let p = src.join(ckdir);
        if !p.is_dir() { continue; }
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten() {
                let sub = e.path();
                if !sub.is_dir() { continue; }
                if let Ok(rd2) = std::fs::read_dir(&sub) {
                    for f in rd2.flatten() {
                        let fname = f.file_name().into_string().unwrap_or_default();
                        if fname.starts_with("meta_") && fname.ends_with(".json") {
                            if let Ok(meta) = std::fs::read_to_string(f.path()) {
                                if let Ok(m) = DataObject::try_from_string(&meta) {
                                    if let Ok(mc) = m.try_get_object("model_config") {
                                        if let Ok(sl) = mc.try_get_int("sequence_len") { context_len = sl; }
                                    }
                                }
                            }
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
}

// fingerprint: FNV-1a over sorted relative paths + sizes. Provenance,
// not a content hash - weights are GBs; names+sizes catch swaps and
// truncation, which is what the door needs.
let mut h: u64 = 0xcbf29ce484222325;
let mut files = 0i64;
walk(&src, &src, &mut h, &mut files);
let fingerprint = format!("{:016x}", h);

// the record - runtime library, the salience_log idiom, list-shaped
// like a controls index
let store = DataStore::new();
let mut rec = if store.exists("runtime", "models") {
    store.get_data("runtime", "models")
} else {
    let mut r = DataObject::new();
    r.put_string("id", "models");
    r.put_string("username", "system");
    r.put_array("readers", DataArray::new());
    r.put_array("writers", DataArray::new());
    r.put_object("data", DataObject::new());
    r
};
let mut d = rec.get_object("data");
let mut list = if d.has("list") { d.get_array("list") } else { DataArray::new() };
for i in 0..list.len() {
    if let Ok(m) = list.try_get_object(i) {
        if m.has("name") && m.get_string("name") == name_t {
            return err(format!("model '{}' is already registered - model_remove it first", name_t));
        }
    }
}
let anchor_status = if anchor_t == "mint" { "mint_pending" }
    else if anchor_t == "none" { "none" } else { "named" };
let mut m = DataObject::new();
m.put_string("name", &name_t);
m.put_string("backend", &backend_t);
m.put_string("source", &source_t);
m.put_string("revision", "");
m.put_string("path", &src.display().to_string());
m.put_string("dtype", &dtype);
m.put_int("context_len", context_len);
m.put_int("files", files);
m.put_string("fingerprint", &fingerprint);
m.put_string("anchor", &anchor_t);
m.put_string("anchor_status", anchor_status);
m.put_string("lineage", "imported");
m.put_int("at", time());
list.push_object(m);
d.put_array("list", list);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "models", rec);

// render registry.json - the service's window onto the registry (the
// service never reads the store); mtime is the pickup signal
let root = match store.root.canonicalize() {
    Ok(r) => match r.parent() {
        Some(p) => p.to_path_buf(),
        None => { return err("store root has no parent".to_string()); }
    },
    Err(e) => { return err(format!("store root: {}", e)); }
};
let modeldir = root.join("runtime").join("agent").join("model");
let _ = std::fs::create_dir_all(&modeldir);
let d2 = store.get_data("runtime", "models").get_object("data");
let mut reg = DataObject::new();
reg.put_array("models", if d2.has("list") { d2.get_array("list") } else { DataArray::new() });
reg.put_array("datasets", if store.exists("runtime", "datasets") {
    let dd = store.get_data("runtime", "datasets").get_object("data");
    if dd.has("list") { dd.get_array("list") } else { DataArray::new() }
} else { DataArray::new() });
reg.put_int("rendered_at", time());
if let Err(e) = std::fs::write(modeldir.join("registry.json"), reg.to_string()) {
    return err(format!("registry render failed: {}", e));
}

// anchor minting (ruling 2): ask the service to self-sample; the
// record stays mint_pending until the minted set registers (S2). A
// deferred mint is recorded, never fatal - the door stays fast.
let mut mint = String::from("not_requested");
if anchor_t == "mint" {
    {
        // S3: both backends mint through the seam - the service's
        // generate_text is the one door. The serving env must be able
        // to import the record's backend; a mismatch records an
        // honest error in /status.mint.
        let url = format!("http://127.0.0.1:{}/mint_anchor",
                          prop("MODEL_SERVICE_PORT", "8077"));
        let mut mb = DataObject::new();
        mb.put_string("path", &src.display().to_string());
        mb.put_string("name", &format!("{}-anchor", name_t));
        mb.put_string("backend", &backend_t);
        mb.put_int("n", 200);
        mint = match ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(3000))
            .build()
            .post(&url)
            .send_string(&mb.to_string()) {
            Ok(r) => match r.into_string() {
                Ok(t) => match DataObject::try_from_string(&t) {
                    Ok(d) if d.has("started") => "requested".to_string(),
                    Ok(d) if d.has("msg") =>
                        format!("deferred: {}", d.get_string("msg")),
                    _ => "deferred: unexpected service reply".to_string(),
                },
                Err(_) => "deferred: unreadable service reply".to_string(),
            },
            Err(e) => {
                let es = e.to_string();
                if es.contains("409") || es.contains("Status(409") {
                    "deferred: stub mode or a mint already running - re-run import once a real checkpoint serves".to_string()
                } else {
                    "deferred: service not answering - bootstrap it and re-run import, or mint later".to_string()
                }
            }
        };
    }
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("name", &name_t);
o.put_string("backend", &backend_t);
o.put_string("path", &src.display().to_string());
o.put_string("dtype", &dtype);
o.put_int("context_len", context_len);
o.put_int("files", files);
o.put_string("fingerprint", &fingerprint);
o.put_string("anchor_status", anchor_status);
o.put_string("mint", &mint);
o.put_string("registry", &modeldir.join("registry.json").display().to_string());
o

}
