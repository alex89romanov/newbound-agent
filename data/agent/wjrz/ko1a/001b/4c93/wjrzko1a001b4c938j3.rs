// seed_export (docs/one-memory-cycle.md A3): export the named kb domains -
// the primer - as one reviewable JSON document. Deliberate and explicit;
// this is also the brain's backup tool. Fails loud on a missing domain
// rather than silently exporting a partial primer.
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
    o.put_string("msg", "no kb library in this store - nothing to export");
    return o;
}
let list = store.get_data("kb", "controls").get_object("data").get_array("list");
let mut doms = DataArray::new();
let mut total: i64 = 0;
let mut missing: Vec<String> = Vec::new();
for want in domains.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
    let mut found = false;
    for ci in 0..list.len() {
        let item = list.get_object(ci);
        if !item.has("name") || item.get_string("name") != want || !item.has("id") { continue; }
        found = true;
        let cid = item.get_string("id");
        if !store.exists("kb", &cid) { break; }
        let dd = store.get_data("kb", &cid).get_object("data");
        let mut d = DataObject::new();
        d.put_string("name", &want);
        d.put_string("desc", &(if dd.has("desc") { dd.get_string("desc") } else { String::new() }));
        d.put_string("tags", &(if dd.has("tags") { dd.get_string("tags") } else { String::new() }));
        let entries = if dd.has("memory") && !dd.get_string("memory").trim().is_empty() {
            match DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory").replace("\r", ""))) {
                Ok(w) => match w.try_get_array("a") { Ok(a) => a, Err(_) => DataArray::new() },
                Err(_) => DataArray::new(),
            }
        } else { DataArray::new() };
        total += entries.len() as i64;
        d.put_array("entries", entries);
        doms.push_object(d);
        break;
    }
    if !found { missing.push(want); }
}
if !missing.is_empty() {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &format!("domains not found in kb: {}", missing.join(", ")));
    return o;
}
let mut seed = DataObject::new();
seed.put_int("version", 1);
seed.put_int("time", time());
seed.put_array("domains", doms.duplicate());
let text = val(Data::DObject(seed.data_ref), 0) + "\n";
if let Err(e) = std::fs::write(&path, &text) {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &format!("could not write {}: {}", path, e));
    return o;
}
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("path", &path);
o.put_int("domains", doms.len() as i64);
o.put_int("entries", total);
o
