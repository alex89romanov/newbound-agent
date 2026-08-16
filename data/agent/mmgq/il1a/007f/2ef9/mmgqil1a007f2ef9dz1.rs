// bootstrap: the agent builds its own nanochat server (owner directive,
// 2026-08-16). Idempotent and settings-driven - no seams, no runbook
// steps: SALIENCE=on in botd.properties turns the subsystem on, and the
// executive fires this once per start whenever the service isn't
// answering. Modeled on the owner's oneshot-installer idiom: donefile-
// guarded stages, everything under the agent app's own runtime folder
// (runtime/agent/model - the runtime/ root belongs to apps), nothing
// at a hardcoded absolute path.
//
//   MODEL_CHECKPOINT    stub (default) | path to a nanochat checkpoint
//   MODEL_SERVICE_PORT  8077 (default)
//   NANOCHAT_REPO       https://github.com/karpathy/nanochat.git
//
// Stub mode installs nothing (the service is stdlib python); the
// nanochat environment (clone + venv + deps) is only built when a real
// checkpoint is configured - that is the GPU box's path. The service
// script ships as a library asset compiled into this dylib
// (data/agent/_ASSETS/service.py): the platform carries its own server
// and rewrites the on-disk copy whenever the shipped one differs.
fn prop(key: &str, dflt: &str) -> String {
    // Settings live in runtime/agent/botd.properties like everything else.
    (|| -> Option<String> {
        let s = DataStore::globals().try_get_object("system").ok()?;
        let a = s.try_get_object("apps").ok()?;
        let g = a.try_get_object("agent").ok()?;
        let r = g.try_get_object("runtime").ok()?;
        match r.try_get_string(key) {
            Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
            _ => None,
        }
    })().unwrap_or_else(|| dflt.to_string())
}
fn service_url() -> String {
    format!("http://127.0.0.1:{}", prop("MODEL_SERVICE_PORT", "8077"))
}
fn err(msg: String) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &msg);
    o
}

let checkpoint = prop("MODEL_CHECKPOINT", "stub");
let port = prop("MODEL_SERVICE_PORT", "8077");
let repo = prop("NANOCHAT_REPO", "https://github.com/karpathy/nanochat.git");

let root = DataStore::new().root;
let root = match root.canonicalize() {
    Ok(r) => r,
    Err(e) => { return err(format!("store root: {}", e)); }
};
let root = match root.parent() {
    Some(p) => p.to_path_buf(),
    None => { return err("store root has no parent".to_string()); }
};
let modeldir = root.join("runtime").join("agent").join("model");
let deps = modeldir.join("deps");
let _ = std::fs::create_dir_all(&deps);

let mut o = DataObject::new();
o.put_string("status", "ok");

// stage 1: the nanochat environment - only for a real checkpoint.
// No `pip install -e .`: nanochat's repo is flat-layout (dev/, runs/,
// nanochat/ at top level) and setuptools refuses to build it. The
// package is never installed - the service runs with PYTHONPATH at the
// clone; only the pyproject [project] dependencies go into the venv,
// extracted with tomllib. The success sentinel (env_ready) is written
// by the script ITSELF as its last act under set -e, so a failed pip
// can never mark the env ready (the old rust-side venv-exists check
// could, and did).
let nc = deps.join("nanochat");
let mut nanochat_env = "not_needed".to_string();
if checkpoint != "stub" {
    let sentinel = nc.join("env_ready");
    if sentinel.exists() && nc.join("venv").exists() {
        nanochat_env = "ready".to_string();
    } else {
        if nc.exists() { let _ = std::fs::remove_dir_all(&nc); }
        let mut cmd = "set -e; cd ".to_string();
        cmd += &deps.display().to_string();
        cmd += &format!("; git clone {} nanochat", repo);
        cmd += "; cd nanochat; python3 -m venv venv; source venv/bin/activate";
        cmd += "; pip install --upgrade pip setuptools wheel";
        cmd += "; python3 -c 'import tomllib; print(\"\\n\".join(tomllib.load(open(\"pyproject.toml\",\"rb\"))[\"project\"][\"dependencies\"]))' > .deps.txt";
        cmd += "; pip install -r .deps.txt";
        cmd += "; touch env_ready";
        let mut x = DataArray::new();
        x.push_string("bash");
        x.push_string("-c");
        x.push_string(&cmd);
        let r = system_call(x);
        println!("BOOTSTRAP NANOCHAT ENV {}", r.to_string());
        if sentinel.exists() {
            nanochat_env = "installed".to_string();
        } else {
            nanochat_env = "install_failed".to_string();
            o.put_string("status", "err");
        }
    }
}
o.put_string("nanochat_env", &nanochat_env);

