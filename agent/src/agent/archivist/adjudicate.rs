use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::data::Data;
use flowlang::datastore::DataStore;
use flowlang::command::Command;
use flowlang::flowlang::system::time::time;
use crate::agent::llm::ask_llm::ask_llm;
pub fn execute(o: DataObject) -> DataObject {
    use std::panic;
    for p in ["lib", "domain", "entry", "author"] {
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
        let arg_2: DataObject = o.get_object("entry");
        let arg_3: String = o.get_string("author");
        adjudicate(arg_0, arg_1, arg_2, arg_3)
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

pub fn adjudicate(lib: String, domain: String, entry: DataObject, author: String) -> DataObject {
// adjudicate (understandingloop.md Phase 3; donor: jerry's Curator): the
// write-side counterpart to recall. A new claim is matched against the
// domain's existing beliefs; the relationship (corroborates / contradicts
// / novel) is decided procedurally where certain (exact restatement, no
// candidates) and by the LLM where judgment is needed; the EFFECT is
// governed by the hysteresis rule, never the LLM: a flaky extraction
// moves a claim's confidence one step, it never toggles the claim.
//   corroborates: low -> medium -> high; at high the claim is SETTLED and
//     the call writes NOTHING - the fixed point that keeps self-feedback
//     convergent (owner's call, 2026-08-15: convergence over echo
//     suppression).
//   contradicts: high -> medium -> low; only a contradiction AT low
//     supersedes - three consecutive contradictions to flip a high
//     belief, no thrash. The superseding draft enters at LOW and earns
//     its way up like everything else.
//   novel: files through remember (validation, source stamping,
//     dedupe - the whole write path, unchanged).
// Every adjudication that writes also appends a curation trace to the
// domain's `traces` facet (capped at 200) - provenance for the owner's
// audit today, curriculum material for the flywheel later.
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
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
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
fn toks(s: &str) -> Vec<String> {
    const STOP: &[&str] = &["the", "and", "for", "with", "that", "this", "from",
        "are", "was", "were", "has", "have", "had", "does", "did", "not", "its",
        "you", "your", "all", "any", "can", "will", "into", "out", "about",
        "what", "when", "how", "why", "who", "against"];
    s.to_lowercase().split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && !STOP.contains(t))
        .map(|t| t.to_string())
        .collect()
}
fn conf_up(c: &str) -> &'static str {
    match c { "low" => "medium", "medium" => "high", _ => "high" }
}
fn conf_down(c: &str) -> &'static str {
    match c { "high" => "medium", "medium" => "low", _ => "low" }
}

let mut entry = entry;
if !entry.has("claim") {
    return err("entry.claim is required - one atomic, recallable sentence".to_string());
}
let claim = match entry.try_get_string("claim") {
    Ok(c) => c.trim().to_string(),
    Err(_) => return err("entry.claim must be a string".to_string()),
};
if claim.is_empty() {
    return err("entry.claim must not be empty".to_string());
}
entry.put_string("claim", &claim);
let draft_conf_ok = entry.has("confidence")
    && ["high", "medium", "low"].contains(&entry.get_string("confidence").as_str());
if !draft_conf_ok {
    entry.put_string("confidence", "medium");
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
    String::new()
};
let arr = if old_source.trim().is_empty() {
    DataArray::new()
} else {
    match DataObject::try_from_string(&format!("{{\"a\":{}}}", old_source)) {
        Ok(w) => match w.try_get_array("a") {
            Ok(a) => a,
            Err(_) => return err(format!("{}.{}'s memory facet is not a JSON ARRAY - fix it before adjudicating", lib, domain)),
        },
        Err(_) => return err(format!("{}.{}'s memory facet is not valid JSON - fix it before adjudicating; it is refused, never clobbered", lib, domain)),
    }
};

// ── match: exact restatement, token-overlap candidates, or nothing ──────
let draft_tokens = toks(&claim);
let mut exact: i64 = -1;
let mut cands: Vec<(usize, usize)> = Vec::new(); // (idx, overlap)
for i in 0..arr.len() {
    let e = match arr.try_get_object(i) { Ok(e) => e, Err(_) => continue };
    if !e.has("claim") || e.has("superseded") { continue; }
    let ec = e.get_string("claim");
    if ec.trim() == claim {
        exact = i as i64;
        break;
    }
    let et = toks(&ec);
    let overlap = draft_tokens.iter().filter(|t| et.contains(t)).count();
    if overlap >= 2 {
        cands.push((i, overlap));
    }
}
cands.sort_by(|a, b| b.1.cmp(&a.1));
cands.truncate(3);

