use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
use crate::agent::archivist::remember::remember;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["lib"] {
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
        let arg_0: String = o.get_string("lib");
        promote(arg_0)
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

pub fn promote(lib: String) -> DataObject {
// promote (docs/one-memory-cycle.md A3): sweep the brain for unpromoted
// claims whose `subject` extra names this library, union-merge them into
// the subject controls' shipped manuals through remember (whose exact-claim
// dedupe IS the identity merge), then stamp the brain copies promoted via a
// journaled patch_control_facet. Explicit trigger only - publish warns,
// never promotes. A claim already present in the target still gets its
// brain copy stamped: the union holds either way. A bare-library subject
// ("dev") files onto the eponymous control (dev.dev).
fn esc(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
fn val(d: Data, ind: usize) -> String {
    match d {
        Data::DString(s) => format!("\"{}\"", esc(&s)),
        Data::DInt(i) => format!("{}", i),
        Data::DFloat(f) => format!("{}", f),
        Data::DBoolean(b) => format!("{}", b),
        Data::DNull => "null".to_string(),
        Data::DObject(r) => obj(DataObject::get(r), ind),
        Data::DArray(r) => {
            let a = DataArray::get(r);
            if a.len() == 0 { return "[]".to_string(); }
            let pad = "  ".repeat(ind + 1);
            let mut out = String::from("[");
            for i in 0..a.len() {
                if i > 0 { out.push(','); }
                out.push_str(&format!("\n{}{}", pad, val(a.get_property(i), ind + 1)));
            }
            out.push_str(&format!("\n{}]", "  ".repeat(ind)));
            out
        }
        _ => "null".to_string(),
    }
}
fn obj(o: DataObject, ind: usize) -> String {
    // Canonical field order: the entry shape first, then pointer fields,
    // then anything else sorted - hash-backed ndata loses file order, so
    // a fixed order is what makes rewrites diff cleanly.
    let canon = ["claim", "detail", "tags", "source", "confidence", "time",
                 "lib", "ctl", "facet", "hash", "doc"];
    let mut keys: Vec<String> = canon.iter().filter(|k| o.has(k)).map(|s| s.to_string()).collect();
    let mut extra: Vec<String> = o.get_keys().into_iter()
        .filter(|k| !canon.contains(&k.as_str())).collect();
    extra.sort();
    keys.extend(extra);
    if keys.is_empty() { return "{}".to_string(); }
    let pad = "  ".repeat(ind + 1);
    let mut out = String::from("{");
    let mut first = true;
    for k in &keys {
        if !first { out.push(','); }
        first = false;
        out.push_str(&format!("\n{}\"{}\": {}", pad, esc(k), val(o.get_property(k), ind + 1)));
    }
    out.push_str(&format!("\n{}}}", "  ".repeat(ind)));
    out
}

let store = DataStore::new();
if !store.exists("kb", "controls") {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", "no kb library in this store - nothing to promote");
    return o;
}
let mut promoted: i64 = 0;
let mut already: i64 = 0;
let mut failures = DataArray::new();
let list = store.get_data("kb", "controls").get_object("data").get_array("list");
for ci in 0..list.len() {
    let item = list.get_object(ci);
    if !item.has("name") || !item.has("id") { continue; }
    let ctl_name = item.get_string("name");
    let ctl_id = item.get_string("id");
    if !store.exists("kb", &ctl_id) { continue; }
    let dd = store.get_data("kb", &ctl_id).get_object("data");
    if !dd.has("memory") { continue; }
    let old_source = dd.get_string("memory");
    if old_source.trim().is_empty() { continue; }
    let arr = match DataObject::try_from_string(&format!("{{\"a\":{}}}", old_source.replace("\r", ""))) {
        Ok(w) => match w.try_get_array("a") { Ok(a) => a, Err(_) => continue },
        Err(_) => continue,
    };
    let mut stamped: i64 = 0;
    for i in 0..arr.len() {
        let e = match arr.try_get_object(i) { Ok(x) => x, Err(_) => continue };
        if !e.has("claim") || !e.has("subject") || e.has("promoted") { continue; }
        let subject = e.get_string("subject");
        let (tlib, tctl) = match subject.find('.') {
            Some(p) => (subject[..p].to_string(), subject[p + 1..].to_string()),
            None => (subject.clone(), subject.clone()),
        };
        if tlib != lib { continue; }
        let mut copy = e.deep_copy();
        copy.remove_property("subject");
        let res = remember(tlib.clone(), tctl.clone(), copy, "promote".to_string());
        let status = if res.has("status") { res.get_string("status") } else { String::new() };
        let msg = if res.has("msg") { res.get_string("msg") } else { String::new() };
        if status == "ok" {
            promoted += 1;
        } else if msg.contains("exact claim already exists") {
            already += 1;
        } else {
            let mut f = DataObject::new();
            f.put_string("domain", &ctl_name);
            f.put_string("claim", &e.get_string("claim"));
            f.put_string("msg", &msg);
            failures.push_object(f);
            continue;
        }
        let mut e = e;
        e.put_int("promoted", time());
        stamped += 1;
    }
    if stamped > 0 {
        let new_source = val(Data::DArray(arr.data_ref), 0) + "\n";
        let pc = Command::lookup("dev", "code", "patch_control_facet");
        let mut args = DataObject::new();
        args.put_string("lib", "kb");
        args.put_string("ctl", &ctl_name);
        args.put_string("facet", "memory");
        args.put_string("old_snippet", &old_source);
        args.put_string("new_snippet", &new_source);
        args.put_string("base", "");
        args.put_string("label", &format!("promote: {} claim(s) -> {}", stamped, lib));
        args.put_string("author", "promote");
        args.put_string("nn_sessionid", "");
        let stamp_ok = match pc.execute(args) {
            Ok(r) => r.has("a") && r.get_object("a").has("status")
                     && r.get_object("a").get_string("status") == "ok",
            Err(_) => false,
        };
        if !stamp_ok {
            let mut f = DataObject::new();
            f.put_string("domain", &ctl_name);
            f.put_string("msg", "filed to target but stamping the brain copy failed - a re-promote will report these as already_present");
            failures.push_object(f);
        }
    }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("promoted", promoted);
o.put_int("already_present", already);
o.put_array("failures", failures);
o

}
