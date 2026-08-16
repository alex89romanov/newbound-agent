#!/usr/bin/env bash
# Train a nanochat model into $1 (the NANOCHAT_BASE_DIR) using the venv
# and clone that agent-model-bootstrap built. Bootstrap writes this file
# from the compiled-in library asset and launches it in the background
# when MODEL_CHECKPOINT points at a directory with no loadable
# checkpoint - the agent builds its own model. This is nanochat's own
# speedrun pipeline (dataset -> tokenizer -> base_train -> chat_sft),
# with the GPU count auto-detected and the training knobs passed in as
# $3 (from the NANOCHAT_TRAIN_ARGS setting). Logs land in train.log
# beside this script; train_done is touched in the base dir on success.
set -e
set -o pipefail
BASE_DIR="$1"
CLONE="$2"
EXTRA_ARGS="${3:-}"
DIST_SPEC="${4:-}"   # NANOCHAT_DIST: nnodes=2,rank=0,master=IP:PORT,iface=IF
export NANOCHAT_BASE_DIR="$BASE_DIR"
export OMP_NUM_THREADS=1
export WANDB_RUN=dummy
# Reduces fragmentation-driven OOM on tightly-fitting consumer GPUs
export PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
mkdir -p "$BASE_DIR"
cd "$CLONE"
source venv/bin/activate
NGPU=$(nvidia-smi -L 2>/dev/null | wc -l)
if [ "$NGPU" -lt 1 ]; then
  echo "FATAL: no CUDA GPUs visible (nvidia-smi found none) - training needs a GPU"
  exit 1
fi
# Multi-node mode (NANOCHAT_DIST, e.g. a DGX Spark pair over its
# ConnectX link): every node runs this same script with its own rank.
# The NANOCHAT_BASE_DIR must hold the same data on every node - share
# it over NFS (downloads are filelock-guarded, so concurrent nodes
# dedupe) or point at pre-synced local copies. Rank 0 trains the
# tokenizer; other ranks download data and wait for it to appear, then
# every node joins the torchrun rendezvous.
RANK=0
TORCHRUN_ARGS="--standalone --nproc_per_node=$NGPU"
if [ -n "$DIST_SPEC" ]; then
  NN=""; MASTER=""; IFACE=""
  IFS=','
  for kv in $DIST_SPEC; do
    case "$kv" in
      nnodes=*) NN="${kv#nnodes=}";;
      rank=*)   RANK="${kv#rank=}";;
      master=*) MASTER="${kv#master=}";;
      iface=*)  IFACE="${kv#iface=}";;
    esac
  done
  unset IFS
  ADDR="${MASTER%:*}"; PORT="${MASTER##*:}"
  if [ -z "$NN" ] || [ -z "$MASTER" ] || [ "$ADDR" = "$PORT" ]; then
    echo "FATAL: NANOCHAT_DIST needs nnodes=, rank=, master=IP:PORT (got '$DIST_SPEC')"
    exit 1
  fi
  if [ -n "$IFACE" ]; then export NCCL_SOCKET_IFNAME="$IFACE"; fi
  TORCHRUN_ARGS="--nnodes=$NN --node-rank=$RANK --master-addr=$ADDR --master-port=$PORT --nproc_per_node=$NGPU"
  echo "=== distributed: $TORCHRUN_ARGS (NCCL_SOCKET_IFNAME=${IFACE:-unset}) ==="
fi
echo "=== nanochat training into $BASE_DIR on $NGPU GPU(s) rank $RANK; args: ${EXTRA_ARGS:-defaults} ==="
python -m nanochat.dataset -n 8
python -m nanochat.dataset -n 170 &
DL_PID=$!
if [ "$RANK" = "0" ]; then
  python -m scripts.tok_train
  echo "=== tokenizer done; waiting for dataset download... ==="
else
  echo "=== waiting for rank 0's tokenizer... ==="
  while [ ! -f "$BASE_DIR/tokenizer/tokenizer.pkl" ] || [ ! -f "$BASE_DIR/tokenizer/token_bytes.pt" ]; do
    sleep 5
  done
fi
wait $DL_PID
torchrun $TORCHRUN_ARGS -m scripts.base_train -- --run=$WANDB_RUN $EXTRA_ARGS
echo "=== base training done; SFT... ==="
torchrun $TORCHRUN_ARGS -m scripts.chat_sft -- --run=$WANDB_RUN
if [ "$RANK" = "0" ]; then touch "$BASE_DIR/train_done"; fi
echo "=== training complete: $BASE_DIR ==="