// stage 1.6: the model itself. If MODEL_CHECKPOINT has no loadable
// checkpoint, the agent TRAINS one (owner, 2026-08-16: "isn't that the
// whole point of the bootstrap?"). nanochat's own speedrun pipeline
// (dataset -> tokenizer -> base_train -> chat_sft), run from a script
// shipped as a library asset, in the background - this is GPU-hours,
// logged to runtime/agent/model/train.log, pidfile-guarded so repeat
// bootstraps report `running` instead of double-starting. The service
// launches regardless and sits in `waiting`, retrying its load every
// 60s, so verdicts begin on their own once base weights land. Training
// knobs come from NANOCHAT_TRAIN_ARGS. The default is sized for one
// consumer GPU (~32GB, no FlashAttention 3): --device-batch-size=8
// because base_train's own default of 32 is an 80GB-card number and
// OOMs a 5090 on step one, and --window-pattern=L because the SDPA
// fallback cannot do sliding windows (nanochat's own warning). The
// speedrun's 8xH100 scale is --depth=24 --device-batch-size=16 --fp8.
// chat_sft inherits device_batch_size from the pretrain meta, so one
// setting sizes both stages.
let mut training = "not_needed".to_string();
if checkpoint != "stub" && nanochat_env != "install_failed" {
    let ckpath = std::path::Path::new(&checkpoint);
    let has_ckpt = ["base_checkpoints", "chatsft_checkpoints", "chatrl_checkpoints"]
        .iter().any(|d| ckpath.join(d).is_dir())
        || ckpath.join("train_done").exists();
    if !has_ckpt {
        let pidfile = modeldir.join("train.pid");
        let mut already = false;
        if let Ok(pid) = std::fs::read_to_string(&pidfile) {
            let pid = pid.trim().to_string();
            if !pid.is_empty() && std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                already = true;
            }
        }
        if already {
            training = "running".to_string();
        } else {
            let tsh = modeldir.join("train.sh");
            let tasset = include_str!("../../../../data/agent/_ASSETS/train.sh");
            if std::fs::read_to_string(&tsh).unwrap_or_default() != tasset {
                if let Err(e) = std::fs::write(&tsh, tasset) {
                    return err(format!("could not write {}: {}", tsh.display(), e));
                }
            }
            let targs = prop("NANOCHAT_TRAIN_ARGS", "--depth=20 --device-batch-size=8 --window-pattern=L");
            let mut cmd = "cd ".to_string();
            cmd += &modeldir.display().to_string();
            cmd += &format!(
                "; nohup bash train.sh '{}' '{}' '{}' >> train.log 2>&1 & echo $! > train.pid",
                checkpoint, nc.display(), targs);
            let mut x = DataArray::new();
            x.push_string("bash");
            x.push_string("-c");
            x.push_string(&cmd);
            let r = system_call(x);
            println!("BOOTSTRAP START TRAINING {}", r.to_string());
            training = "started".to_string();
        }
    }
}
o.put_string("training", &training);

// stage 2: the service script, from the compiled-in asset
let svc = modeldir.join("service.py");
let asset = include_str!("../../../../data/agent/_ASSETS/service.py");
let current = std::fs::read_to_string(&svc).unwrap_or_default();
if current != asset {
    if let Err(e) = std::fs::write(&svc, asset) {
        return err(format!("could not write {}: {}", svc.display(), e));
    }
    o.put_boolean("script_written", true);
} else {
    o.put_boolean("script_written", false);
}

