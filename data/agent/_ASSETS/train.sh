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
echo "=== nanochat training into $BASE_DIR on $NGPU GPU(s); args: ${EXTRA_ARGS:-defaults} ==="
python -m nanochat.dataset -n 8
python -m nanochat.dataset -n 170 &
DL_PID=$!
python -m scripts.tok_train
echo "=== tokenizer done; waiting for dataset download... ==="
wait $DL_PID
torchrun --standalone --nproc_per_node=$NGPU -m scripts.base_train -- --run=$WANDB_RUN $EXTRA_ARGS
echo "=== base training done; SFT... ==="
torchrun --standalone --nproc_per_node=$NGPU -m scripts.chat_sft -- --run=$WANDB_RUN
touch "$BASE_DIR/train_done"
echo "=== training complete: $BASE_DIR ==="
