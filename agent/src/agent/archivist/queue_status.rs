use ndata::dataobject::DataObject;
use flowlang::datastore::DataStore;

pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        queue_status()
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

pub fn queue_status() -> DataObject {
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

}
