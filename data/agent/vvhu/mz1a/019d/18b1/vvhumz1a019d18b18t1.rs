// agent-model-dataset_feed - THE feed contract, as one command instead
// of a convention (harvest H1c/H6; the amended page's ruling: factor
// the feeder before a third channel copies the plumbing). A channel
// hands over JSONL lines; this command owns everything after that:
// line-hash dedup (idempotent re-feeds), the append, the stream
// record's registration and row count, and the registry.json re-render
// the trainer's window reads. curriculum_export's salience-pairs and
// memory sweeps call it, the chat sweep calls it, H3's why-harvest
// will call it - one feeder, every channel, no drift between banks.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn fnv_line(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
let name_t = name.trim().to_lowercase();
let ok_name = !name_t.is_empty()
    && !["fresh", "replay", "standard", "stub"].contains(&name_t.as_str())
    && name_t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
if !ok_name {
    return err(format!("name must be lowercase [a-z0-9-_] and not a built-in pool name (got '{}')", name));
}
let kind_t = kind.trim().to_lowercase();
if !["cpt", "sft", "eval", "persona", "anchor"].contains(&kind_t.as_str()) {
    return err(format!("kind must be cpt|sft|eval|persona|anchor (got '{}')", kind));
}
if holdout_every < 0 {
    return err("holdout_every must be >= 0 (0 = no held-out split)".to_string());
}
let store = DataStore::new();
let root = match store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    Some(r) => r,
    None => { return err("cannot resolve the runtime folder".to_string()); }
};
let modeldir = root.join("runtime").join("agent").join("model");
let dir = modeldir.join("datasets").join(&name_t);
if std::fs::create_dir_all(&dir).is_err() {
    return err(format!("cannot create {}", dir.display()));
}
let file = dir.join("data.jsonl");

// dedup against what the stream already holds - a re-fed line is a no-op
let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
if let Ok(existing) = std::fs::read_to_string(&file) {
    for ln in existing.lines() {
        if !ln.trim().is_empty() { seen.insert(fnv_line(ln)); }
    }
}
let mut fresh: Vec<String> = Vec::new();
for ln in lines.lines() {
    let ln = ln.trim();
    if ln.is_empty() { continue; }
    let flat = ln.replace('\n', " ");
    if seen.insert(fnv_line(&flat)) { fresh.push(flat); }
}
if !fresh.is_empty() {
    use std::io::Write;
    match std::fs::OpenOptions::new().create(true).append(true).open(&file) {
        Ok(mut f) => { for ln in &fresh { let _ = writeln!(f, "{}", ln); } },
        Err(e) => { return err(format!("cannot append to {}: {}", file.display(), e)); }
    }
}
let total_rows = seen.len() as i64;

// the stream registers itself on first feed; rows stay current after
let mut drec = if store.exists("runtime", "datasets") {
    store.get_data("runtime", "datasets")
} else {
    let mut r = DataObject::new();
    r.put_string("id", "datasets");
    r.put_string("username", "system");
    r.put_array("readers", DataArray::new());
    r.put_array("writers", DataArray::new());
    r.put_object("data", DataObject::new());
    r
};
let mut dd = drec.get_object("data");
let mut dlist = if dd.has("list") { dd.get_array("list") } else { DataArray::new() };
let mut found = false;
for i in 0..dlist.len() {
    if let Ok(mut m) = dlist.try_get_object(i) {
        if m.has("name") && m.get_string("name") == name_t {
            if m.has("kind") && m.get_string("kind") != kind_t {
                return err(format!("dataset '{}' is registered as kind '{}' - a channel may not re-kind a bank", name_t, m.get_string("kind")));
            }
            m.put_int("rows", total_rows);
            found = true;
            break;
        }
    }
}
if !found {
    let mut m = DataObject::new();
    m.put_string("name", &name_t);
    m.put_string("kind", &kind_t);
    m.put_string("format", "jsonl");
    m.put_string("path", &dir.display().to_string());
    m.put_int("rows", total_rows);
    m.put_string("hash", "rolling");
    m.put_int("holdout_every", holdout_every);
    m.put_string("mode", "stream");
    m.put_string("lineage", lineage.trim());
    m.put_string("provenance", provenance.trim());
    m.put_int("at", time());
    dlist.push_object(m);
}
dd.put_array("list", dlist);
drec.put_object("data", dd);
drec.put_int("time", time());
store.set_data("runtime", "datasets", drec);

// re-render registry.json - the trainer's window
let mut reg = DataObject::new();
reg.put_array("models", if store.exists("runtime", "models") {
    let d = store.get_data("runtime", "models").get_object("data");
    if d.has("list") { d.get_array("list") } else { DataArray::new() }
} else { DataArray::new() });
reg.put_array("datasets", {
    let d = store.get_data("runtime", "datasets").get_object("data");
    if d.has("list") { d.get_array("list") } else { DataArray::new() }
});
reg.put_int("rendered_at", time());
let _ = std::fs::write(modeldir.join("registry.json"), reg.to_string());

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("name", &name_t);
o.put_int("appended", fresh.len() as i64);
o.put_int("rows", total_rows);
o
