use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::flowlang::system::time::time;
use crate::agent::archivist::epistemic_work::epistemic_work;
use crate::agent::archivist::decay::decay;
use crate::agent::archivist::remember::remember;
use crate::agent::llm::ask_llm::ask_llm;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["limit"] {
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
        let arg_0: i64 = o.get_int("limit");
        reverify(arg_0)
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

pub fn reverify(limit: i64) -> DataObject {
// agent-archivist-reverify - the curator the memory system has waited
// for (harvest H5). Picks up to `limit` STALE claims (source referent
// drifted since the hash was stamped), re-reads each referent's
// CURRENT content, and asks the frontier: does the claim still hold?
//   confirm -> the one write this act owns: re-stamp source.hash to
//     the current referent, mark reverified, step confidence up one
//     (drift checked and survived is evidence) - the stale flag
//     clears because the hash now matches. Trace + journal ride the
//     write, so rumination trains the model too.
//   amend  -> decay the old claim (drift acknowledged) + remember the
//     corrected text with a fresh source pointer - two governed
//     writes through the standing commands.
//   retire -> decay alone; at the low floor the claim waits in the
//     review queue for the owner. Autonomy never deletes.
// An unparseable verdict skips the claim (counted); the arm erroring
// aborts with the count spent. Frontier spend rides the drive budget.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
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
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}
fn ctl_id(store: &DataStore, lib: &str, name: &str) -> String {
    if !store.exists(lib, "controls") { return String::new(); }
    let rec = store.get_data(lib, "controls").get_object("data");
    if !rec.has("list") { return String::new(); }
    for c in rec.get_array("list").objects() {
        let c = c.object();
        if c.has("name") && c.get_string("name") == name { return c.get_string("id"); }
    }
    String::new()
}
fn ensure_key(data_obj: &mut DataObject, name: &str) {
    let mut keys = if data_obj.has("attachmentkeynames") {
        data_obj.get_array("attachmentkeynames")
    } else { DataArray::new() };
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
fn conf_up(c: &str) -> &'static str {
    match c { "low" => "medium", _ => "high" }
}

if limit < 1 || limit > 10 {
    return err("limit must be 1..=10 - re-verification spends one frontier call per claim".to_string());
}
let store = DataStore::new();
let work = epistemic_work();
if !work.has("items") {
    return err("epistemic_work returned no items array".to_string());
}
let items = work.get_array("items");
let mut confirmed = 0i64;
let mut amended = 0i64;
let mut retired = 0i64;
let mut unparseable = 0i64;
let mut spent = 0i64;
for i in 0..items.len() {
    if spent >= limit { break; }
    let it = match items.try_get_object(i) { Ok(x) => x, Err(_) => continue };
    if !it.has("kind") || it.get_string("kind") != "stale" { continue; }
    let (wlib, wdom, wclaim) = (it.get_string("lib"), it.get_string("domain"), it.get_string("claim"));
    // the entry's source pointer, from the domain facet itself
    let ctlid = ctl_id(&store, &wlib, &wdom);
    if ctlid.is_empty() || !store.exists(&wlib, &ctlid) { continue; }
    let mut record = store.get_data(&wlib, &ctlid);
    let mut data_obj = record.get_object("data");
    let old_source = if data_obj.has("memory") { data_obj.get_string("memory").replace("\r", "") } else { continue };
    let mut arr = match DataObject::try_from_string(&format!("{{\"a\":{}}}", old_source)) {
        Ok(w) => match w.try_get_array("a") { Ok(a) => a, Err(_) => continue },
        Err(_) => continue,
    };
    let mut idx: i64 = -1;
    for j in 0..arr.len() {
        if let Ok(e) = arr.try_get_object(j) {
            if e.has("claim") && e.get_string("claim").trim() == wclaim.trim()
                && !e.has("superseded") { idx = j as i64; break; }
        }
    }
    if idx < 0 { continue; }
    let mut entry = arr.get_object(idx as usize);
    let src = match entry.try_get_object("source") { Ok(s) => s, Err(_) => continue };
    if !(src.has("lib") && src.has("ctl") && src.has("facet")) { continue; }
    let (slib, sctl, sfacet) = (src.get_string("lib"), src.get_string("ctl"), src.get_string("facet"));
    let scid = ctl_id(&store, &slib, &sctl);
    if scid.is_empty() || !store.exists(&slib, &scid) { continue; }
    let sdata = store.get_data(&slib, &scid).get_object("data");
    if !sdata.has(&sfacet) { continue; }
    let content = sdata.get_string(&sfacet).replace("\r", "");
    let cur_hash = content_hash(&content);
    let excerpt: String = content.chars().take(3000).collect();
    let detail = if entry.has("detail") { entry.get_string("detail") } else { String::new() };
    let prompt = format!(
        "You are the agent re-verifying one of its own memories against reality. The claim below was stamped against an older version of the referent; the referent has since changed. Decide whether the claim still holds of the CURRENT content.\nCLAIM: {}\nDETAIL: {}\nREFERENT ({}.{} facet {}, current):\n{}\nReply with ONLY one JSON object, no fences:\n{{\"verdict\": \"confirm\" | \"amend\" | \"retire\", \"claim\": \"<the corrected claim if amend, else empty>\", \"why\": \"<one sentence>\"}}",
        wclaim, detail, slib, sctl, sfacet, excerpt);
    let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ask_llm(prompt, Data::DNull)
    })).unwrap_or_else(|_| "ERROR: ask_llm panicked".to_string());
    if reply.starts_with("ERROR") {
        return err(format!("the frontier arm failed after {} claims: {}", spent,
            reply.chars().take(200).collect::<String>()));
    }
    spent += 1;
    let fd = reply.find('{').and_then(|s0| reply.rfind('}').map(|e0| (s0, e0)))
        .filter(|(s0, e0)| e0 > s0)
        .and_then(|(s0, e0)| DataObject::try_from_string(&reply[s0..=e0]).ok());
    let fd = match fd { Some(f) => f, None => { unparseable += 1; continue; } };
    let verdict = if fd.has("verdict") { fd.get_string("verdict") } else { String::new() };
    let why = if fd.has("why") { fd.get_string("why") } else { String::new() };
    let now = time();
    if verdict == "confirm" {
        // the curator's write: re-stamp the hash, mark reverified,
        // step confidence up - drift checked and survived is evidence
        let mut nsrc = src.deep_copy();
        nsrc.put_string("hash", &cur_hash);
        entry.put_object("source", nsrc);
        entry.put_int("reverified", now);
        let c = if entry.has("confidence") { entry.get_string("confidence") } else { "medium".to_string() };
        let up = conf_up(&c).to_string();
        entry.put_string("confidence", &up);
        // trace, adjudicate's shape
        let mut trace = DataObject::new();
        trace.put_string("input_claim", &wclaim);
        trace.put_string("relation", "reverified");
        trace.put_string("action", "confirmed");
        trace.put_string("before", &c);
        trace.put_string("after", &up);
        trace.put_string("reasoning", &why);
        trace.put_int("time", now);
        trace.put_string("author", "reverify");
        let old_traces = if data_obj.has("traces") { data_obj.get_string("traces").replace("\r", "") } else { String::new() };
        let mut traces_arr = if old_traces.trim().is_empty() { DataArray::new() } else {
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
        store.set_data(&wlib, &ctlid, record);
        journal(&store, &wlib, &ctlid, "memory", &old_source, &new_source, "reverify",
            &format!("reverify: confirmed {}", wclaim));
        confirmed += 1;
    } else if verdict == "amend" {
        let new_claim = if fd.has("claim") { fd.get_string("claim") } else { String::new() };
        if new_claim.trim().is_empty() { unparseable += 1; continue; }
        // acknowledge the drift on the old claim, insert the corrected
        // one with a FRESH source stamp - both through standing commands
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            decay(wlib.clone(), wdom.clone(), wclaim.clone(), "reverify".to_string())
        }));
        let mut ne = DataObject::new();
        ne.put_string("claim", new_claim.trim());
        ne.put_string("detail", &format!("amended by reverify: {}", why));
        ne.put_string("tags", "reverified,amended");
        ne.put_string("confidence", "medium");
        let mut nsrc = DataObject::new();
        nsrc.put_string("lib", &slib);
        nsrc.put_string("ctl", &sctl);
        nsrc.put_string("facet", &sfacet);
        ne.put_object("source", nsrc);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            remember(wlib.clone(), wdom.clone(), ne.deep_copy(), "reverify".to_string())
        }));
        amended += 1;
    } else if verdict == "retire" {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            decay(wlib.clone(), wdom.clone(), wclaim.clone(), "reverify".to_string())
        }));
        retired += 1;
    } else {
        unparseable += 1;
    }
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("spent", spent);
o.put_int("confirmed", confirmed);
o.put_int("amended", amended);
o.put_int("retired", retired);
o.put_int("unparseable", unparseable);
o

}
