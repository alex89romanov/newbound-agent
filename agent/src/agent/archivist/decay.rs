use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["lib", "domain", "claim", "author"] {
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
        let arg_1: String = o.get_string("domain");
        let arg_2: String = o.get_string("claim");
        let arg_3: String = o.get_string("author");
        decay(arg_0, arg_1, arg_2, arg_3)
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

pub fn decay(lib: String, domain: String, claim: String, author: String) -> DataObject {
// decay (understandingloop.md Phase 4): the ONE autonomous write
// initiative permits - an evidence-based confidence step-down on a claim
// whose source referent drifted. Same ladder as adjudicate-contradicts,
// but the evidence is the drift itself, so each distinct drift state
// decays AT MOST ONE step: the observed referent hash is recorded
// (decayed_hash) and a repeat call against the same drift is a no-op.
// At the low floor only the acknowledgment is written, once - the claim
// then waits in the review queue. Convergent by construction: bounded
// writes per drift state, zero on repeat.
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
fn content_hash(s: &str) -> String {
    // FNV-1a over the \r-normalized source - MUST stay in sync with
    // remember/recall/adjudicate/read_control_facet/patch_control_facet.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}
fn lookup_ctl_id(lib: &str, name: &str) -> String {
    let store = DataStore::new();
    if !store.exists(lib, "controls") { return String::new(); }
    let rec = store.get_data(lib, "controls").get_object("data");
    if !rec.has("list") { return String::new(); }
    for c in rec.get_array("list").objects() {
        let c = c.object();
        if c.has("name") && c.get_string("name") == name {
            return c.get_string("id");
        }
    }
    String::new()
}

fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn ensure_key(data_obj: &mut DataObject, name: &str) {
    let mut keys = if data_obj.has("attachmentkeynames") {
        data_obj.get_array("attachmentkeynames")
    } else {
        DataArray::new()
    };
    let mut hasit = false;
    for i in 0..keys.len() {
        if let Data::DString(s) = keys.get_property(i) {
            if s == name { hasit = true; break; }
        }
    }
    if !hasit { keys.push_string(name); }
    data_obj.put_array("attachmentkeynames", keys);
}
fn journal(store: &DataStore, lib: &str, ctlid: &str, facet: &str, old: &str, new: &str, author: &str, label: &str) -> String {
    let jid = format!("{}_patches", ctlid);
    let mut jrec;
    let mut jdata;
    let mut jlist;
    if store.exists(lib, &jid) {
        jrec = store.get_data(lib, &jid);
        jdata = jrec.get_object("data");
        jlist = if jdata.has("list") { jdata.get_array("list") } else { DataArray::new() };
    } else {
        jrec = DataObject::new();
        jrec.put_string("id", &jid);
        jrec.put_string("username", "system");
        jrec.put_array("readers", DataArray::new());
        jrec.put_array("writers", DataArray::new());
        jdata = DataObject::new();
        jlist = DataArray::new();
    }
    let patch_id = format!("p{}", jlist.len() + 1);
    let mut jentry = DataObject::new();
    jentry.put_string("patch_id", &patch_id);
    jentry.put_string("author", author);
    jentry.put_string("facet", facet);
    jentry.put_string("cmd", "");
    jentry.put_string("old", old);
    jentry.put_string("new", new);
    jentry.put_int("time", time());
    let mut label = label.to_string();
    if label.chars().count() > 72 {
        label = label.chars().take(69).collect::<String>() + "...";
    }
    jentry.put_string("label", &label);
    jlist.push_object(jentry);
    jdata.put_array("list", jlist);
    jrec.put_object("data", jdata);
    jrec.put_int("time", time());
    store.set_data(lib, &jid, jrec);
    patch_id
}

let ctlid = lookup_ctl_id(&lib, &domain);
let store = DataStore::new();
if !store.exists(&lib, &ctlid) {
    return err(format!("Domain control '{}' not found in library '{}'", domain, lib));
}
let mut record = store.get_data(&lib, &ctlid);
let mut data_obj = record.get_object("data");
let old_source = if data_obj.has("memory") {
    data_obj.get_string("memory").replace("\r", "")
} else {
    return err(format!("{}.{} has no memory facet", lib, domain));
};
let arr = match DataObject::try_from_string(&format!("{{\"a\":{}}}", old_source)) {
    Ok(w) => match w.try_get_array("a") {
        Ok(a) => a,
        Err(_) => return err(format!("{}.{}'s memory facet is not a JSON ARRAY", lib, domain)),
    },
    Err(_) => return err(format!("{}.{}'s memory facet is not valid JSON", lib, domain)),
};
let mut found: i64 = -1;
for i in 0..arr.len() {
    let e = match arr.try_get_object(i) { Ok(e) => e, Err(_) => continue };
    if e.has("claim") && !e.has("superseded") && e.get_string("claim").trim() == claim.trim() {
        found = i as i64;
        break;
    }
}
if found < 0 {
    return err(format!("no live entry with that exact claim in {}.{}", lib, domain));
}
let mut e = arr.get_object(found as usize);
let src = match e.try_get_object("source") {
    Ok(s) if s.has("lib") && s.has("ctl") && s.has("facet") && s.has("hash") => s,
    _ => return err("claim has no checkable source pointer - decay needs drift evidence".to_string()),
};
let slib = src.get_string("lib");
let sctl = src.get_string("ctl");
let sfacet = src.get_string("facet");
let sid = lookup_ctl_id(&slib, &sctl);
let mut current = "missing".to_string();
if !sid.is_empty() && store.exists(&slib, &sid) {
    let sdata = store.get_data(&slib, &sid).get_object("data");
    if sdata.has(&sfacet) {
        current = content_hash(&sdata.get_string(&sfacet).replace("\r", ""));
    }
}
if current == src.get_string("hash") {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("action", "fresh");
    return o;
}
if e.has("decayed_hash") && e.get_string("decayed_hash") == current {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_string("action", "already_acknowledged");
    return o;
}
let now = time();
let c = if e.has("confidence") { e.get_string("confidence") } else { "medium".to_string() };
let mut action = String::new();
let mut after = String::new();
if c == "low" {
    e.put_string("decayed_hash", &current);
    e.put_int("adjudicated", now);
    action = "at_floor".to_string();
} else {
    after = (if c == "high" { "medium" } else { "low" }).to_string();
    e.put_string("confidence", &after);
    e.put_string("decayed_hash", &current);
    e.put_int("adjudicated", now);
    action = "decayed".to_string();
}
// trace on the domain's traces facet, capped like adjudicate's
let mut trace = DataObject::new();
trace.put_string("input_claim", &claim);
trace.put_string("relation", "drift");
trace.put_string("action", &action);
trace.put_string("before", &c);
if !after.is_empty() { trace.put_string("after", &after); }
trace.put_string("reasoning", &format!("source {}.{}:{} drifted; hysteresis step", slib, sctl, sfacet));
trace.put_int("time", now);
trace.put_string("author", &author);
let old_traces = if data_obj.has("traces") { data_obj.get_string("traces").replace("\r", "") } else { String::new() };
let mut traces_arr = if old_traces.trim().is_empty() {
    DataArray::new()
} else {
    match DataObject::try_from_string(&format!("{{\"a\":{}}}", old_traces)) {
        Ok(w) => w.try_get_array("a").unwrap_or_else(|_| DataArray::new()),
        Err(_) => DataArray::new(),
    }
};
traces_arr.push_object(trace);
while traces_arr.len() > 200 { traces_arr.remove_property(0); }
let new_source = val(Data::DArray(arr.data_ref), 0) + "\n";
data_obj.put_string("memory", &new_source);
ensure_key(&mut data_obj, "memory");
data_obj.put_string("traces", &(val(Data::DArray(traces_arr.data_ref), 0) + "\n"));
ensure_key(&mut data_obj, "traces");
record.put_object("data", data_obj);
record.put_int("time", now);
store.set_data(&lib, &ctlid, record);
let patch_id = journal(&store, &lib, &ctlid, "memory", &old_source, &new_source, &author,
    &format!("decay: {} {}", action, claim));
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("action", &action);
o.put_string("before", &c);
if !after.is_empty() { o.put_string("after", &after); }
o.put_string("patch_id", &patch_id);
o

}
