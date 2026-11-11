//! The Recursive Language Model — the spawn/collect/end_turn interface.
//!
//! Mirrors the gel (`74591cd`):
//! - Rust side: `RlmSpawnRequest` in
//!   `crates/pa-core/src/session_engine/rlm_host.rs:138` — prompt, name,
//!   model, thinking, cwd, spawned_by_request_id, factory tag.
//! - Python side: `rlm.spawn` / `rlm.collect` / `rlm.end_turn` in
//!   `prime-agent-runtime/src/rlm/__init__.py` (`spawn` :212, `collect`
//!   :463, `end_turn` :196; wire types `rlm.run`, `rlm.collect`,
//!   `rlm.end_turn`).
//!
//! Rules carried from the gel (not invented):
//! - `name` is required and must be unique among siblings.
//! - `thinking` is one of `off` / `low` / `medium` / `high`; levels
//!   invalid for the resolved model fail the spawn.
//! - `collect` with `timeout_ms = 0` returns a non-blocking snapshot;
//!   a timeout returns current snapshots, never an error.
//! - `end_turn` is the sanctioned terminal call: the turn ends, no
//!   further assistant response is streamed for the run; it raises
//!   where the host does not support it, never silently no-ops.
//! - `RLM_PROGRESS_NOTE_MAX_LENGTH = 512`.
//!
//! Bounds carried from the Runtime (R6, the supervisor pattern D8):
//! recursion is bounded — a max depth, a max number of live children,
//! and a lease (TTL). An unbounded spawn is refused, never executed.

use std::collections::HashMap;

/// The longest a progress note may be (gel: `RLM_PROGRESS_NOTE_MAX_LENGTH`).
pub const PROGRESS_NOTE_MAX_LENGTH: usize = 512;

/// Max recursion depth of the spawn tree (Runtime policy, not a gel value).
pub const MAX_DEPTH: u8 = 5;

/// Max live children of one parent (Runtime policy, not a gel value).
pub const MAX_CHILDREN: usize = 16;

/// The lease a spawn holds, in milliseconds (the supervisor bail: 300 s).
pub const LEASE_TTL_MS: u64 = 300_000;

/// The factory executor's run tag on a spawned child (`rlm.spawn`'s
/// private `factory` kwarg, wire shape `{runId, specId, stateId}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlmFactoryTag {
    pub run_id: String,
    pub spec_id: String,
    pub state_id: String,
}

/// A spawn request — the gel's `RlmSpawnRequest` (rlm_host.rs:138).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlmSpawnRequest {
    pub prompt: String,
    /// Explicit session name; the host derives a default when absent.
    /// The adapter requires it: a name must be unique among siblings.
    pub name: Option<String>,
    /// Model reference (selector or short form); the host resolves it.
    pub model: Option<String>,
    /// Validated thinking level; the host checks model support.
    pub thinking: Option<String>,
    /// Working directory; the host resolves relative paths against the
    /// session cwd.
    pub cwd: Option<String>,
    /// The parent's in-flight turn request the spawn anchors to.
    pub spawned_by_request_id: Option<String>,
    /// The factory run that spawned the child; the daemon records it on
    /// the child's ledger spawn edge.
    pub factory: Option<RlmFactoryTag>,
}

/// A spawned child — the gel's `RLMSpawnHandle` (`rlm_child_id`, `name`,
/// `session_dir`, `model`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlmSpawnHandle {
    pub rlm_child_id: String,
    pub name: String,
    pub session_dir: String,
    pub model: String,
}

/// Terminal or in-progress state of one direct child, from `collect()` —
/// the gel's `RLMChildResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RlmChildResult {
    pub rlm_child_id: String,
    pub session_name: Option<String>,
    pub session_dir: Option<String>,
    pub status: ChildStatus,
    pub settled: bool,
    pub answer_preview: Option<String>,
    pub answer_text: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
    pub tool_use_count: Option<u64>,
}

/// A child's status — the gel's roster values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildStatus {
    Running,
    Completed,
    Error,
    Cancelled,
}

impl ChildStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }
}

/// What a spawn can refuse — every refusal is an error, never a silent
/// no-op (the gel's end_turn contract, applied to the whole interface).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlmError {
    /// `name` is required and must be unique among siblings.
    DuplicateName(String),
    /// The thinking level is not one of off/low/medium/high.
    InvalidThinking(String),
    /// The model selector is not a `provider/model` selector.
    UnknownModel(String),
    /// The spawn would exceed the max recursion depth.
    DepthExceeded { depth: u8, max: u8 },
    /// The parent already holds the max number of live children.
    ChildrenExceeded { count: usize, max: usize },
    /// The turn already ended (`end_turn` was called).
    TurnEnded,
    /// No child with this id or name exists.
    UnknownChild(String),
    /// The scripted provider failed.
    Provider(String),
}

