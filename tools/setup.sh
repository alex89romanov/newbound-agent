#!/bin/sh
# One-command session setup: overlay this agent repo onto a newbound
# checkout, build everything, keep both git trees clean of builder-written
# local state, and probe the result.
#
#   path/to/newbound-agent/tools/setup.sh [path-to-newbound-checkout] [--no-probe]
#
# With no path argument the current directory is used if it is a newbound
# checkout; otherwise a `newbound` directory beside this repo.
#
# Idempotent: on a fully set-up checkout every step short-circuits and the
# whole run takes seconds. On a fresh clone it is the complete first-time
# sequence from the README (overlay -> cmd scaffold -> host build ->
# `newbound rebuild` -> host build -> dylibs), then the app staging the
# overlay otherwise misses (the platform's install_lib step: the server
# serves only apps listed in config.properties, and runtime/agent must
# carry the app shell), followed by the git hygiene that keeps regenerated
# local state out of accidental commits:
#
#   - newbound repo: Cargo.toml (workspace exclude), the generated
#     initializer, and newbound_core/src/api.rs are rewritten by
#     `newbound rebuild` and must never be committed there -> marked
#     skip-worktree (undo: git update-index --no-skip-worktree <file>).
#   - agent repo: the rebuild regenerates each FFI crate's src/api.rs
#     against the libraries present in THIS checkout, deleting stubs for
#     libraries that exist elsewhere (camera, hollis, ...). That churn is
#     environment-induced, not authored, so it is reverted -- unless the
#     file was already modified before setup ran, in which case it is
#     left alone as in-progress work.

set -e

AGENT_DIR=$(cd "$(dirname "$0")/.." && pwd)

NB=""
PROBE=yes
for arg in "$@"; do
  case "$arg" in
    --no-probe) PROBE=no ;;
    *) NB="$arg" ;;
  esac
done
if [ -z "$NB" ]; then
  if [ -f Cargo.toml ] && [ -d data ]; then
    NB=$(pwd)
  elif [ -d "$AGENT_DIR/../newbound/data" ]; then
    NB=$(cd "$AGENT_DIR/../newbound" && pwd)
  else
    echo "error: no newbound checkout found (pass its path, run from inside one, or keep one beside this repo)" >&2
    exit 1
  fi
fi
NB=$(cd "$NB" && pwd)
cd "$NB"
if [ ! -f Cargo.toml ] || [ ! -d data ]; then
  echo "error: $NB is not a newbound checkout (needs Cargo.toml and data/)" >&2
  exit 1
fi

echo "== setup: agent repo $AGENT_DIR onto checkout $NB"

# Remember which churn-prone agent-repo files carry pre-existing edits, so
# the hygiene step never reverts intentional work.
CHURN_FILES="agent/src/api.rs kb/src/api.rs"
DIRTY_BEFORE=$(cd "$AGENT_DIR" && git status --porcelain -- $CHURN_FILES)

# 1. Overlay (idempotent; also sets skip-worktree on the scratch skeleton).
"$AGENT_DIR/tools/overlay.sh" "$AGENT_DIR"

# 2. cmd crate scaffold, if this checkout ships without one.
if [ ! -d cmd/src ]; then
  "$AGENT_DIR/tools/gen-cmd-crate.py" .
fi

# Whether the host binary predates this run decides what can be said
# about MCP attachment at the end (step 10).
HOST_PREBUILT=no
[ -x target/release/newbound ] && HOST_PREBUILT=yes

