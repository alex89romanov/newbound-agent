use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["venue", "limit"] {
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
        let arg_0: String = o.get_string("venue");
        let arg_1: i64 = o.get_int("limit");
        recent(arg_0, arg_1)
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

pub fn recent(venue: String, limit: i64) -> DataObject {
// agent-msg-recent - the last N messages, newest last (harvest H1).
// Reads the append-only index under the runtime crate rather than the
// store: records answer "what is this id", the index answers "what
// happened lately", and neither pretends to do the other's job. The
// index is order, not truth - a message missing from it is still a
// record, and rebuilding it is a repair operation, not a read path.
// Filtering is BY VENUE, never by sensor: that is what lets the H2
// assembler take room speech without the agent library knowing which
// sensor produced it (the sensor-layering rule).
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
if limit <= 0 { return err("limit must be > 0".to_string()); }
let cap = if limit > 500 { 500 } else { limit } as usize;
let venue_t = venue.trim().to_lowercase();
let store = DataStore::new();
let path = match store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    Some(root) => root.join("runtime").join("agent").join("msg").join("index.jsonl"),
    None => { return err("cannot resolve the runtime folder".to_string()); }
};
let text = std::fs::read_to_string(&path).unwrap_or_default();
let mut hits: Vec<DataObject> = Vec::new();
for ln in text.lines() {
    let ln = ln.trim();
    if ln.is_empty() { continue; }
    let row = match DataObject::try_from_string(ln) { Ok(r) => r, Err(_) => continue };
    if !venue_t.is_empty() {
        let v = if row.has("venue") { row.get_string("venue") } else { String::new() };
        if v != venue_t { continue; }
    }
    hits.push(row);
}
let start = if hits.len() > cap { hits.len() - cap } else { 0 };
let mut out = DataArray::new();
for row in hits[start..].iter() {
    let mut r = row.deep_copy();
    let cid = if r.has("content_id") { r.get_string("content_id") } else { String::new() };
    if !cid.is_empty() && store.exists("runtime", &cid) {
        let cd = store.get_data("runtime", &cid).get_object("data");
        r.put_string("content", &(if cd.has("text") { cd.get_string("text") } else { String::new() }));
    } else {
        r.put_string("content", "");
        r.put_boolean("content_missing", true);
    }
    out.push_object(r);
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("indexed", hits.len() as i64);
o.put_int("count", out.len() as i64);
o.put_array("messages", out);
o

}