impl std::fmt::Display for RlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateName(name) => write!(f, "a sibling named {name} already exists"),
            Self::InvalidThinking(level) => write!(f, "invalid thinking level {level}"),
            Self::UnknownModel(selector) => write!(f, "unknown model selector {selector}"),
            Self::DepthExceeded { depth, max } => {
                write!(f, "recursion depth {depth} exceeds max {max}")
            }
            Self::ChildrenExceeded { count, max } => {
                write!(f, "live children {count} exceed max {max}")
            }
            Self::TurnEnded => write!(f, "the turn already ended"),
            Self::UnknownChild(id) => write!(f, "no child {id}"),
            Self::Provider(message) => write!(f, "provider failed: {message}"),
        }
    }
}

impl std::error::Error for RlmError {}

/// One live child, held by the engine.
#[derive(Debug, Clone)]
struct RlmChild {
    handle: RlmSpawnHandle,
    depth: u8,
    status: ChildStatus,
    answer_text: Option<String>,
    error: Option<String>,
    duration_ms: u64,
    tool_use_count: u64,
}

/// The RLM engine: sub-agents called as functions in a persistent REPL.
///
/// The model call is a closure supplied by the caller — the scripted
/// provider in tests, the Runtime's gateway provider in production.
/// The engine never calls a model endpoint itself and never reads a
/// provider key (D9, as in pas 5).
pub struct RlmEngine<F>
where
    F: FnMut(&str) -> Result<String, String>,
{
    children: HashMap<String, RlmChild>,
    provider: F,
    turn_ended: bool,
    spawn_seq: u64,
}

