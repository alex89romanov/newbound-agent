// persona_write: replace the persona corpus with validated JSONL.
// Every non-empty line must parse as {"user","assistant"} or
// {"messages":[...]} - any bad line rejects the WHOLE write with its
// line number, so a typo can never truncate the corpus. Atomic
// (tmp + rename). The service reads the corpus fresh at each gate
// tick and derivation, so saves take effect on their own - and a big
// rewrite can slip the probe and trigger re-derivation, by design.
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}
fn agent_root() -> Result<std::path::PathBuf, String> {
    let root = DataStore::new().root;
    let root = root.canonicalize().map_err(|e| format!("store root: {}", e))?;
    Ok(root.parent().ok_or("store root has no parent")?.to_path_buf())
}
fn valid_row(o: &DataObject) -> bool {
    (o.has("user") && o.has("assistant")) || o.has("messages")
}

let root = match agent_root() { Ok(r) => r, Err(e) => return err(e) };
let dir = root.join("runtime").join("agent").join("model").join("persona");
if let Err(e) = std::fs::create_dir_all(&dir) {
    return err(format!("persona dir: {}", e));
}
let mut lines: Vec<String> = Vec::new();
for (i, line) in content.lines().enumerate() {
    let line = line.trim();
    if line.is_empty() { continue; }
    match DataObject::try_from_string(line) {
        Ok(o) if valid_row(&o) => lines.push(o.to_string()),
        Ok(_) => return err(format!(
            "line {}: rows need user+assistant or messages", i + 1)),
        Err(_) => return err(format!("line {}: not valid JSON", i + 1)),
    }
}
let path = dir.join("persona.jsonl");
let tmp = dir.join("persona.jsonl.tmp");
let body = lines.join("\n") + "\n";
if let Err(e) = std::fs::write(&tmp, body.as_bytes()) {
    return err(format!("persona write failed: {}", e));
}
if let Err(e) = std::fs::rename(&tmp, &path) {
    return err(format!("persona rename failed: {}", e));
}
let n = lines.len() as i64;
let mut o = DataObject::new();
o.put_string("status", "ok");
o.put_int("rows", n);
o.put_int("heldout", n / 5);
o.put_string("path", &path.display().to_string());
o