// stage 3: launch if it isn't answering
let status_url = format!("{}/status", service_url());
let probe = || ureq::AgentBuilder::new()
    .timeout(std::time::Duration::from_millis(800))
    .build()
    .get(&status_url)
    .call()
    .is_ok();
let mut service = "already_running".to_string();
// Converge a stale service: if one is answering but was started from an
// older script than the one just shipped (/status reports stale_script
// by comparing its file's mtime to its own start), kill it by the pid
// it reports and fall through to a fresh launch. Without this, a repo
// update leaves an old process holding the port and the report reads
// already_running while the behavior is last week's.
let mut was_stale = false;
if probe() {
    if let Ok(r) = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .get(&status_url)
        .call() {
        if let Ok(t) = r.into_string() {
            if let Ok(d) = DataObject::try_from_string(&t) {
                if matches!(d.try_get_boolean("stale_script"), Ok(true)) {
                    if let Ok(pid) = d.try_get_int("pid") {
                        let mut x = DataArray::new();
                        x.push_string("bash");
                        x.push_string("-c");
                        x.push_string(&format!("kill {} 2>/dev/null; sleep 0.6", pid));
                        let r = system_call(x);
                        println!("BOOTSTRAP RESTART STALE SERVICE pid {} {}", pid, r.to_string());
                        was_stale = true;
                    }
                }
            }
        }
    }
}
if was_stale || !probe() {
    // Real checkpoint: the venv python (torch et al) with PYTHONPATH at
    // the clone, since the nanochat package is deliberately uninstalled.
    let (py, envprefix) = if checkpoint == "stub" {
        ("python3".to_string(), "".to_string())
    } else {
        (nc.join("venv").join("bin").join("python").display().to_string(),
         format!("PYTHONPATH='{}' ", nc.display()))
    };
    let mut cmd = "cd ".to_string();
    cmd += &root.display().to_string();
    cmd += &format!(
        "; {}nohup '{}' runtime/agent/model/service.py --data-dir runtime/agent/model --port {} --checkpoint '{}' >> runtime/agent/model/service.log 2>&1 &",
        envprefix, py, port, checkpoint);
    let mut x = DataArray::new();
    x.push_string("bash");
    x.push_string("-c");
    x.push_string(&cmd);
    let r = system_call(x);
    println!("BOOTSTRAP LAUNCH MODEL SERVICE {}", r.to_string());
    // The service binds its port immediately and loads the scorer in
    // the background, so /status answers within a couple of seconds
    // even when a real checkpoint takes a minute to land - poll up to
    // 20s for the bind, then report the service's own view (mode may
    // legitimately be "loading"; a load failure shows as mode "error"
    // with boot_error, visible any time via service_status).
    let mut up = false;
    let mut waited = 0;
    while waited < 20000 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        waited += 500;
        if probe() { up = true; break; }
    }
    if up {
        service = if was_stale { "relaunched".to_string() } else { "launched".to_string() };
    } else {
        service = "launch_failed".to_string();
        o.put_string("status", "err");
    }
}
if service != "launch_failed" {
    if let Ok(r) = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .get(&status_url)
        .call() {
        if let Ok(t) = r.into_string() {
            if let Ok(d) = DataObject::try_from_string(&t) {
                if d.has("mode") { o.put_string("service_mode", &d.get_string("mode")); }
                if let Ok(be) = d.try_get_string("boot_error") {
                    o.put_string("boot_error", &be);
                    // `waiting` while training runs is the expected state,
                    // not a failure - only a load error with nothing
                    // training behind it is genuinely wrong.
                    if training == "not_needed" { o.put_string("status", "err"); }
                }
            }
        }
    }
}
o.put_string("service", &service);
o.put_string("checkpoint", &checkpoint);
o.put_string("port", &port);
o.put_string("path", &modeldir.display().to_string());
o
