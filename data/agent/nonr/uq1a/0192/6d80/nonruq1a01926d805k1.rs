// agent-model-recipes - the registered recipes (spectrum S6).
let store = DataStore::new();
let mut list = DataArray::new();
if store.exists("runtime", "recipes") {
    let d = store.get_data("runtime", "recipes").get_object("data");
    if d.has("list") { list = d.get_array("list"); }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("count", list.len() as i64);
o.put_array("recipes", list);
o
