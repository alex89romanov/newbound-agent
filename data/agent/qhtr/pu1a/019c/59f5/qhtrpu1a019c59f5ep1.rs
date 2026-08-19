// agent-msg-get - resolve one message id to its words (harvest H1).
// Takes either half of the split: an occurrence id ("mo...") returns
// when/who/where joined with the text its content record holds; a
// content id ("mc...") returns the text alone. Claims and training
// rows cite occurrence ids, so this is the resolver every later
// provenance trail ends at.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
let id_t = id.trim().to_string();
if id_t.is_empty() { return err("id is required".to_string()); }
let store = DataStore::new();
if !store.exists("runtime", &id_t) {
    return err(format!("no message '{}'", id_t));
}
let d = store.get_data("runtime", &id_t).get_object("data");
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("id", &id_t);
if d.has("text") {
    // a content record: the words, with no occurrence around them
    o.put_string("kind", "content");
    o.put_string("content_id", &id_t);
    o.put_string("content", &d.get_string("text"));
    return o;
}
o.put_string("kind", "occurrence");
for k in ["role", "venue", "entity", "provenance", "content_id"] {
    o.put_string(k, &(if d.has(k) { d.get_string(k) } else { String::new() }));
}
o.put_int("t", if d.has("t") { d.get_int("t") } else { 0 });
let cid = if d.has("content_id") { d.get_string("content_id") } else { String::new() };
if !cid.is_empty() && store.exists("runtime", &cid) {
    let cd = store.get_data("runtime", &cid).get_object("data");
    o.put_string("content", &(if cd.has("text") { cd.get_string("text") } else { String::new() }));
} else {
    // the occurrence outlived its content record - say so rather than
    // returning an empty string that reads like an empty message
    o.put_string("content", "");
    o.put_boolean("content_missing", true);
}
o
