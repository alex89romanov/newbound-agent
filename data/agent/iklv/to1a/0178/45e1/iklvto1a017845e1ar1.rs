// agent-model-dataset_list - the registered datasets with provenance
// (spectrum S2). Read-only window onto the runtime library's datasets
// record; the trainer reads registry.json instead.
let store = DataStore::new();
let mut list = DataArray::new();
if store.exists("runtime", "datasets") {
    let d = store.get_data("runtime", "datasets").get_object("data");
    if d.has("list") { list = d.get_array("list"); }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("count", list.len() as i64);
o.put_array("datasets", list);
o
