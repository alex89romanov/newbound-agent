# Spectrum S8 — SFT joins the loop

**Status: gate design + implementation notes for branch
`claude/spectrum-s8-sft`, from the ruled charter
(`docs/spectrum-cycle.md` S8).** The phase the harvest charter
deferred with the condition that its gate design be written before
code. This page's first section IS that page; the code below it came
after.

## The gate design (written first)

An SFT run turns a banked conversation dataset (H1's chat bank, or
any registered `sft`/`persona`-kind dataset) into a candidate for the
USER lane — the stricter lane, the one that talks to people. The
design question the harvest charter deferred: what must that
candidate prove, and to whom?

**The answer in one sentence: the SFT gate is the bench's instruments
guarding the ENTRY to the ring, and everything after the ring is the
shipped soak-and-user-gate machinery, unchanged.**

1. **The candidate is a delta.** An SFT run trains a bounded
   hook-LoRA delta (rank an argument, default above persona's) over a
   fresh load of its base — the pointer's base or any registered
   model — with the masked conversation loss (assistant tokens only,
   the persona derivation's loss at dataset scale). It borrows the
   trainer's time-share like a bench run: candidate steps pause,
   serving never does, the borrow is published. Borrowing also
   freezes the ring during the run, so the delta's base reference
   stays true.

2. **Three instruments, measured bare-then-adapted on the same
   frozen material** (the charter: "the first question about an
   SFT'd candidate is a bench question"):
   - **Subject gain** — held-out SFT conversations' masked loss must
     improve by `min_gain`: did it actually learn the bank?
   - **The forgetting guard** — anchor/standard loss within `guard`:
     a chat voice must not lobotomize the base (ruling 2's anchor
     resolves through the manager as everywhere).
   - **Agreement non-regression** — generated-verdict agreement on
     the held-out frontier pairs must not fall more than `slack`:
     when one resident serves both lanes, learning to chat must not
     unlearn the judge's dialect. (No pairs banked yet = not
     measurable = not gating, reported as such.)
   Reject → a report and nothing else. Accept → the delta lands in
   the ring as `sft-<ts>-<name>`, and the report lands in the bench's
   ledger (experiments.jsonl) either way.

3. **The ring namespace is the safety.** CPT deltas live in `cpt-*`
   and are BY DEFINITION already merged into serving — restart resume
   re-merges them. An accepted SFT candidate is NOT yet serving, so
   it lives in `sft-*`, which resume never touches. It becomes the
   serving salience pointer only by a deliberate promote-by-key
   (`agent-model-sft_promote`), which loads base + current CPT state
   + the candidate delta into the inactive slot and flips — the
   double buffer, unchanged.

4. **From there, everything is shipped machinery.** The promoted
   candidate soaks on the fast lane under its `sft-*` key; the
   standard user gate (soak_s, verdicts, agreement, no-regress)
   qualifies it READY; `/user_promote` advances the user pointer onto
   it; persona and the adapter stack re-apply on top as they do for
   any pointer; rollback is the same one step back. No new gate, no
   new pointer semantics — the SFT gate bought its way INTO the
   pipeline the 8a/8b design already trusts, and nothing downstream
   knows the candidate's lessons came from a chat bank.

What this deliberately does not do: no full-parameter SFT (the delta
is the candidate at every scale until the solver's full rung learns
hf saves), no auto-promotion past the SFT gate (entering the ring is
the gate's whole authority), no training on raw logs (the bank is
curated conversations — the feed contract's rules hold).

## Implementation notes

- `agent-model-sft_run {name, dataset, base, rank, steps}` →
  `/sft_run` (background; `/status.sft` carries progress; the report
  appends to experiments.jsonl with `sft: true`).
- `agent-model-sft_promote {checkpoint}` → `/promote` with a key —
  the deliberate act that puts an accepted candidate on the fast
  lane for its soak.
- Both trainer rungs' borrow guard now yields to SFT runs as well as
  bench runs.
- `load_pointer_scorer` learned the `sft-*` namespace: base + all
  `cpt-*` deltas + the named candidate. The user pointer's persisted
  key round-trips through restarts like any other.

## Battery (CPU, tiny model — see the manual's S8 claim for results)

Accept path: fabricated 24-conversation sft dataset → sft_run →
gate report (three instruments, bare vs adapted) → `sft-*` ring
entry → restart does NOT auto-merge it → sft_promote → fast lane
serves base+candidate → soak → READY → /user_promote → /chat.
Reject path: a 2-step run fails min_gain → report, no ring entry.
R1 restored.

## Merge word

`Merge: S8 - SFT joins the loop`.
