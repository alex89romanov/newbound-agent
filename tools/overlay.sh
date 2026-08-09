#!/bin/sh
# Overlay this agent repo onto a newbound checkout via symlinks.
#
# Run from the newbound checkout root:
#   path/to/newbound-agent/tools/overlay.sh path/to/newbound-agent
#
# Idempotent: existing correct links are left alone; a real directory in
# the way is an error (never deleted).

set -e

AGENT_DIR="$1"
if [ -z "$AGENT_DIR" ]; then
  echo "usage: overlay.sh <path-to-newbound-agent>" >&2
  exit 1
fi
AGENT_DIR=$(cd "$AGENT_DIR" && pwd)

if [ ! -d "data" ] || [ ! -f "Cargo.toml" ]; then
  echo "error: run from a newbound checkout root (needs data/ and Cargo.toml)" >&2
  exit 1
fi

link() { # link <checkout-relative-path> <agent-repo-relative-path>
  src="$AGENT_DIR/$2"
  dst="$1"
  if [ -L "$dst" ]; then
    ln -sfn "$src" "$dst"
  elif [ -e "$dst" ]; then
    echo "error: $dst exists and is not a symlink — refusing to touch it" >&2
    exit 1
  else
    ln -s "$src" "$dst"
  fi
  echo "  $dst -> $src"
}

echo "overlaying $AGENT_DIR:"
link data/agent   data/agent
link data/kb      data/kb
link data/scratch data/scratch
link agent        agent
link kb           kb
link scratch      scratch

# Silence runtime mutations of the tracked scratch skeleton (per-clone).
(cd "$AGENT_DIR" && git ls-files -z data/scratch scratch \
  | xargs -0 -r git update-index --skip-worktree \
  && echo "  skip-worktree set on the scratch skeleton")

echo "done. Build the host, then the agent/kb dylibs (see README)."