# 2b. TRANSITIONAL - delete once flowlang 0.3.34 is on crates.io and the
#     pins are bumped (docs/ffi-dynamic-loading.md, Phase 5). The committed
#     platform code references flowlang::hotswap, which crates.io 0.3.33
#     lacks; a sibling mraiser/flow checkout provides it via a local-only
#     [patch] in .cargo/config.toml. Two copies because cargo resolves a
#     build cwd physically: the checkout's covers the host build, the agent
#     repo's covers the dylib builds reached through the overlay symlinks.
#     Both are untracked and per-clone excluded - never committed.
if grep -q 'flowlang = { version="0.3.33" }' Cargo.toml; then
  FLOW_DIR=""
  for cand in "$AGENT_DIR/../flow" "$NB/../flow"; do
    if [ -f "$cand/Cargo.toml" ] && grep -q '^name = "flowlang"' "$cand/Cargo.toml"; then
      FLOW_DIR=$(cd "$cand" && pwd); break
    fi
  done
  if [ -n "$FLOW_DIR" ]; then
    for root in "$NB" "$AGENT_DIR"; do
      if [ ! -f "$root/.cargo/config.toml" ]; then
        mkdir -p "$root/.cargo"
        printf '# LOCAL ONLY (setup.sh transitional): flowlang with hotswap until 0.3.34 ships\n[patch.crates-io]\nflowlang = { path = "%s" }\n' "$FLOW_DIR" > "$root/.cargo/config.toml"
      fi
      GD=$(cd "$root" && git rev-parse --absolute-git-dir 2>/dev/null || true)
      if [ -n "$GD" ]; then
        mkdir -p "$GD/info"
        grep -qx '/.cargo' "$GD/info/exclude" 2>/dev/null || echo '/.cargo' >> "$GD/info/exclude"
      fi
    done
    # Building under the patch rewrites Cargo.lock (the flowlang entry
    # becomes a path source); that churn is transitional local state too.
    git update-index --skip-worktree Cargo.lock 2>/dev/null || true
    echo "== transitional: flowlang patched to $FLOW_DIR via .cargo/config.toml (both roots)"
    echo "==   (undo when 0.3.34 ships: rm .cargo/config.toml; git update-index --no-skip-worktree Cargo.lock)"
  else
    echo "== WARNING: this tree references flowlang::hotswap, which crates.io flowlang 0.3.33 lacks;"
    echo "==   clone mraiser/flow beside the repos and rerun, or wait for the flowlang 0.3.34 release."
  fi
fi

# 3. Host build (first pass). No-op when already built.
cargo build --release --features=serde_support

# 4. One rebuild per checkout. The initializer is FFI-agnostic since the
#    hotswap upgrade (docs/ffi-dynamic-loading.md): FFI crates load from
#    store metadata at runtime, so nothing is baked in anymore. The rebuild
#    still writes the workspace exclude (dev.dev.compile's artifact probe
#    reads it) and regenerates crate scaffolds/api.rs against this store.
if ! grep -Eq 'exclude *= *\[.*"agent"' Cargo.toml \
  || ! grep -q 'hotswap::start' src/generated_initializer.rs 2>/dev/null; then
  ./target/release/newbound rebuild
  cargo build --release --features=serde_support
else
  echo "== workspace exclude and initializer are current; skipping rebuild"
fi

# 5. The dylibs (fast no-ops when unchanged; hot-load into a running server).
(cd agent && cargo build --release --features=serde_support,python_runtime)
(cd kb && cargo build --release)
(cd scratch && cargo build --release)

# 6. Stage the agent app — what the platform's install_lib does on a real
#    install, which the overlay never runs. All of it is per-clone local
#    state: runtime/agent is excluded via .git/info/exclude (the tracked
#    .gitignore belongs to the platform and doesn't know the agent), and
#    config.properties is already gitignored.
if [ ! -d runtime/agent ]; then
  cp -r data/agent/_APPS/agent runtime/agent
  echo "== staged data/agent/_APPS/agent -> runtime/agent"
fi
if ! grep -qx '/runtime/agent' .git/info/exclude 2>/dev/null; then
  echo '/runtime/agent' >> .git/info/exclude
  echo "== added /runtime/agent to .git/info/exclude"
fi
if [ ! -f config.properties ]; then
  sed 's/^apps=.*/&,agent/' config.properties_example > config.properties
  echo "== created config.properties from the example, agent app enabled"
elif grep -Eq '^apps=(.*,)?agent(,|$)' config.properties; then
  : # agent already enabled
elif grep -q '^apps=' config.properties; then
  sed -i 's/^apps=.*/&,agent/' config.properties
  echo "== added agent to the apps list in config.properties"
else
  echo 'apps=app,dev,security,peer,agent' >> config.properties
  echo "== added an apps list with agent to config.properties"
