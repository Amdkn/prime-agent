# AGENTS.md — aspace-prime-agent-adapter (D-N052 R2)

House rules for this adapter crate. It is a standalone crate (its own
`[workspace]`), not a member of the fork workspace.

- **Std only.** No `[dependencies]` section, no `pa-*` import, no
  fork import of any kind (H-M2). If a dependency looks necessary,
  stop: the interface is wrong, not the dependency list.
- **No model call, no provider key.** The model is a closure supplied
  by the caller (scripted in tests, the Runtime gateway in
  production). No endpoint string in this crate (H-M4) — the lineage
  test scans the sources and fails on `primeintellect.ai`.
- **No daemon launch.** `DaemonRef::launch_attempt` returns the
  refusal by construction (A1). No `std::process::Command` anywhere
  in this crate.
- **No second daemon.** The Python runtime is a surface, checked,
  never launched (A2).
- **No evidence logs committed here.** The fiche, manifeste and audit
  live in the R2 evidence directory (`v4-factory/evidence/D-N052/r2/`),
  not in the fork.
- **Gates (run before every commit):** `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- **Files under 2,000 lines; modules named for what they own.**
- **No `Co-authored-by` trailers.** Commit message starts with
  `This is by an automated agent, not a human.`
- **No test without a runner:** every test file is run by `cargo test`.
