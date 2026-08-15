// epistemic_work (understandingloop.md Phase 4): the work queue derives
// from CLAIM STATE - no LLM, no randomness, nothing like the donor
// spawner's random injection (deleted for good by never being built).
// Kinds, by priority:
//   stale      (3): source hash drifted and not yet acknowledged -
//                   evidence the claim may be out of date; the one kind
//                   the executive may act on autonomously (decay).
//   review     (2): confidence at the low floor, or a drift already
//                   acknowledged - a human or a better extraction
//                   decides its fate; the executive only surfaces it.
//   unpromoted (1): subject-bearing claims still in the brain -
//                   promotion pressure. Surfaced, never auto-promoted:
//                   that channel stays curated (owner's ritual).
// "Gapped" (knowledge gaps) needs semantic judgment and waits for the
// salience tier (Phase 5). Read-only; safe at any tick rate.
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

let _ = val; let _ = obj; // serializer unused here; shared helper block
let store = DataStore::new();
let mut libs: Vec<String> = Vec::new();
if let Ok(rd) = std::fs::read_dir(&store.root) {
    for e in rd.flatten() {
        if e.path().is_dir() {
            if let Ok(n) = e.file_name().into_string() { libs.push(n); }
        }
    }
}
libs.sort();
let mut items: Vec<(i64, i64, DataObject)> = Vec::new();
let mut n_stale = 0i64;
let mut n_review = 0i64;
let mut n_unpromoted = 0i64;
for lib in libs {
    if !store.exists(&lib, "controls") { continue; }
    let list = store.get_data(&lib, "controls").get_object("data").get_array("list");
    for i in 0..list.len() {
        let item = list.get_object(i);
        if !item.has("name") || !item.has("id") { continue; }
        let name = item.get_string("name");
        let id = item.get_string("id");
        if !store.exists(&lib, &id) { continue; }
        let dd = store.get_data(&lib, &id).get_object("data");
        if !dd.has("memory") { continue; }
        let w = match DataObject::try_from_string(&format!("{{\"a\":{}}}", dd.get_string("memory"))) {
            Ok(w) => w,
            Err(_) => continue,
        };
        let a = match w.try_get_array("a") { Ok(a) => a, Err(_) => continue };
        for j in 0..a.len() {
            let e = match a.try_get_object(j) { Ok(e) => e, Err(_) => continue };
            if !e.has("claim") || e.has("superseded") { continue; }
            let claim = e.get_string("claim");
            let t0 = if e.has("time") { e.get_int("time") } else { 0 };
            let conf = if e.has("confidence") { e.get_string("confidence") } else { "medium".to_string() };
            let mut pushed = false;
            if e.has("source") {
                if let Ok(src) = e.try_get_object("source") {
                    if src.has("lib") && src.has("ctl") && src.has("facet") && src.has("hash") {
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
                        if current != src.get_string("hash") {
                            let acknowledged = e.has("decayed_hash")
                                && e.get_string("decayed_hash") == current;
                            let mut it = DataObject::new();
                            it.put_string("lib", &lib);
                            it.put_string("domain", &name);
                            it.put_string("claim", &claim);
                            if acknowledged {
                                it.put_string("kind", "review");
                                it.put_int("priority", 2);
                                it.put_string("why", &format!("source {}.{}:{} drift acknowledged; awaiting review", slib, sctl, sfacet));
                                n_review += 1;
                                items.push((2, t0, it));
                            } else {
                                it.put_string("kind", "stale");
                                it.put_int("priority", 3);
                                it.put_string("why", &format!("source {}.{}:{} drifted since this claim was stamped", slib, sctl, sfacet));
                                n_stale += 1;
                                items.push((3, t0, it));
                            }
                            pushed = true;
                        }
                    }
                }
            }
            if !pushed && conf == "low" {
                let mut it = DataObject::new();
                it.put_string("kind", "review");
                it.put_int("priority", 2);
                it.put_string("lib", &lib);
                it.put_string("domain", &name);
                it.put_string("claim", &claim);
                it.put_string("why", "confidence at the low floor - contradicted or decayed down; needs review");
                n_review += 1;
                items.push((2, t0, it));
                pushed = true;
            }
            if !pushed && e.has("subject") {
                let mut it = DataObject::new();
                it.put_string("kind", "unpromoted");
                it.put_int("priority", 1);
                it.put_string("lib", &lib);
                it.put_string("domain", &name);
                it.put_string("claim", &claim);
                it.put_string("why", &format!("subject '{}' claim still in the brain - awaits promote", e.get_string("subject")));
                n_unpromoted += 1;
                items.push((1, t0, it));
            }
        }
    }
}
items.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
let mut out = DataArray::new();
for (_, _, it) in items.iter().take(20) { out.push_object(it.clone()); }
let total = items.len() as i64;
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_array("items", out);
o.put_int("stale", n_stale);
o.put_int("review", n_review);
o.put_int("unpromoted", n_unpromoted);
o.put_int("total", total);
o
