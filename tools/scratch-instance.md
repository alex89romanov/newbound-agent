# Disposable scratch instance

All write-path testing happens against a throwaway copy of the platform —
never the primary instance. The recipe below is the one that actually works
(deleting the whole `runtime/` folder, as originally suggested, hangs startup:
peer init waits forever on `security_ready` because the app bootstrap needs
`runtime/<app>/app.properties`).

```bash
rsync -a --exclude .git --exclude target --exclude graphify-out \
  /home/gm/Desktop/Marc/newbound/ /home/gm/Desktop/Marc/newbound-scratch/
mkdir -p /home/gm/Desktop/Marc/newbound-scratch/target/release
cp /home/gm/Desktop/Marc/newbound/target/release/newbound \
   /home/gm/Desktop/Marc/newbound-scratch/target/release/
# fresh P2P identity + fresh ports, without breaking app bootstrap:
rm -f /home/gm/Desktop/Marc/newbound-scratch/runtime/peer/botd.properties \
      /home/gm/Desktop/Marc/newbound-scratch/runtime/app/botd.properties
sed -i 's/^http_port=.*/http_port=33182/' /home/gm/Desktop/Marc/newbound-scratch/config.properties
# the bench is a published app (DESIGN 7.3 as-built) — put its lib in apps=
# at creation and no restart is ever needed to serve /bench/:
sed -i 's/^apps=\(.*\)/apps=\1,bench/' /home/gm/Desktop/Marc/newbound-scratch/config.properties
```

Start it:

```bash
cd /home/gm/Desktop/Marc/newbound-scratch && ./target/release/newbound
```

It comes up with HTTP on 33182 and a freshly assigned P2P port/identity.
Login for API use: any fresh session id in the `sessionid` cookie +
`GET /app/login?user=admin&pass=<from newbound-scratch/users/admin.properties>`.

**`runtime/metaidentity` seeds itself** — `dev.code.init` (timer-fired
once at boot, like `security.security.init`) creates the record with the
platform's historical default identity, "Some Dev", whenever it is absent
— which it is on every clone-built instance, since the repo gitignores
`/data/runtime` (identity keys — redaction working as intended). So a
disposable publishes out of the box; no hand-seeding, and `publishapp`'s
missing-record panic can't be reached from a normal boot. Change the
identity in the workbench publish pane (or `dev-code-set_meta_identity`)
if the test needs a specific name. Only a binary older than `dev.code.init`
still needs the old hand-seed of a minimal record.

Installing the bench on it: `python3 tools/install-bench.py --base
http://localhost:33182 --user admin --password <pw> --disposable --publish`
(`--publish` needed on the first run — it runs `publishapp` on the `bench`
control; add `--prune-assets` when reusing a pre-port install), then sign
in via the instance's own UI and open `http://localhost:33182/bench/`.

**The notebook agent** (ask ▸) additionally needs the **agent app exposed
and pointed at an LLM** — `chat_llm` reads `system.apps.agent.runtime`,
which only exists for apps in the `apps=` list (its `runtime` object is
`runtime/agent/botd.properties`). Without it the ask errors with
"Key 'agent' not found". Setup:

```bash
sed -i 's/^apps=\(.*\)/apps=\1,agent/' $S/config.properties
mkdir -p $S/runtime/agent
cat >> $S/runtime/agent/botd.properties <<'EOF'
VLLM_URL=http://<your-vllm-host>:<port>/v1/chat/completions
VLLM_MODEL=<the served model name>
EOF
# restart the instance (apps= is read at startup)
```

Copy the two values from the primary's `runtime/agent/botd.properties`.

Rules:

- Mutating commands (`app/exec` on save commands, `app/write`, `dev/*` builds)
  go **only** to the scratch instance's port. In the bench UI this means:
  the "saves ON" toggle in the connect dialog is only ever combined with
  `http://localhost:33182`.
- The scratch copy is disposable: if a write test corrupts it, delete the
  folder and re-copy. Never "fix" a corrupted scratch store by hand-editing
  `data/` — that's the failure mode this whole architecture exists to avoid.
- Re-copy from the primary whenever you need current state.

## The sandbox variant (Claude's environment — proven 2026-07-29)

The mirror builds and runs in the Claude sandbox, so disposable-instance
work (validator runs, install syncs, browser E2Es against a real
platform) no longer needs the owner's machine. Differences from the
desktop recipe above:

```bash
# build (registry works through the sandbox proxy; ~20s each)
cd /workspace/newbound && cargo build --release --features="serde_support"
cd agent && cargo build --release --features="serde_support,python_runtime"

# copy (no rsync in the sandbox — tar instead), place binaries
S=/workspace/nb-scratch; mkdir -p $S
tar -C /workspace/newbound --exclude='./.git' --exclude='./target' \
    --exclude='./agent/target' --exclude='./graphify-out' -cf - . | tar -C $S -xf -
mkdir -p $S/target/release $S/agent/target/release
cp /workspace/newbound/target/release/newbound $S/target/release/
cp /workspace/newbound/agent/target/release/libagent.so $S/agent/target/release/

# config: the mirror ships only config.properties_example (the real one is
# redacted) — create one with a fresh port and bench pre-exposed
sed -e 's/^http_port=.*/http_port=33199/' \
    -e 's/^apps=.*/apps=app,dev,security,peer,bench/' \
    $S/config.properties_example > $S/config.properties

# boot detached (metaidentity self-seeds at boot — see above)
cd $S && setsid nohup ./target/release/newbound > boot.log 2>&1 < /dev/null &
```

No admin secret is needed: first boot generates `users/admin.properties`
(random password) — read it from the file and `GET /app/login`. P2P binds
local ports and finds no peers; harmless. Browser E2Es: saves-ON is the
localStorage `bench.connection` object with `writable:true` and
`baseUrl` = the instance origin; the flow route takes control/command
IDS (`#/flow/<lib>/<ctlId>/<cmdId>`), not names.