// ── decide the relationship ─────────────────────────────────────────────
// Procedural where certain; the LLM only where judgment is needed, and
// its parse failure defaults to novel (the donor's default-INSERT).
let mut relation;
let mut target: i64 = -1;
let mut reasoning;
if exact >= 0 {
    relation = "corroborates".to_string();
    target = exact;
    reasoning = "exact restatement".to_string();
} else if cands.is_empty() {
    relation = "novel".to_string();
    reasoning = "no related beliefs in this domain".to_string();
} else {
    let mut listing = String::new();
    for (n, (i, _)) in cands.iter().enumerate() {
        let e = arr.get_object(*i);
        let c = if e.has("confidence") { e.get_string("confidence") } else { "medium".to_string() };
        listing.push_str(&format!("{}: \"{}\" (confidence: {})\n", n, e.get_string("claim"), c));
    }
    // The prompt template is overridable via an `adjudication` facet on
    // agent.archivist (consolidate's pattern: store-resident, tunable
    // without a rebuild); the built-in default keeps the command whole on
    // a fresh install.
    let selfid = lookup_ctl_id("agent", "archivist");
    let tmpl = {
        let mut t = String::new();
        if !selfid.is_empty() && store.exists("agent", &selfid) {
            let sp = store.get_data("agent", &selfid).get_object("data");
            if sp.has("adjudication") {
                t = sp.get_string("adjudication");
            }
        }
        if t.trim().is_empty() {
            t = "You adjudicate a NEW CLAIM against EXISTING BELIEFS in one memory domain.\n\nEXISTING BELIEFS:\n{cands}\nNEW CLAIM: \"{claim}\"\n\nDoes the new claim corroborate (restate or directly support) ONE existing belief, contradict ONE, or say something genuinely new?\nReply with ONLY a JSON object: {\"relation\": \"corroborates\" | \"contradicts\" | \"novel\", \"target\": <number of the existing belief, or -1 for novel>, \"reasoning\": \"<one sentence>\"}".to_string();
        }
        t
    };
    let prompt = tmpl.replace("{cands}", &listing).replace("{claim}", &claim);
    let resp = ask_llm(prompt, Data::DNull);
    relation = "novel".to_string();
    reasoning = "unparseable adjudication; defaulting to insert".to_string();
    if !resp.starts_with("ERROR") {
        if let (Some(s), Some(e)) = (resp.find('{'), resp.rfind('}')) {
            if e > s {
                if let Ok(d) = DataObject::try_from_string(&resp[s..=e]) {
                    let r = if d.has("relation") { d.get_string("relation") } else { String::new() };
                    let t = if d.has("target") { d.get_int("target") } else { -1 };
                    if (r == "corroborates" || r == "contradicts")
                        && t >= 0 && (t as usize) < cands.len() {
                        relation = r;
                        target = cands[t as usize].0 as i64;
                        reasoning = if d.has("reasoning") { d.get_string("reasoning") } else { String::new() };
                    } else if r == "novel" {
                        reasoning = if d.has("reasoning") { d.get_string("reasoning") } else { "judged novel".to_string() };
                    }
                }
            }
        }
    }
}

// ── apply under hysteresis ──────────────────────────────────────────────
let now = time();
let mut action = String::new();
let mut before = String::new();
let mut after = String::new();
let mut wrote_facet = false;
let mut inserted = false;

if relation == "corroborates" {
    let mut t = arr.get_object(target as usize);
    let c = if t.has("confidence") { t.get_string("confidence") } else { "medium".to_string() };
    before = c.clone();
    if c == "high" {
        // SETTLED: the fixed point. Writes nothing - acting twice on the
        // same state must not write twice.
        let mut o = DataObject::new();
        o.put_string("status", "ok");
        o.put_string("action", "settled");
        o.put_string("target", &t.get_string("claim"));
        o.put_string("reasoning", &reasoning);
        return o;
    }
    after = conf_up(&c).to_string();
    t.put_string("confidence", &after);
    t.put_int("corroborations", if t.has("corroborations") { t.get_int("corroborations") + 1 } else { 1 });
    t.put_int("adjudicated", now);
    action = "corroborated".to_string();
    wrote_facet = true;
} else if relation == "contradicts" {
    let mut t = arr.get_object(target as usize);
    let c = if t.has("confidence") { t.get_string("confidence") } else { "medium".to_string() };
    before = c.clone();
    if c == "low" {
        // Floor: the belief flips. The old claim retires in place (history,
        // not deletion); the contradicting draft enters at LOW.
        let mut sup = DataObject::new();
        sup.put_string("by", &claim);
        sup.put_string("reason", &reasoning);
        sup.put_int("time", now);
        t.put_object("superseded", sup);
        t.put_int("adjudicated", now);
        after = "superseded".to_string();
        action = "superseded".to_string();
        entry.put_string("confidence", "low");
    } else {
        after = conf_down(&c).to_string();
        t.put_string("confidence", &after);
        t.put_int("adjudicated", now);
        action = "contradicted".to_string();
        // The draft is NOT stored: the target's confidence is the
        // accumulator. Repeat contradictions walk it down the ladder.
    }
    wrote_facet = true;
} else {
    action = "inserted".to_string();
}

