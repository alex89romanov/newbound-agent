use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
use flowlang::flowlang::system::unique_session_id::unique_session_id;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["role", "venue", "content", "entity", "provenance", "id"] {
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
        let arg_0: String = o.get_string("role");
        let arg_1: String = o.get_string("venue");
        let arg_2: String = o.get_string("content");
        let arg_3: String = o.get_string("entity");
        let arg_4: String = o.get_string("provenance");
        let arg_5: String = o.get_string("id");
        put(arg_0, arg_1, arg_2, arg_3, arg_4, arg_5)
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

pub fn put(role: String, venue: String, content: String, entity: String, provenance: String, id: String) -> DataObject {
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

// occurrence: minted unless the caller supplies one. A supplied id
// makes put idempotent - capture derives ids by chaining hashes over
// the conversation prefix, so a turn re-sent on every later request
// records exactly once (the occurrence-level half of the dedup; the
// content record is the text-level half).
let now = time();
let id_t = id.trim().to_string();
if !id_t.is_empty() {
    if !id_t.starts_with("mo") {
        return err("a supplied id must start with 'mo' (occurrence namespace)".to_string());
    }
    if store.exists("runtime", &id_t) {
        let mut o = DataObject::new();
        o.put_string("status", "ok");
        o.put_string("id", &id_t);
        o.put_string("content_id", &cid);
        o.put_boolean("deduped", deduped);
        o.put_boolean("occurrence_deduped", true);
        o.put_boolean("indexed", false);
        o.put_int("t", now);
        return o;
    }
}
let oid = if id_t.is_empty() { format!("mo{}", unique_session_id()) } else { id_t };
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
o.put_boolean("occurrence_deduped", false);
o.put_boolean("indexed", indexed);
o.put_int("t", now);
o

}
