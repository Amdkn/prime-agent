//! A'Space Prime Agent Adapter — D-N052 R2 (= F8 pas 6).
//!
//! This is by an automated agent, not a human.
//!
//! The adapter is the interface by which the Runtime calls prime-agent
//! as its recursion engine (H1) — A9. It mirrors the gel's contracts
//! (`74591cd`) without ever importing the fork: no `pa-*` dependency,
//! std only (H-M2). The Runtime composes the engine through this
//! interface, not through the fork.
//!
//! Modules mirror the gel:
//! - `rlm` — the Recursive Language Model: sub-agents called as
//!   functions in a persistent REPL (`rlm.spawn`, `rlm.collect`,
//!   `rlm.end_turn`; Rust side `RlmSpawnRequest`).
//! - `refine` — the Continual Harness: evidence-backed harness updates,
//!   session-local by default (`/refine`), with rollback.
//! - `runtime_surface` — the Python REPL as a surface, re-pointed at
//!   the single daemon (A2), never a second daemon (H-F8-2).
//! - `daemon_ref` — `pa-daemon` as a reference of its ACP wire
//!   interface (A1), never launched (H-F8-1).
//! - `lineage` — the measure zero and the upstream surface pins
//!   (H-M1/H-M2).

pub mod daemon_ref;
pub mod lineage;
pub mod refine;
pub mod rlm;
pub mod runtime_surface;
