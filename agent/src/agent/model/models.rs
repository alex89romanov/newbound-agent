use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        models()
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

pub fn models() -> DataObject {
// agent-model-models - list the registered models with provenance
// (spectrum S1). Read-only; the registry records live in the runtime
// library (ruling 1) and this is their window for humans and sessions -
// the service reads registry.json instead.
let store = DataStore::new();
let mut list = DataArray::new();
if store.exists("runtime", "models") {
    let d = store.get_data("runtime", "models").get_object("data");
    if d.has("list") { list = d.get_array("list"); }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("count", list.len() as i64);
o.put_array("models", list);
o

}
