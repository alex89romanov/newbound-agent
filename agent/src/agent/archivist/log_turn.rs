use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
use ndata::dataarray::DataArray;

pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["venue", "ask", "reply", "tools", "author"] {
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
        let arg_1: String = o.get_string("ask");
        let arg_2: String = o.get_string("reply");
        let arg_3: String = o.get_string("tools");
        let arg_4: String = o.get_string("author");
        log_turn(arg_0, arg_1, arg_2, arg_3, arg_4)
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

pub fn log_turn(venue: String, ask: String, reply: String, tools: String, author: String) -> DataObject {
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

}
