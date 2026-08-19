// agent-context-assemble - purpose-built context from every knowledge
// source (harvest H2). One command any consumer calls - chat shells,
// the escalation prompt, rumination acts, MCP delegates, all arms
// alike. `purpose` picks a PROFILE (per-source budget weights);
// `budget` is a token ceiling enforced by ranked truncation (tokens
// estimated at chars/4). Every block carries provenance so downstream
// answers can cite and downstream training pairs inherit traceability.
// Sources are tolerated individually: an absent service or an empty
// venue costs its section, never the block. Read-only against every
// store; its one write is the metrics row - the syspack-shrinkage
// baseline (token counts per purpose, watched to FALL as the local
// model absorbs the domain).
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn clip(s: &str, chars: usize) -> String {
    if s.chars().count() <= chars { return s.to_string(); }
    let cut: String = s.chars().take(chars.saturating_sub(1)).collect();
    format!("{}\u{2026}", cut)
}
fn hhmm(t: i64) -> String {
    format!("{:02}:{:02}", (t / 3_600_000) % 24, (t / 60_000) % 60)
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

let purpose_t = purpose.trim().to_lowercase();
// profile -> (claims, code, room, session, system) weights; a weight of
// zero drops the section outright. These are the H2 proposal pending
// owner call 2 - tune per profile in one place.
let weights: (f64, f64, f64, f64, f64) = match purpose_t.as_str() {
    "chat"       => (0.45, 0.00, 0.20, 0.25, 0.10),
    "escalation" => (0.60, 0.30, 0.00, 0.00, 0.10),
    "rumination" => (0.50, 0.30, 0.10, 0.00, 0.10),
    "coding"     => (0.35, 0.55, 0.00, 0.00, 0.10),
    "briefing"   => (0.30, 0.00, 0.25, 0.20, 0.25),
    _ => { return err(format!("unknown purpose '{}' - profiles: chat | escalation | rumination | coding | briefing", purpose)); }
};
if budget <= 0 { return err("budget must be > 0 (a token ceiling)".to_string()); }
let budget_chars = (budget as f64) * 4.0;
let store = DataStore::new();
let mut sections: Vec<String> = Vec::new();
let mut src_tokens = DataObject::new();

// ── claims: the federation, staleness marks honored ──────────────────
let mut code_ptrs: Vec<(String, String, String, bool)> = Vec::new();
if weights.0 > 0.0 {
    let cap_chars = (budget_chars * weights.0) as usize;
    let r = recall(subject.clone(), String::new(), 24);
    let mut part = String::new();
    if r.has("claims") {
        let list = r.get_array("claims");
        for i in 0..list.len() {
            if let Ok(c) = list.try_get_object(i) {
                let stale = c.has("stale") && c.get_boolean("stale");
                let home = if c.has("home") { c.get_string("home") } else { String::new() };
                let age = if c.has("age_days") { c.get_int("age_days") } else { -1 };
                let mark = if stale { " STALE".to_string() }
                    else if age >= 0 { format!(" age={}d", age) } else { String::new() };
                let line = format!("[claim {}{}] {}\n", home, mark,
                    clip(&c.get_string("claim"), 400));
                if part.chars().count() + line.chars().count() > cap_chars { break; }
                part.push_str(&line);
                // remember the top few facet pointers for the code
                // section. Stale claims' pointers are INCLUDED: a
                // drifted referent is exactly when the live code matters
                // most - the drift mark rides the code block instead.
                if code_ptrs.len() < 4 && c.has("source") {
                    if let Ok(s) = c.try_get_object("source") {
                        code_ptrs.push((s.get_string("lib"), s.get_string("ctl"), s.get_string("facet"), stale));
                    }
                }
            }
        }
    }
    src_tokens.put_int("claims", (part.chars().count() / 4) as i64);
    if !part.is_empty() { sections.push(format!("## Claims (federated memory)\n{}", part)); }
}

// ── code: the claims' referents - the store IS the codebase ──────────
if weights.1 > 0.0 && !code_ptrs.is_empty() {
    let cap_chars = (budget_chars * weights.1) as usize;
    let per = cap_chars / code_ptrs.len().max(1);
    let mut part = String::new();
    for (lib, ctl, facet, drifted) in &code_ptrs {
        let cid = ctl_id(&store, lib, ctl);
        if cid.is_empty() || !store.exists(lib, &cid) { continue; }
        let d = store.get_data(lib, &cid).get_object("data");
        if !d.has(facet.as_str()) { continue; }
        let content = d.get_string(facet.as_str());
        let mark = if *drifted { " DRIFTED-SINCE-CLAIM" } else { "" };
        // the clip must leave room for the provenance header AND the
        // newlines, or a full-budget piece overshoots by a few chars
        // and the break below empties the whole section
        let header = format!("[code {}.{} {}{}]\n", lib, ctl, facet, mark);
        let room = per.saturating_sub(header.chars().count() + 2);
        let piece = format!("{}{}\n", header, clip(&content, room));
        if part.chars().count() + piece.chars().count() > cap_chars { break; }
        part.push_str(&piece);
    }
    src_tokens.put_int("code", (part.chars().count() / 4) as i64);
    if !part.is_empty() { sections.push(format!("## Code (claim referents)\n{}", part)); }
}

// ── room: recent acoustic reality, by venue - never by sensor ────────
if weights.2 > 0.0 {
    let cap_chars = (budget_chars * weights.2) as usize;
    let r = recent("room".to_string(), 30);
    let mut part = String::new();
    if r.has("messages") {
        let list = r.get_array("messages");
        // newest matter most: walk backward, prepend, stop at budget
        let mut i = list.len() as i64 - 1;
        while i >= 0 {
            if let Ok(m) = list.try_get_object(i as usize) {
                let who = if m.has("entity") && !m.get_string("entity").is_empty() {
                    format!(" {}", m.get_string("entity")) } else { String::new() };
                let t = if m.has("t") { m.get_int("t") } else { 0 };
                let line = format!("[transcript {}{}] {}\n", hhmm(t), who,
                    clip(&m.get_string("content"), 300));
                if part.chars().count() + line.chars().count() > cap_chars { break; }
                part.insert_str(0, &line);
            }
            i -= 1;
        }
    }
    src_tokens.put_int("room", (part.chars().count() / 4) as i64);
    if !part.is_empty() { sections.push(format!("## Room (recent speech)\n{}", part)); }
}

// ── session: the archivist's transient turn queue ────────────────────
if weights.3 > 0.0 {
    let cap_chars = (budget_chars * weights.3) as usize;
    let mut part = String::new();
    if store.exists("runtime", "archivist_queue") {
        let d = store.get_data("runtime", "archivist_queue").get_object("data");
        if d.has("turns") {
            let turns = d.get_array("turns");
            let mut i = turns.len() as i64 - 1;
            while i >= 0 {
                if let Ok(t) = turns.try_get_object(i as usize) {
                    let venue = if t.has("venue") { t.get_string("venue") } else { String::new() };
                    let ask = if t.has("ask") { t.get_string("ask") } else { String::new() };
                    let reply = if t.has("reply") { t.get_string("reply") } else { String::new() };
                    let line = format!("[turn {}] Q: {} A: {}\n", venue,
                        clip(&ask, 200), clip(&reply, 200));
                    if part.chars().count() + line.chars().count() > cap_chars { break; }
                    part.insert_str(0, &line);
                }
                i -= 1;
            }
        }
    }
    src_tokens.put_int("session", (part.chars().count() / 4) as i64);
    if !part.is_empty() { sections.push(format!("## Session (recent turns)\n{}", part)); }
}

// ── system: service/trainer metrics + the banks' row counts ──────────
if weights.4 > 0.0 {
    let cap_chars = (budget_chars * weights.4) as usize;
    let mut part = String::new();
    if let Some(root) = store.root.canonicalize().ok()
            .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
        let mpath = root.join("runtime").join("agent").join("model").join("metrics.jsonl");
        if let Ok(text) = std::fs::read_to_string(&mpath) {
            let tail: Vec<&str> = text.lines().rev().take(3).collect();
            for ln in tail.iter().rev() {
                let line = format!("[metrics] {}\n", clip(ln, 300));
                if part.chars().count() + line.chars().count() > cap_chars { break; }
                part.push_str(&line);
            }
        }
    }
    if store.exists("runtime", "datasets") {
        let d = store.get_data("runtime", "datasets").get_object("data");
        if d.has("list") {
            let list = d.get_array("list");
            let mut counts: Vec<String> = Vec::new();
            for i in 0..list.len() {
                if let Ok(m) = list.try_get_object(i) {
                    counts.push(format!("{}={}", m.get_string("name"),
                        if m.has("rows") { m.get_int("rows") } else { 0 }));
                }
            }
            if !counts.is_empty() {
                let line = format!("[banks] {}\n", counts.join(" "));
                if part.chars().count() + line.chars().count() <= cap_chars {
                    part.push_str(&line);
                }
            }
        }
    }
    src_tokens.put_int("system", (part.chars().count() / 4) as i64);
    if !part.is_empty() { sections.push(format!("## System\n{}", part)); }
}

let block = sections.join("\n");
let tokens = (block.chars().count() / 4) as i64;

// the syspack-shrinkage baseline: one metrics row per assembly
if let Some(root) = store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    let dir = root.join("runtime").join("agent").join("model");
    if std::fs::create_dir_all(&dir).is_ok() {
        use std::io::Write;
        let mut row = DataObject::new();
        row.put_int("t", time());
        row.put_string("kind", "context");
        row.put_string("purpose", &purpose_t);
        row.put_int("budget", budget);
        row.put_int("tokens", tokens);
        row.put_object("sources", src_tokens.deep_copy());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true)
                .open(dir.join("metrics.jsonl")) {
            let _ = writeln!(f, "{}", row.to_string().replace('\n', " "));
        }
    }
}

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_string("purpose", &purpose_t);
o.put_string("block", &block);
o.put_int("tokens", tokens);
o.put_int("budget", budget);
o.put_object("sources", src_tokens);
o
