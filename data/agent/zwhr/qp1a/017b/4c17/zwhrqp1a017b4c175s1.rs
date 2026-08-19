// agent-model-adapters - the recorded adapters with their gate
// reports and applied state (spectrum S4). Read-only.
let store = DataStore::new();
let mut list = DataArray::new();
if store.exists("runtime", "adapters") {
    let d = store.get_data("runtime", "adapters").get_object("data");
    if d.has("list") { list = d.get_array("list"); }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("count", list.len() as i64);
o.put_array("adapters", list);
o