// ── the curation trace (capped) ─────────────────────────────────────────
let mut trace = DataObject::new();
trace.put_string("input_claim", &claim);
trace.put_string("relation", &relation);
trace.put_string("action", &action);
if target >= 0 {
    trace.put_string("target", &arr.get_object(target as usize).get_string("claim"));
}
if !before.is_empty() { trace.put_string("before", &before); }
if !after.is_empty() { trace.put_string("after", &after); }
trace.put_string("reasoning", &reasoning);
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
let new_traces = val(Data::DArray(traces_arr.data_ref), 0) + "\n";

// ── write ───────────────────────────────────────────────────────────────
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

let mut patch_id = String::new();
if wrote_facet {
    let new_source = val(Data::DArray(arr.data_ref), 0) + "\n";
    data_obj.put_string("memory", &new_source);
    ensure_key(&mut data_obj, "memory");
    data_obj.put_string("traces", &new_traces);
    ensure_key(&mut data_obj, "traces");
    record.put_object("data", data_obj);
    record.put_int("time", now);
    store.set_data(&lib, &ctlid, record);
    patch_id = journal(&store, &lib, &ctlid, "memory", &old_source, &new_source, &author,
        &format!("adjudicate: {} {}", action, claim));
} else {
    // novel: remember does the whole validated write (source stamping,
    // dedupe, its own journal entry); the trace follows in its own
    // facet write.
    let cmd = Command::lookup("agent", "archivist", "remember");
    let mut args = DataObject::new();
    args.put_string("lib", &lib);
    args.put_string("domain", &domain);
    args.put_object("entry", entry.deep_copy());
    args.put_string("author", &author);
    match cmd.execute(args) {
        Ok(r) => {
            if r.has("a") && r.get_object("a").has("status")
                && r.get_object("a").get_string("status") == "ok" {
                inserted = true;
            } else if r.has("a") && r.get_object("a").has("msg") {
                let mut o = DataObject::new();
                o.put_string("status", "err");
                o.put_string("msg", &format!("insert refused: {}", r.get_object("a").get_string("msg")));
                return o;
            }
        }
        Err(_) => return err("remember failed during novel insert".to_string()),
    }
    let mut record2 = store.get_data(&lib, &ctlid);
    let mut d2 = record2.get_object("data");
    let old_t = if d2.has("traces") { d2.get_string("traces").replace("\r", "") } else { String::new() };
    d2.put_string("traces", &new_traces);
    ensure_key(&mut d2, "traces");
    record2.put_object("data", d2);
    record2.put_int("time", now);
    store.set_data(&lib, &ctlid, record2);
    patch_id = journal(&store, &lib, &ctlid, "traces", &old_t, &new_traces, &author,
        &format!("adjudicate: inserted {}", claim));
}

if action == "superseded" {
    // The successor enters at LOW through the full write path.
    let cmd = Command::lookup("agent", "archivist", "remember");
    let mut args = DataObject::new();
    args.put_string("lib", &lib);
    args.put_string("domain", &domain);
    args.put_object("entry", entry.deep_copy());
    args.put_string("author", &author);
    if let Ok(r) = cmd.execute(args) {
        if r.has("a") && r.get_object("a").has("status")
            && r.get_object("a").get_string("status") == "ok" {
            inserted = true;
        }
    }
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("action", &action);
if target >= 0 {
    o.put_string("target", &arr.get_object(target as usize).get_string("claim"));
}
if !before.is_empty() { o.put_string("before", &before); }
if !after.is_empty() { o.put_string("after", &after); }
o.put_boolean("inserted", inserted);
o.put_string("reasoning", &reasoning);
if !patch_id.is_empty() { o.put_string("patch_id", &patch_id); }
o

}
