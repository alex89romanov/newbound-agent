// The archivist strip's read (docs/agent-app.md): how many turns await
// the next consolidate sweep. Read-only.
let store = DataStore::new();
let queued = if store.exists("runtime", "archivist_queue") {
    let d = store.get_data("runtime", "archivist_queue").get_object("data");
    if d.has("turns") { d.get_array("turns").len() as i64 } else { 0 }
} else { 0 };
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("queued", queued);
o
