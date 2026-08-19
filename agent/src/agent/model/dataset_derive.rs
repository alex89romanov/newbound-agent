use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
use ndata::data::Data;
use crate::agent::model::dataset_feed::dataset_feed;
use crate::agent::llm::ask_llm::ask_llm;
use crate::agent::archivist::adjudicate::adjudicate;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["name", "out_name", "transform", "limit"] {
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
        let arg_3: i64 = o.get_int("limit");
        dataset_derive(arg_0, arg_1, arg_2, arg_3)
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

pub fn dataset_derive(name: String, out_name: String, transform: String, limit: i64) -> DataObject {
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
// (spectrum S2, ruling 10). Two transforms, two addresses:
//   render_dialect - procedural, runs IN THE SERVICE so the serving
//     dialect keeps one home (render_sample is the transform).
//   distill_why - model-driven (harvest H3b), runs HERE because its
//     generator is the frontier arm behind ask_llm. Where a generator
//     EXECUTES picks the dispatch path; which MODEL answers is only
//     ever a provenance tag on the derived rows (the standing rule).
// Every derived record carries lineage: source, transform, generator,
// and the source pin hashed at derive time.
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
if !["render_dialect", "distill_why"].contains(&transform_t.as_str()) {
    return err(format!("transforms: render_dialect (procedural, runs in the service) | distill_why (model-driven, runs here behind the frontier arm) (got '{}')", transform));
}
// The spender's cap (harvest H3b, ruling 10): model-driven derivation
// spends tokens, so it runs by deliberate command with an EXPLICIT row
// cap - never unbounded, never ambient. The drive-budgeted caller
// arrives with H5. render_dialect is procedural and ignores limit.
if transform_t == "distill_why" && (limit < 1 || limit > 25) {
    return err("distill_why requires 1 <= limit <= 25 - the explicit spend cap of one deliberate run".to_string());
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

if transform_t == "distill_why" {
    // ── the model-driven branch (harvest H3b) ───────────────────────
    // For each source row (a change paired with its stated why), the
    // frontier is asked what the change teaches about how this system
    // works - with an H2 coding context in front of it, not a bare
    // row. Products: a hysteresis-guarded claim on the subject control
    // (patch rows only - commit rows have no lib.ctl home) and one QA
    // conversation row into the out stream (kind sft) through the one
    // feeder. The generator is a PROVENANCE TAG on every row - which
    // arm answered is recorded, never branched on.
    fn fnv64(s: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        h
    }
    let src_path = if src_rec.has("path") { src_rec.get_string("path") } else { String::new() };
    let src_file = std::path::Path::new(&src_path).join("data.jsonl");
    let text = match std::fs::read_to_string(&src_file) {
        Ok(t) => t,
        Err(e) => { return err(format!("cannot read {}: {}", src_file.display(), e)); }
    };
    // the pin: what was read, exactly, at derive time - every QA row
    // carries this hash plus its own row hash, so lineage survives the
    // stream moving on (the registry-bloat-free form of ruling 10's
    // snapshot pinning; a frozen copy is dataset_snapshot's job when a
    // training run needs one)
    let src_hash = format!("{:016x}", fnv64(&text));
    let rows: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();

    // the done-set: spend once per source row, across runs
    let root2 = match store.root.canonicalize().ok()
            .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
        Some(r) => r,
        None => { return err("cannot resolve the checkout root".to_string()); }
    };
    let out_dir2 = root2.join("runtime").join("agent").join("model").join("datasets").join(&out_t);
    let _ = std::fs::create_dir_all(&out_dir2);
    let done_path = out_dir2.join("distilled.json");
    let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Ok(dt) = std::fs::read_to_string(&done_path) {
        if let Ok(da) = std::panic::catch_unwind(|| DataArray::from_string(&dt)) {
            for i in 0..da.len() {
                if let Ok(s) = da.try_get_string(i) { done.insert(s); }
            }
        }
    }

    let arm = format!("{}:{}", prop("LLM", "unset"), prop("LLM_CTL", ""));
    let budget: i64 = prop("CONTEXT_DISTILL_BUDGET", "900").parse().unwrap_or(900);
    let mut qa_lines: Vec<String> = Vec::new();
    let mut distilled = 0i64;
    let mut skipped_done = 0i64;
    let mut unparseable = 0i64;
    let mut claims_deposited = 0i64;
    let mut claims_held = 0i64;

    // newest rows first - recent changes teach the current system
    for raw in rows.iter().rev() {
        if distilled >= limit { break; }
        let row_hash = format!("{:016x}", fnv64(raw));
        if done.contains(&row_hash) { skipped_done += 1; continue; }
        let row = match DataObject::try_from_string(raw) { Ok(r) => r, Err(_) => continue };
        let why0 = if row.has("why") { row.get_string("why") } else { String::new() };
        let home = if row.has("home") { row.get_string("home") } else { String::new() };
        let subject = if home.is_empty() {
            why0.chars().take(120).collect::<String>()
        } else {
            format!("{} {}", home, why0.chars().take(100).collect::<String>())
        };
        let ctx = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::agent::context::assemble::assemble("coding".to_string(), subject.clone(), budget)
        })).ok()
            .filter(|c| c.try_get_string("status").ok().as_deref() == Some("ok"))
            .map(|c| c.try_get_string("block").unwrap_or_default())
            .unwrap_or_default();
        let prompt = format!(
            "You are the agent studying its own becoming. A change was made to this system; its journal row follows.\nCHANGE: {}\nSYSTEM CONTEXT (assembled, provenance-tagged):\n{}\nAnswer with ONLY one JSON object, no fences:\n{{\"question\": \"<the natural question a developer would ask about this change>\", \"why\": \"<why it was made, 1-2 sentences>\", \"teaches\": \"<one durable, standalone claim about how this system works that the change reveals>\"}}",
            raw, ctx);
        let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ask_llm(prompt, Data::DNull)
        })).unwrap_or_else(|_| "ERROR: ask_llm panicked".to_string());
        // spent either way: the row is done unless the ARM itself errored
        if reply.starts_with("ERROR") {
            return err(format!("the frontier arm failed after {} rows: {}", distilled, reply.chars().take(200).collect::<String>()));
        }
        done.insert(row_hash.clone());
        distilled += 1;
        let parsed = reply.find('{').and_then(|s0| reply.rfind('}').map(|e0| (s0, e0)))
            .filter(|(s0, e0)| e0 > s0)
            .and_then(|(s0, e0)| DataObject::try_from_string(&reply[s0..=e0]).ok());
        let fd = match parsed { Some(f) => f, None => { unparseable += 1; continue; } };
        let q = if fd.has("question") { fd.get_string("question") } else { String::new() };
        let w = if fd.has("why") { fd.get_string("why") } else { String::new() };
        let t2 = if fd.has("teaches") { fd.get_string("teaches") } else { String::new() };
        if q.trim().is_empty() || (w.trim().is_empty() && t2.trim().is_empty()) {
            unparseable += 1; continue;
        }
        // (a) the claim, hysteresis-guarded like any adjudication -
        //     patch rows only: they name a lib.ctl subject
        if !home.is_empty() && !t2.trim().is_empty() {
            if let Some((hlib, hdom)) = home.split_once('.') {
                let mut entry = DataObject::new();
                entry.put_string("claim", t2.trim());
                if !w.trim().is_empty() { entry.put_string("detail", w.trim()); }
                entry.put_string("tags", "code-why,distilled");
                entry.put_string("confidence", "medium");
                let adj = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    adjudicate(hlib.to_string(), hdom.to_string(), entry.deep_copy(), "distill_why".to_string())
                }));
                if let Ok(a) = adj {
                    let st = if a.has("status") { a.get_string("status") } else { String::new() };
                    if st == "ok" { claims_deposited += 1; } else { claims_held += 1; }
                }
            }
        }
        // (b) the QA conversation row, provenance riding every row
        let mut conv = DataObject::new();
        let mut msgs = DataArray::new();
        let mut mu = DataObject::new();
        mu.put_string("role", "user");
        mu.put_string("content", q.trim());
        msgs.push_object(mu);
        let mut ma = DataObject::new();
        ma.put_string("role", "assistant");
        let answer = if w.trim().is_empty() { t2.trim().to_string() }
            else if t2.trim().is_empty() { w.trim().to_string() }
            else { format!("{} {}", w.trim(), t2.trim()) };
        ma.put_string("content", &answer);
        msgs.push_object(ma);
        conv.put_array("messages", msgs);
        let mut prov = DataObject::new();
        prov.put_string("generator", &arm);
        prov.put_string("source", &name_t);
        prov.put_string("src_hash", &src_hash);
        prov.put_string("row", &row_hash);
        prov.put_int("t", time());
        conv.put_object("provenance", prov);
        qa_lines.push(conv.to_string().replace('\n', " "));
    }

    // persist the done-set, feed the bank
    let mut da = DataArray::new();
    for h in &done { da.push_string(h); }
    let _ = std::fs::write(&done_path, da.to_string());
    let mut appended = 0i64;
    if !qa_lines.is_empty() {
        let fed = dataset_feed(out_t.clone(), "sft".to_string(), qa_lines.join("
"),
            format!("derived:distill_why:{}", name_t), "dataset_derive".to_string(), 10);
        if fed.has("appended") { appended = fed.get_int("appended"); }
    }
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("transform", "distill_why");
    o.put_string("from", &name_t);
    o.put_string("derived", &out_t);
    o.put_string("generator", &arm);
    o.put_int("distilled", distilled);
    o.put_int("skipped_done", skipped_done);
    o.put_int("unparseable", unparseable);
    o.put_int("claims_deposited", claims_deposited);
    o.put_int("claims_held", claims_held);
    o.put_int("qa_appended", appended);
    o.put_string("src_pin", &src_hash);
    return o;
}

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
