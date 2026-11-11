# A'Space Prime Agent Adapter — D-N052 R2 (= F8 pas 6)

This is by an automated agent, not a human.

Adapter of **prime-agent, the recursion** — the A'Space-native engine
interface that lets the Runtime call prime-agent as its recursion
engine (H1), without ever importing the fork.

## Anchors

| Item | Value |
|---|---|
| Fork | `Amdkn/prime-agent` ← `PrimeIntellect-ai/prime-agent` |
| Gel | `74591cddef28d11249c3171112114fc4755375d7` (2026-11-04T22:27:39Z, `rlm.end_turn()` — the sanctioned terminal call, #3830) |
| Bind | `3ba6791` = 1 file `aspace-plugin.json`, parent = gel (branch `aspace-v5-native`) |
| Drift | 21 behind upstream head `6aaa3a2` — revisit candidate, not a gate (A4) |
| Licence | MIT (file) |
| Version | 0.10.2, Rust edition 2021, workspace 10 crates |
| State | 0 → 2 (at integration) |
| Manifest | `fiber:a0:prime-agent` (A3) |

## What the adapter is

Rust, std only, in `aspace_prime_agent_adapter/`:

| Module | Mirrors (gel) | Native rule |
|---|---|---|
| `rlm.rs` | `RlmSpawnRequest` (rlm_host.rs:138) + `rlm.spawn` / `collect` / `end_turn` (rlm/__init__.py:212/:463/:196) | sub-agents as functions in a persistent REPL; bounded recursion (depth 5, 16 children, lease 300 s) |
| `refine.rs` | `plan_refinement` / `apply_refinement_plan` / `rollback_proposal` (executor.rs:194/:304, planner.rs:505) + skill `refine` | evidence-backed updates, Local by default; rollback is inverse edits in reverse order |
| `runtime_surface.rs` | `prime-agent-runtime/` (Python REPL) | surface re-pointed at the single daemon (A2), never a second daemon |
| `daemon_ref.rs` | `crates/pa-daemon` (ACP wire interface) | deposited as reference, never launched (A1) |
| `lineage.rs` | — | H-M1/H-M2 + 10 upstream surface pins |

`contract.json` is the public contract. `tests/smoke.rs` holds the
three smoke proofs, run by `cargo test` in this crate.

## What the adapter is not

- It never imports the fork. `pa-core`, `pa-agent`, `pa-daemon`,
  `pa-telemetry` are absent from the execution path (H-M2) — the
  crate has no `[dependencies]` section at all.
- It never calls a model endpoint and never reads a provider key:
  the model call is a closure supplied by the caller (scripted in
  tests, the Runtime's gateway provider in production).
- It never launches a daemon. `DaemonRef::launch_attempt` returns
  the refusal by construction (H-F8-1).
- It never carries the fork's telemetry: no analytics endpoint, no
  install-id minting — the fork's default-ON analytics
  (`https://api.primeintellect.ai/api/v1/agent-analytics/events`,
  `TelemetrySwitch::Default => true` in release builds) is abandoned
  with the fork (H-M4, see the telemetry audit).

## The three smoke proofs (A7)

1. **Spawn returns** — a recursive sub-agent is spawned as a
   function in the persistent REPL, and it returns (`rlm.spawn` +
   `rlm.collect`, settled, answer present).
2. **Refine applies, session-local** — an evidence-backed harness
   update is applied with scope Local by default (`/refine`).
3. **Refused update rolls back** — the harness returns to the
   previous state (rollback plan from the recorded before-snapshots);
   an unknown rollback id errors, never silently no-ops.

Plus the Runtime cycle with H1 (spawn → collect → refine from the
answer → end_turn) and the single-daemon / re-pointed-surface checks.

## Decisions carried (A1–A11)

- **A1** `pa-daemon` deposited as reference, never launched.
- **A2** Python runtime re-pointed as surface, not ported, never a
  second daemon.
- **A3** Manifest `fiber:a0:prime-agent`, state 2 from the start.
- **A4** Drift 7 → 21 is a revisit candidate, not a gate, no re-gel.
- **A5** Ceilings 1 % / 5 % / 20 % on the 5.5 Md pool.
- **A6** Reserve R-1 (TabFM PARTIAL test) non-gate, test untouched.
- **A7** Smoke entry points confirmed at execution (this file's
  table), not invented.
- **A8** Telemetry audit: every trigger decided (see
  `prime-agent-TELEMETRY-AUDIT.md` in the R2 evidence).
- **A9** Composition by the adapter (this crate is the interface).
- **A10** Pin updated to the adapter (by Ryan, after the merge).
- **A11** One development, two uses — the R2 PR is the pas 6 PR.

## Run

```sh
cargo test          # suite + smoke
cargo fmt --check   # format gate
cargo clippy --all-targets -- -D warnings   # lint gate
```