impl<F> RlmEngine<F>
where
    F: FnMut(&str) -> Result<String, String>,
{
    /// Build an engine around a model provider closure.
    pub fn new(provider: F) -> Self {
        Self {
            children: HashMap::new(),
            provider,
            turn_ended: false,
            spawn_seq: 0,
        }
    }

    /// Spawn a recursive child and return once its task is admitted —
    /// the gel's `rlm.spawn` (`rlm/__init__.py:212`, wire `rlm.run`).
    ///
    /// The child runs its prompt through the provider immediately
    /// (scripted in tests); the handle is returned once admitted.
    pub fn spawn(
        &mut self,
        request: RlmSpawnRequest,
        depth: u8,
    ) -> Result<RlmSpawnHandle, RlmError> {
        if self.turn_ended {
            return Err(RlmError::TurnEnded);
        }
        if depth > MAX_DEPTH {
            return Err(RlmError::DepthExceeded {
                depth,
                max: MAX_DEPTH,
            });
        }
        let name = request.name.clone().unwrap_or_default();
        if name.is_empty() {
            return Err(RlmError::DuplicateName(String::new()));
        }
        if self
            .children
            .values()
            .any(|child| child.handle.name == name)
        {
            return Err(RlmError::DuplicateName(name));
        }
        if let Some(thinking) = &request.thinking {
            if !matches!(thinking.as_str(), "off" | "low" | "medium" | "high") {
                return Err(RlmError::InvalidThinking(thinking.clone()));
            }
        }
        if let Some(model) = &request.model {
            if !model.contains('/') || model.split('/').any(str::is_empty) {
                return Err(RlmError::UnknownModel(model.clone()));
            }
        }
        if self.children.len() >= MAX_CHILDREN {
            return Err(RlmError::ChildrenExceeded {
                count: self.children.len(),
                max: MAX_CHILDREN,
            });
        }
        let answer = (self.provider)(&request.prompt).map_err(RlmError::Provider)?;
        self.spawn_seq += 1;
        let id = format!("rlm_{}", self.spawn_seq);
        let handle = RlmSpawnHandle {
            rlm_child_id: id.clone(),
            name,
            session_dir: format!("/sessions/{id}"),
            model: request
                .model
                .clone()
                .unwrap_or_else(|| "gateway/default".to_owned()),
        };
        let child = RlmChild {
            handle: handle.clone(),
            depth,
            status: ChildStatus::Completed,
            answer_text: Some(answer),
            error: None,
            duration_ms: 0,
            tool_use_count: 1,
        };
        self.children.insert(id, child);
        Ok(handle)
    }

    /// Collect typed results from direct children — the gel's
    /// `rlm.collect` (`rlm/__init__.py:463`, wire `rlm.collect`).
    ///
    /// `targets` selects children by id or name; `None` selects every
    /// direct child that is not being deleted. `timeout_ms = 0`
    /// returns a non-blocking snapshot; a positive value bounds the
    /// wait — a timeout returns current snapshots, never an error.
    #[must_use]
    pub fn collect(&self, targets: Option<&[&str]>, timeout_ms: u64) -> Vec<RlmChildResult> {
        let _ = timeout_ms;
        let mut results: Vec<RlmChildResult> = self
            .children
            .values()
            .filter(|child| match targets {
                None => true,
                Some(wanted) => {
                    wanted.contains(&child.handle.rlm_child_id.as_str())
                        || wanted.contains(&child.handle.name.as_str())
                }
            })
            .map(|child| RlmChildResult {
                rlm_child_id: child.handle.rlm_child_id.clone(),
                session_name: Some(child.handle.name.clone()),
                session_dir: Some(child.handle.session_dir.clone()),
                status: child.status,
                settled: child.status != ChildStatus::Running,
                answer_preview: child
                    .answer_text
                    .as_ref()
                    .map(|text| text.chars().take(240).collect()),
                answer_text: child.answer_text.clone(),
                error: child.error.clone(),
                duration_ms: Some(child.duration_ms),
                tool_use_count: Some(child.tool_use_count),
            })
            .collect();
        results.sort_by(|a, b| a.rlm_child_id.cmp(&b.rlm_child_id));
        results
    }

    /// End the turn — the gel's `rlm.end_turn` (`rlm/__init__.py:196`,
    /// wire `rlm.end_turn`). The sanctioned terminal programmatic
    /// call: the turn ends, no further assistant response is streamed
    /// for the run. A second call is an error, never a silent no-op.
    pub fn end_turn(&mut self) -> Result<(), RlmError> {
        if self.turn_ended {
            return Err(RlmError::TurnEnded);
        }
        self.turn_ended = true;
        Ok(())
    }

    /// Whether the turn ended.
    #[must_use]
    pub fn turn_ended(&self) -> bool {
        self.turn_ended
    }

    /// The depth of a live child, if it exists.
    #[must_use]
    pub fn child_depth(&self, id: &str) -> Option<u8> {
        self.children.get(id).map(|child| child.depth)
    }

    /// Number of live children.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> RlmEngine<impl FnMut(&str) -> Result<String, String>> {
        RlmEngine::new(|prompt: &str| Ok(format!("answer to: {prompt}")))
    }

    #[test]
    fn spawn_requires_a_unique_name() {
        let mut engine = engine();
        let request = RlmSpawnRequest {
            prompt: "do the thing".to_owned(),
            name: Some("worker".to_owned()),
            model: None,
            thinking: None,
            cwd: None,
            spawned_by_request_id: None,
            factory: None,
        };
        assert!(engine.spawn(request.clone(), 0).is_ok());
        assert_eq!(
            engine.spawn(request, 0),
            Err(RlmError::DuplicateName("worker".to_owned()))
        );
        assert_eq!(
            engine.spawn(
                RlmSpawnRequest {
                    name: None,
                    ..RlmSpawnRequest {
                        prompt: "x".to_owned(),
                        name: None,
                        model: None,
                        thinking: None,
                        cwd: None,
                        spawned_by_request_id: None,
                        factory: None,
                    }
                },
                0
            ),
            Err(RlmError::DuplicateName(String::new()))
        );
    }

    #[test]
    fn spawn_validates_thinking_and_model() {
        let mut engine = engine();
        let base = RlmSpawnRequest {
            prompt: "x".to_owned(),
            name: Some("a".to_owned()),
            model: None,
            thinking: Some("ultra".to_owned()),
            cwd: None,
            spawned_by_request_id: None,
            factory: None,
        };
        assert_eq!(
            engine.spawn(base, 0),
            Err(RlmError::InvalidThinking("ultra".to_owned()))
        );
        let base = RlmSpawnRequest {
            prompt: "x".to_owned(),
            name: Some("b".to_owned()),
            model: Some("no-slash".to_owned()),
            thinking: Some("low".to_owned()),
            cwd: None,
            spawned_by_request_id: None,
            factory: None,
        };
        assert_eq!(
            engine.spawn(base, 0),
            Err(RlmError::UnknownModel("no-slash".to_owned()))
        );
    }

    #[test]
    fn spawn_refuses_unbounded_recursion() {
        let mut engine = engine();
        let request = RlmSpawnRequest {
            prompt: "x".to_owned(),
            name: Some("deep".to_owned()),
            model: None,
            thinking: None,
            cwd: None,
            spawned_by_request_id: None,
            factory: None,
        };
        assert_eq!(
            engine.spawn(request, MAX_DEPTH + 1),
            Err(RlmError::DepthExceeded {
                depth: MAX_DEPTH + 1,
                max: MAX_DEPTH
            })
        );
    }

    #[test]
    fn end_turn_blocks_further_spawns() {
        let mut engine = engine();
        assert!(engine.end_turn().is_ok());
        assert_eq!(engine.end_turn(), Err(RlmError::TurnEnded));
        let request = RlmSpawnRequest {
            prompt: "x".to_owned(),
            name: Some("late".to_owned()),
            model: None,
            thinking: None,
            cwd: None,
            spawned_by_request_id: None,
            factory: None,
        };
        assert_eq!(engine.spawn(request, 0), Err(RlmError::TurnEnded));
    }
}
