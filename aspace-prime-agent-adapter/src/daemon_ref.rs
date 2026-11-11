//! `pa-daemon` as a reference — deposited, never launched.
//!
//! A1: prime-agent has a daemon (`crates/pa-daemon`, README + src +
//! tests + testdata). The Runtime has a single daemon (RigInfinity —
//! F8 D8). Two daemons do not coexist in the Runtime. A daemon is not
//! a surface — it is not "re-pointed", it is launched or not launched.
//! `pa-daemon` is deposited as reference code (here, as the
//! documentation of its ACP wire interface) and never launched. The
//! single daemon (RigInfinity) is the only daemon that runs (H-F8-1).
//!
//! The reference (from `crates/pa-daemon/README.md`, at the gel):
//! - ACP stdio transport: the JSON-RPC serve surface for Agent Client
//!   Protocol clients — `initialize`, `session/new`, `session/prompt`,
//!   `session/close`, `session/cancel`, and the outgoing
//!   `session/update` notification with namespaced `_meta` correlation.
//! - Supervisor process: one worker process per active session,
//!   restart/backoff supervision, session registry/roster, worker
//!   self-registration (session identity survives supervisor
//!   restarts), parent-death child cleanup.
//! - Append-only session store (JSONL, same layout as
//!   `~/.prime/agent/sessions`), client attach/detach (full-snapshot
//!   and chunked streaming), direct-attach transport (supervisor-
//!   issued single-use tickets, 10 s TTL).
//! - Worker turn loop: compaction arms, auto-refine, goal
//!   continuation; kernel `agent_message` / `agent_observe` bridge;
//!   worker-to-worker peer messaging.
//!
//! This module contains no process-spawning call. The only "launch"
//! path is `DaemonRef::launch_attempt`, which returns the refusal by
//! construction — the reference cannot be launched through this
//! adapter.

/// The ACP methods the daemon serves (the wire interface, as
/// reference — not a client).
pub const ACP_METHODS: &[&str] = &[
    "initialize",
    "session/new",
    "session/prompt",
    "session/close",
    "session/cancel",
    "session/update",
];

/// The daemon as reference code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRef {
    /// The crate the reference documents.
    pub krate: &'static str,
    /// The wire protocol it serves.
    pub protocol: &'static str,
    /// The methods it serves, as reference.
    pub methods: &'static [&'static str],
}

impl DaemonRef {
    /// The `pa-daemon` reference (A1).
    #[must_use]
    pub fn pa_daemon() -> Self {
        Self {
            krate: "pa-daemon",
            protocol: "ACP (Agent Client Protocol, stdio JSON-RPC)",
            methods: ACP_METHODS,
        }
    }

    /// Attempt to launch the daemon through the adapter: always
    /// refused. The reference is documentation, not a process.
    pub fn launch_attempt(&self) -> Result<(), DaemonError> {
        Err(DaemonError::NeverLaunched { krate: self.krate })
    }
}

/// What the daemon reference refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonError {
    /// The daemon is a reference, never launched (A1, H-F8-1).
    NeverLaunched { krate: &'static str },
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeverLaunched { krate } => write!(
                f,
                "{krate} is deposited as reference code and never launched; the single daemon is RigInfinity"
            ),
        }
    }
}

impl std::error::Error for DaemonError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_daemon_reference_cannot_be_launched() {
        let daemon = DaemonRef::pa_daemon();
        assert_eq!(
            daemon.launch_attempt(),
            Err(DaemonError::NeverLaunched { krate: "pa-daemon" })
        );
        assert!(daemon.methods.contains(&"session/prompt"));
        assert_eq!(daemon.methods.len(), 6);
    }
}
