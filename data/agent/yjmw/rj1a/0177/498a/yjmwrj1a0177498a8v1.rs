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