fi
if grep -q '^http_port=0$' config.properties; then
  # http_port=0 is never a choice anyone made in a dev checkout — it is
  # the residue of a bare `newbound mcp` run auto-creating the file.
  # Any other value (a deliberate port) is left alone.
  sed -i 's/^http_port=0$/http_port=8080/' config.properties
  echo "== set http_port=8080 in config.properties (was the mcp-run default 0 = no HTTP listener)"
fi

# 7. Git hygiene, newbound side: builder-written local state stays invisible.
for f in Cargo.toml src/generated_initializer.rs newbound_core/src/api.rs newbound_core/Cargo.toml; do
  git update-index --skip-worktree "$f" 2>/dev/null || true
done
echo "== skip-worktree set on the builder-written newbound files (undo: git update-index --no-skip-worktree <file>)"

# 8. Git hygiene, agent side: drop environment-induced api.rs regeneration.
(cd "$AGENT_DIR"
 for f in $CHURN_FILES; do
   case "$DIRTY_BEFORE" in
     *"$f"*) echo "== $f was already modified before setup; leaving it alone" ;;
     *) if [ -n "$(git status --porcelain -- "$f")" ]; then
          git checkout -- "$f"
          echo "== reverted environment-induced regeneration of $f"
        fi ;;
   esac
 done)

# 9. Top up the brain from the primer (docs/one-memory-cycle.md A4).
#    Idempotent (exact-claim dedupe) and best-effort: a fresh clone's brain
#    starts from the frozen kb snapshot, and the primer carries whatever
#    doctrine/process material was curated after the freeze.
if [ -f "$AGENT_DIR/docs/kb-seed.json" ]; then
  if "$AGENT_DIR/tools/nb-call.py" -C "$NB" agent-archivist-bootstrap \
       "{\"path\": \"$AGENT_DIR/docs/kb-seed.json\"}" >/dev/null 2>&1; then
    echo "== brain topped up from docs/kb-seed.json (idempotent)"
  else
    echo "== brain top-up skipped (bootstrap unavailable — run it once the binary serves mcp):"
    echo "==   tools/nb-call.py agent-archivist-bootstrap '{\"path\": \"$AGENT_DIR/docs/kb-seed.json\"}'"
  fi
fi

# 10. Prove it.
if [ "$PROBE" = yes ]; then
  "$AGENT_DIR/tools/overlay-probe.py" "$NB"
fi

echo "== setup complete: $NB serves the store via ./target/release/newbound mcp,"
echo "==   or run ./target/release/newbound for the web UI (agent app at /agent/index.html)"

# 11. Attachment sense. The server side is proven (the probe), but whether a
#     CLIENT holds the native attachment can't be read from here — harnesses
#     spawn .mcp.json servers only at their own startup. Infer what we can:
#     a live server process means some client is attached; none means none is.
MCP_PID=$(pgrep -f 'newbound mcp$' | head -1 || true)
if [ -n "$MCP_PID" ]; then
  echo "== MCP: a live 'newbound mcp' process exists (pid $MCP_PID) — a client is attached"
else
  if [ "$HOST_PREBUILT" = no ]; then
    echo "== MCP: no client is attached — none could be: the newbound binary was built by"
    echo "==   THIS run, and clients spawn .mcp.json servers only when they start."
  else
    echo "== MCP: no live 'newbound mcp' process — no client is currently attached."
  fi
  if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
    echo "==   This is a Claude Code web session: attachment happens only at session start"
    echo "==   and builds don't persist between sessions, so set the environment's setup"
    echo "==   script to newbound-agent/tools/setup.sh to make every session attach"
    echo "==   natively. Until then, drive the store with tools/nb-call.py (same surface)."
  else
    echo "==   Claude Code (CLI/IDE) reads this checkout's .mcp.json — restart it or"
    echo "==   reconnect via /mcp. Claude Desktop doesn't read .mcp.json; add the server"
    echo "==   to its config (Settings > Developer > Edit Config):"
    echo "==     {\"mcpServers\": {\"newbound\": {\"command\": \"/bin/sh\","
    echo "==       \"args\": [\"-c\", \"cd $NB && exec ./target/release/newbound mcp\"]}}}"
  fi
fi
