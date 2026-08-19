// agent-model-why_harvest - the procedural why-harvester (harvest H3a).
// The store journals every mutation with a label and an author; our
// commits carry rationale. This command pairs each CHANGE with its
// STATED why - no model, free, retroactive - and feeds the pairs into
// the code-why stream dataset (kind cpt) through the one feeder.
// Re-runs are idempotent: dataset_feed dedups line-wise.
//
// Two sources:
//   patches - every library's _patches journals: one row per patch
//     entry {home, facet|cmd, label (the why), author, t, excerpt}.
//   git - commit subjects+bodies (the why) paired with --stat file
//     lists (the change), from repo_path (empty = this checkout).
//     Owner call 5's proposal is full history; depth caps a run.
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
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n { return s.to_string(); }
    s.chars().take(n).collect()
}
fn walk_patches(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { walk_patches(&p, out); }
            else if p.file_name().and_then(|n| n.to_str())
                     .map(|n| n.ends_with("_patches")).unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

let source_t = source.trim().to_lowercase();
if !["patches", "git", "both"].contains(&source_t.as_str()) {
    return err(format!("source must be patches | git | both (got '{}')", source));
}
if limit < 0 { return err("limit must be >= 0 (0 = no cap)".to_string()); }
let store = DataStore::new();
let root = match store.root.canonicalize().ok()
        .and_then(|r| r.parent().map(|p| p.to_path_buf())) {
    Some(r) => r,
    None => { return err("cannot resolve the checkout root".to_string()); }
};

let mut lines: Vec<String> = Vec::new();
let mut patch_rows = 0i64;
let mut git_rows = 0i64;

if source_t == "patches" || source_t == "both" {
    // id -> control name, per library, from the controls records
    let mut names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut libs: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(store.root.clone()) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                if let Some(lib) = e.file_name().to_str() { libs.push(lib.to_string()); }
            }
        }
    }
    for lib in &libs {
        if !store.exists(lib, "controls") { continue; }
        let d = store.get_data(lib, "controls").get_object("data");
        if !d.has("list") { continue; }
        for c in d.get_array("list").objects() {
            let c = c.object();
            if c.has("id") && c.has("name") {
                names.insert(format!("{}/{}", lib, c.get_string("id")),
                             c.get_string("name"));
            }
        }
    }
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    walk_patches(&store.root, &mut files);
    files.sort();
    'outer: for f in &files {
        let rel = f.strip_prefix(&store.root).unwrap_or(f);
        let lib = rel.components().next()
            .and_then(|c| c.as_os_str().to_str()).unwrap_or("").to_string();
        let id = f.file_name().and_then(|n| n.to_str()).unwrap_or("")
            .trim_end_matches("_patches").to_string();
        let home = match names.get(&format!("{}/{}", lib, id)) {
            Some(n) => format!("{}.{}", lib, n),
            None => format!("{}.{}", lib, clip(&id, 8)),
        };
        let text = std::fs::read_to_string(f).unwrap_or_default();
        let rec = match DataObject::try_from_string(&text) { Ok(r) => r, Err(_) => continue };
        let d = rec.get_object("data");
        if !d.has("list") { continue; }
        let list = d.get_array("list");
        for i in 0..list.len() {
            if let Ok(p) = list.try_get_object(i) {
                let label = if p.has("label") { p.get_string("label") } else { String::new() };
                if label.trim().is_empty() { continue; }
                let what = if p.has("facet") { format!("facet {}", p.get_string("facet")) }
                    else if p.has("cmd") { format!("command {}", p.get_string("cmd")) }
                    else { "record".to_string() };
                let author = if p.has("author") { p.get_string("author") } else { String::new() };
                let t = if p.has("time") { p.get_int("time") } else { 0 };
                let excerpt = if p.has("new") { clip(&p.get_string("new"), 240) } else { String::new() };
                lines.push(format!(
                    "{{\"kind\": \"code_why\", \"source\": \"patch\", \"home\": \"{}\", \"what\": \"{}\", \"why\": \"{}\", \"author\": \"{}\", \"t\": {}, \"excerpt\": \"{}\"}}",
                    esc(&home), esc(&what), esc(&label), esc(&author), t, esc(&excerpt)));
                patch_rows += 1;
                if limit > 0 && patch_rows >= limit { break 'outer; }
            }
        }
    }
}

if source_t == "git" || source_t == "both" {
    let repo = if repo_path.trim().is_empty() { root.display().to_string() }
               else { repo_path.trim().to_string() };
    if !std::path::Path::new(&repo).join(".git").exists() {
        return err(format!("'{}' is not a git checkout", repo));
    }
    let mut cmdv = DataArray::new();
    for a in ["git", "-C", &repo, "log",
              "--pretty=format:%x1e%H%x1f%an%x1f%at%x1f%B%x1f", "--stat",
              "--no-color"] {
        cmdv.push_string(a);
    }
    if limit > 0 {
        cmdv.push_string("-n");
        cmdv.push_string(&limit.to_string());
    }
    let r = system_call(cmdv);
    if r.try_get_string("status").ok().as_deref() != Some("ok") {
        return err("git log failed to execute".to_string());
    }
    let out_s = r.get_string("out");
    for chunk in out_s.split('\u{1e}') {
        let chunk = chunk.trim();
        if chunk.is_empty() { continue; }
        let parts: Vec<&str> = chunk.split('\u{1f}').collect();
        if parts.len() < 4 { continue; }
        let (sha, author, at, msg) = (parts[0], parts[1], parts[2], parts[3]);
        let stat = parts.get(4).map(|s| s.trim()).unwrap_or("");
        // trailers are provenance, not rationale - they would teach the
        // model to emit attribution footers as "why"
        let why: String = msg.lines()
            .filter(|l| {
                let lt = l.trim();
                !lt.starts_with("Co-Authored-By:") && !lt.starts_with("Claude-Session:")
                    && !lt.starts_with("Signed-off-by:")
            })
            .collect::<Vec<_>>().join("\n");
        let why = why.trim();
        if why.is_empty() { continue; }
        // the stat is the change: file list + churn, one compact line each
        let files: Vec<String> = stat.lines().map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()).take(24).collect();
        lines.push(format!(
            "{{\"kind\": \"code_why\", \"source\": \"commit\", \"commit\": \"{}\", \"author\": \"{}\", \"t\": {}, \"why\": \"{}\", \"change\": \"{}\"}}",
            esc(&clip(sha, 12)), esc(author),
            at.trim().parse::<i64>().unwrap_or(0) * 1000,
            esc(&clip(why, 2000)), esc(&clip(&files.join("; "), 1200))));
        git_rows += 1;
        if limit > 0 && git_rows >= limit { break; }
    }
}

if lines.is_empty() {
    let mut o = DataObject::new();
    o.put_string("status", "ok");
    o.put_int("patch_rows", 0);
    o.put_int("git_rows", 0);
    o.put_int("appended", 0);
    o.put_string("note", "nothing to harvest (no labeled patches / no commits matched)");
    return o;
}
let fed = dataset_feed("code-why".to_string(), "cpt".to_string(),
    lines.join("\n"), "harvested:patches+git".to_string(),
    "why_harvest".to_string(), 10);
let appended = if fed.has("appended") { fed.get_int("appended") } else { 0 };
let total = if fed.has("rows") { fed.get_int("rows") } else { -1 };

let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("patch_rows", patch_rows);
o.put_int("git_rows", git_rows);
o.put_int("appended", appended);
o.put_int("dataset_rows", total);
o.put_string("dataset", "code-why");
o
