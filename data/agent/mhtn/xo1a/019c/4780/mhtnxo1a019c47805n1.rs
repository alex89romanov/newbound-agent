// agent-msg-put - the message store's only writer (harvest H1).
// Messages split in two, deliberately (docs/harvest-cycle.md H1):
//   - a CONTENT record, id = "mc" + FNV-1a-128 of the text, holding
//     the words alone. Identical text written twice writes once, which
//     is the whole point: a conversation's turns ride every subsequent
//     request, and a system prompt rides every call of its life.
//   - an OCCURRENCE record, id = "mo" + a minted unique id, holding
//     when/who/where and pointing at the content. Two people saying
//     "ok" are two occurrences of one content record.
// Content-addressing is a choice the store permits, not a property it
// has: set_data shards by whatever id it is handed, so the id IS the
// hash here by construction (owner, 2026-08-19).
// Both records live in the runtime library - user JSON, never git
// (the user-data standing rule). The append-only index beside them is
// order, not truth: lose it and every message is still a record.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn fnv128(s: &str) -> String {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    let prime: u128 = 0x0000000001000000000000000000013b;
    for b in s.as_bytes() {
        h ^= *b as u128;
        h = h.wrapping_mul(prime);
    }
    format!("{:032x}", h)
}
fn rec(id: &str, data: DataObject) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("id", id);
    o.put_object("data", data);
    o.put_string("username", "system");
    o.put_int("time", time());
    o.put_array("readers", DataArray::new());
    o.put_array("writers", DataArray::new());
    o
}

let role_t = role.trim().to_lowercase();
if role_t.is_empty() {
    return err("role is required (user|assistant|system|speaker)".to_string());
}
let venue_t = venue.trim().to_lowercase();
if venue_t.is_empty() {
    return err("venue is required (chat|room|escalation|...) - it is how consumers filter without knowing which sensor produced a message".to_string());
}
if content.is_empty() {
    return err("content is required - an empty message is not an event".to_string());
}
let store = DataStore::new();

// content: written once, ever
let cid = format!("mc{}", fnv128(&content));
let deduped = store.exists("runtime", &cid);
if !deduped {
    let mut cd = DataObject::new();
    cd.put_string("text", &content);
    store.set_data("runtime", &cid, rec(&cid, cd));
}

// occurrence: always new
let now = time();
let oid = format!("mo{}", unique_session_id());
let mut od = DataObject::new();
od.put_int("t", now);
od.put_string("role", &role_t);
od.put_string("venue", &venue_t);
od.put_string("content_id", &cid);
od.put_string("entity", entity.trim());
od.put_string("provenance", provenance.trim());
store.set_data("runtime", &oid, rec(&oid, od));

// the index: append-only order, under the runtime crate (user files)
let mut indexed = false;
if let Some(root) = store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    let dir = root.join("runtime").join("agent").join("msg");
    if std::fs::create_dir_all(&dir).is_ok() {
        use std::io::Write;
        let mut row = DataObject::new();
        row.put_string("id", &oid);
        row.put_int("t", now);
        row.put_string("role", &role_t);
        row.put_string("venue", &venue_t);
        row.put_string("content_id", &cid);
        row.put_string("entity", entity.trim());
        if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true).append(true).open(dir.join("index.jsonl")) {
            indexed = writeln!(f, "{}", row.to_string().replace('\n', " ")).is_ok();
        }
    }
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("id", &oid);
o.put_string("content_id", &cid);
o.put_boolean("deduped", deduped);
o.put_boolean("indexed", indexed);
o.put_int("t", now);
o
