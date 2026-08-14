use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
use crate::agent::archivist::remember::remember;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["path"] {
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
        let arg_0: String = o.get_string("path");
        bootstrap(arg_0)
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

pub fn bootstrap(path: String) -> DataObject {
// bootstrap (docs/one-memory-cycle.md A3): populate the brain from a primer
// produced by seed_export. Idempotent - remember's exact-claim dedupe skips
// what is already known; missing domain controls are created through
// dev.code (journaled). Entries whose validation fails (e.g. a source
// pointer into a library this instance does not have) are counted, not
// fatal: a fresh brain with most of its primer beats no brain at all.
let s = match std::fs::read_to_string(&path) {
    Ok(s) => s,
    Err(e) => {
        let mut o = DataObject::new();
        o.put_string("status", "err");
        o.put_string("msg", &format!("could not read {}: {}", path, e));
        return o;
    }
};
let seed = match DataObject::try_from_string(&s) {
    Ok(x) => x,
    Err(_) => {
        let mut o = DataObject::new();
        o.put_string("status", "err");
        o.put_string("msg", "seed file is not valid JSON");
        return o;
    }
};
if !seed.has("domains") {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", "seed file has no domains array");
    return o;
}
let store = DataStore::new();
let doms = seed.get_array("domains");
let mut added: i64 = 0;
let mut skipped: i64 = 0;
let mut failed: i64 = 0;
let mut created: i64 = 0;
for di in 0..doms.len() {
    let d = match doms.try_get_object(di) { Ok(x) => x, Err(_) => continue };
    if !d.has("name") { continue; }
    let name = d.get_string("name");
    let mut exists = false;
    if store.exists("kb", "controls") {
        let list = store.get_data("kb", "controls").get_object("data").get_array("list");
        for ci in 0..list.len() {
            let item = list.get_object(ci);
            if item.has("name") && item.get_string("name") == name { exists = true; break; }
        }
    }
    if !exists {
        let ac = Command::lookup("dev", "code", "add_control");
        let mut args = DataObject::new();
        args.put_string("lib", "kb");
        args.put_string("ctl", &name);
        if ac.execute(args).is_err() { failed += 1; continue; }
        created += 1;
        let sm = Command::lookup("dev", "code", "set_control_meta");
        let mut args = DataObject::new();
        args.put_string("lib", "kb");
        args.put_string("ctl", &name);
        args.put_string("desc", &(if d.has("desc") { d.get_string("desc") } else { String::new() }));
        args.put_string("groups", "");
        let _ = sm.execute(args);
    }
    if !d.has("entries") { continue; }
    let entries = d.get_array("entries");
    for ei in 0..entries.len() {
        let e = match entries.try_get_object(ei) { Ok(x) => x, Err(_) => continue };
        let res = remember("kb".to_string(), name.clone(), e.deep_copy(), "bootstrap".to_string());
        let status = if res.has("status") { res.get_string("status") } else { String::new() };
        let msg = if res.has("msg") { res.get_string("msg") } else { String::new() };
        if status == "ok" { added += 1; }
        else if msg.contains("exact claim already exists") { skipped += 1; }
        else { failed += 1; }
    }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("added", added);
o.put_int("skipped", skipped);
o.put_int("failed", failed);
o.put_int("created_domains", created);
o

}
