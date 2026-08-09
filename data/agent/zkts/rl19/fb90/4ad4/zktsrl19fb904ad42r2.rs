// The archivist's intake (docs/memory.md): every completed turn queues in
// the runtime library - instance-local and gitignored, so raw transcripts
// never ride canon. The consolidate sweep (this control's timer) drains
// it. Bounded at 200 turns, oldest dropped.
let _author = author;
let store = DataStore::new();
let mut rec;
let mut d;
let mut turns;
if store.exists("runtime", "archivist_queue") {
    rec = store.get_data("runtime", "archivist_queue");
    d = rec.get_object("data");
    turns = if d.has("turns") { d.get_array("turns") } else { DataArray::new() };
} else {
    rec = DataObject::new();
    rec.put_string("id", "archivist_queue");
    rec.put_string("username", "system");
    rec.put_array("readers", DataArray::new());
    rec.put_array("writers", DataArray::new());
    d = DataObject::new();
    turns = DataArray::new();
}
let mut t = DataObject::new();
t.put_string("venue", &venue);
t.put_string("ask", &ask.chars().take(4000).collect::<String>());
t.put_string("reply", &reply.chars().take(4000).collect::<String>());
t.put_string("tools", &tools.chars().take(400).collect::<String>());
t.put_int("time", time());
turns.push_object(t);
while turns.len() > 200 {
    turns.remove_property(0);
}
let queued = turns.len() as i64;
d.put_array("turns", turns);
rec.put_object("data", d);
rec.put_int("time", time());
store.set_data("runtime", "archivist_queue", rec);
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("queued", queued);
o
