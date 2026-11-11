//! The Continual Harness — evidence-backed harness updates (`/refine`).
//!
//! Mirrors the gel (`74591cd`):
//! - `plan_refinement` / `apply_refinement_plan` / `rollback_proposal`
//!   in `crates/pa-core/src/refinement/executor.rs` (plan :194, apply
//!   :304, rollback via `RefineOptions.rollback_id`).
//! - `RefineOptions { global, instructions, rollback_id }`
//!   (executor.rs:15) — rollback is a plan, never a mutation in place.
//! - `rollback_proposal` in `crates/pa-core/src/refinement/planner.rs`
//!   (:505) — inverse edits in reverse order, skipping non-applied
//!   edits; unknown rollback id errors (`Refinement {id} not found`).
//! - `RefinementResult` / `AppliedRefinementEdit` in
//!   `crates/pa-core/src/refinement/mod.rs` (:337) — before/after
//!   snapshots ride with every applied edit.
//! - The skill `skills/refine/SKILL.md` — `refine.run(instructions,
//!   global_=False)` schedules refinement at turn end;
//!   `refine.status()` reports pending / in_flight.
//! - `review_auto_refine` (executor.rs:363) — the automatic review gate.
//! - `generate_refinement_id` — canonical `refine_<17 digits>`.
//!
//! Rules carried from the gel (not invented):
//! - Scope is Local by default; Global only on explicit request.
//! - A plan never mutates: callers re-read the harness store before
//!   applying (the planning call can take seconds).
//! - A refused update is annulled by rollback: the harness returns to
//!   the previous state, via the recorded before-snapshots.
//! - Refinement never runs mid-cell; a scheduled refinement runs when
//!   the current turn ends.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// A harness entry kind — the gel's `RefinementKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefinementKind {
    Prompt,
    Memory,
    Skill,
    Subagent,
    Factory,
}

impl RefinementKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Memory => "memory",
            Self::Skill => "skill",
            Self::Subagent => "subagent",
            Self::Factory => "factory",
        }
    }
}

/// An edit action — the gel's `RefinementAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefinementAction {
    Create,
    Update,
    Delete,
}

impl RefinementAction {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// The scope of a refinement — the gel's `HarnessScope`.
/// Local by default; Global only on explicit request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessScope {
    Local,
    Global,
}

impl HarnessScope {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
        }
    }
}

/// One editable continual harness entry — the gel's `HarnessEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessEntry {
    pub id: String,
    pub kind: RefinementKind,
    pub title: String,
    pub content: String,
    pub scope: HarnessScope,
}

/// One planned edit — the gel's `RefinementEdit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementEdit {
    pub action: RefinementAction,
    pub kind: RefinementKind,
    /// The entry id for update/delete; empty for create (the id is
    /// slugged from the title on apply, as in the gel).
    pub id: Option<String>,
    pub title: String,
    pub content: String,
}

/// A planned refinement — the gel's `RefinementProposal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementProposal {
    pub summary: String,
    pub rationale: String,
    pub expected_outcome: String,
    pub edits: Vec<RefinementEdit>,
}

/// One applied (or failed) edit with before/after snapshots — the
/// gel's `AppliedRefinementEdit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedRefinementEdit {
    pub action: RefinementAction,
    pub kind: RefinementKind,
    pub id: String,
    pub applied: bool,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// The result of an applied refinement — the gel's `RefinementResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementResult {
    pub id: String,
    pub summary: String,
    pub rationale: String,
    pub expected_outcome: String,
    pub applied_edits: Vec<AppliedRefinementEdit>,
    pub rollback_of: Option<String>,
    pub scope: Option<HarnessScope>,
}

/// A planned refinement awaiting application — the gel's
/// `RefinementPlan`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementPlan {
    pub proposal: RefinementProposal,
    pub id: String,
    pub rollback_of: Option<String>,
    pub rollback_scope: Option<HarnessScope>,
}

/// Options for planning a refinement — the gel's `RefineOptions`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefineOptions {
    pub global: bool,
    pub instructions: Option<String>,
    pub rollback_id: Option<String>,
}

/// What planning a refinement can refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefineError {
    /// The rollback id is not in the history (gel: `Refinement {id}
    /// not found`).
    UnknownRollback(String),
    /// The proposer failed to produce a proposal.
    Proposer(String),
}

impl std::fmt::Display for RefineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownRollback(id) => write!(f, "Refinement {id} not found"),
            Self::Proposer(message) => write!(f, "proposer failed: {message}"),
        }
    }
}

impl std::error::Error for RefineError {}

/// The harness store: entries by id, in memory. The durable store is
/// the Runtime's JSON file; this is the store the adapter plans and
/// applies against.
#[derive(Debug, Default, Clone)]
pub struct HarnessState {
    pub entries: HashMap<String, HarnessEntry>,
}

impl HarnessState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&HarnessEntry> {
        self.entries.get(id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

static REFINE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Mint a refinement id in the canonical `refine_<timestamp>` format
/// (gel: `generate_refinement_id`, 17 digits).
#[must_use]
pub fn generate_refinement_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = REFINE_SEQ.fetch_add(1, Ordering::SeqCst);
    let digits = format!("{millis:013}{seq:04}");
    format!("refine_{}", &digits[..digits.len().min(17)])
}

/// The inverse of an applied refinement — the gel's `rollback_proposal`
/// (planner.rs:505): inverse edits in reverse order, skipping
/// non-applied edits. A create is undone by a delete; an update or
/// delete is undone by restoring the before-snapshot.
#[must_use]
pub fn rollback_proposal(target: &RefinementResult) -> RefinementProposal {
    let mut edits: Vec<RefinementEdit> = Vec::new();
    for edit in target.applied_edits.iter().rev() {
        if !edit.applied {
            continue;
        }
        if let Some(before) = &edit.before {
            edits.push(RefinementEdit {
                action: if edit.after.is_some() {
                    RefinementAction::Update
                } else {
                    RefinementAction::Create
                },
                kind: edit.kind,
                id: Some(edit.id.clone()),
                title: edit.id.clone(),
                content: before.clone(),
            });
        } else {
            edits.push(RefinementEdit {
                action: RefinementAction::Delete,
                kind: edit.kind,
                id: Some(edit.id.clone()),
                title: edit.id.clone(),
                content: String::new(),
            });
        }
    }
    RefinementProposal {
        summary: format!("Rollback of {}", target.id),
        rationale: format!(
            "Annul refinement {}: the harness returns to the previous state.",
            target.id
        ),
        expected_outcome: "The harness state is the state before the rolled-back refinement."
            .to_owned(),
        edits,
    }
}

/// Produce a refinement proposal without mutating any harness state —
/// the gel's `plan_refinement` (executor.rs:194).
///
/// With `options.rollback_id`, the plan is the rollback of that
/// refinement (error on an unknown id). Otherwise the proposal comes
/// from `propose`, the scripted proposer in tests / the Runtime's
/// refine call in production — the adapter never calls a model itself.
pub fn plan_refinement(
    history: &[RefinementResult],
    options: &RefineOptions,
    propose: &dyn Fn(&str) -> Result<RefinementProposal, String>,
) -> Result<RefinementPlan, RefineError> {
    let id = generate_refinement_id();
    if let Some(rollback_id) = &options.rollback_id {
        let Some(target) = history.iter().find(|item| &item.id == rollback_id) else {
            return Err(RefineError::UnknownRollback(rollback_id.clone()));
        };
        let scope = target.scope.unwrap_or(if options.global {
            HarnessScope::Global
        } else {
            HarnessScope::Local
        });
        return Ok(RefinementPlan {
            proposal: rollback_proposal(target),
            id,
            rollback_of: Some(target.id.clone()),
            rollback_scope: Some(scope),
        });
    }
    let instructions = options.instructions.as_deref().unwrap_or("");
    let proposal = propose(instructions).map_err(RefineError::Proposer)?;
    Ok(RefinementPlan {
        proposal,
        id,
        rollback_of: None,
        rollback_scope: None,
    })
}

/// Apply a plan to the (re-read) harness state — the gel's
/// `apply_refinement_plan` (executor.rs:304).
///
/// Scope is Local unless the plan carries a rollback scope or the
/// options request Global. `baseline` is the state captured before
/// planning: edits whose entry changed since the baseline are
/// rejected (their `applied` is false and no snapshot is taken).
#[must_use]
pub fn apply_refinement_plan(
    state: &mut HarnessState,
    plan: RefinementPlan,
    options: &RefineOptions,
    baseline: Option<&HarnessState>,
) -> RefinementResult {
    let scope = plan.rollback_scope.unwrap_or(if options.global {
        HarnessScope::Global
    } else {
        HarnessScope::Local
    });
    let mut applied_edits: Vec<AppliedRefinementEdit> = Vec::new();
    for edit in &plan.proposal.edits {
        let id = match &edit.id {
            Some(id) => id.clone(),
            None => slug(&edit.title),
        };
        let before = state.get(&id).map(|entry| entry.content.clone());
        let changed_since_baseline = match baseline {
            Some(baseline_state) => match (baseline_state.get(&id), &before) {
                (Some(base), Some(current)) => base.content != *current,
                (Some(_), None) => true,
                (None, Some(_)) => true,
                (None, None) => false,
            },
            None => false,
        };
        if changed_since_baseline {
            applied_edits.push(AppliedRefinementEdit {
                action: edit.action,
                kind: edit.kind,
                id,
                applied: false,
                before,
                after: None,
            });
            continue;
        }
        let after = match edit.action {
            RefinementAction::Create | RefinementAction::Update => {
                state.entries.insert(
                    id.clone(),
                    HarnessEntry {
                        id: id.clone(),
                        kind: edit.kind,
                        title: edit.title.clone(),
                        content: edit.content.clone(),
                        scope,
                    },
                );
                Some(edit.content.clone())
            }
            RefinementAction::Delete => {
                state.entries.remove(&id);
                None
            }
        };
        applied_edits.push(AppliedRefinementEdit {
            action: edit.action,
            kind: edit.kind,
            id,
            applied: true,
            before,
            after,
        });
    }
    RefinementResult {
        id: plan.id,
        summary: plan.proposal.summary,
        rationale: plan.proposal.rationale,
        expected_outcome: plan.proposal.expected_outcome,
        applied_edits,
        rollback_of: plan.rollback_of,
        scope: Some(scope),
    }
}

/// Slug an entry title into an id (the gel's `slug` in planner.rs).
fn slug(raw: &str) -> String {
    let mut normalized = String::new();
    for ch in raw.trim().to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            normalized.push(ch);
        } else if ch == ' ' || ch == '-' || ch == '_' {
            normalized.push('-');
        }
    }
    let trimmed = normalized.trim_matches('-');
    if trimmed.is_empty() {
        "entry".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The automatic `/refine` review gate — the gel's `review_auto_refine`
/// (executor.rs:363) as a deterministic gate: the gate opens when the
/// caller says the turn count since the last review reached its
/// threshold. The threshold is a Runtime policy, passed by the caller —
/// the adapter invents no product default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoRefineReview {
    pub should_refine: bool,
    pub rationale: String,
    pub instructions: Option<String>,
}

/// Decide whether an automatic refine is due.
#[must_use]
pub fn review_auto_refine(
    reason: &str,
    turns_since_last_review: u32,
    min_turns: u32,
) -> AutoRefineReview {
    if turns_since_last_review >= min_turns && !reason.trim().is_empty() {
        AutoRefineReview {
            should_refine: true,
            rationale: format!(
                "Auto-refine due: {turns_since_last_review} turns since the last review ({reason})."
            ),
            instructions: Some(reason.to_owned()),
        }
    } else {
        AutoRefineReview {
            should_refine: false,
            rationale: format!(
                "No auto-refine: {turns_since_last_review} turns since the last review, threshold {min_turns}."
            ),
            instructions: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposer(instructions: &str) -> Result<RefinementProposal, String> {
        Ok(RefinementProposal {
            summary: format!("Apply: {instructions}"),
            rationale: "scripted proposer".to_owned(),
            expected_outcome: "one memory entry created".to_owned(),
            edits: vec![RefinementEdit {
                action: RefinementAction::Create,
                kind: RefinementKind::Memory,
                id: None,
                title: instructions.to_owned(),
                content: instructions.to_owned(),
            }],
        })
    }

    #[test]
    fn apply_is_local_by_default() {
        let mut state = HarnessState::new();
        let plan = plan_refinement(
            &[],
            &RefineOptions {
                global: false,
                instructions: Some("check git status before committing".to_owned()),
                rollback_id: None,
            },
            &proposer,
        )
        .expect("plan");
        let result = apply_refinement_plan(&mut state, plan, &RefineOptions::default(), None);
        assert_eq!(result.scope, Some(HarnessScope::Local));
        assert_eq!(state.len(), 1);
        let entry = state
            .get("check-git-status-before-committing")
            .expect("entry");
        assert_eq!(entry.scope, HarnessScope::Local);
        assert!(result.applied_edits.iter().all(|edit| edit.applied));
    }

    #[test]
    fn unknown_rollback_id_errors() {
        let err = plan_refinement(
            &[],
            &RefineOptions {
                global: false,
                instructions: None,
                rollback_id: Some("refine_00000000000000000".to_owned()),
            },
            &proposer,
        )
        .expect_err("unknown rollback id must error");
        assert_eq!(
            err,
            RefineError::UnknownRollback("refine_00000000000000000".to_owned())
        );
    }

    #[test]
    fn baseline_rejects_changed_entries() {
        let mut state = HarnessState::new();
        state.entries.insert(
            "k".to_owned(),
            HarnessEntry {
                id: "k".to_owned(),
                kind: RefinementKind::Memory,
                title: "k".to_owned(),
                content: "v1".to_owned(),
                scope: HarnessScope::Local,
            },
        );
        let baseline = state.clone();
        state.entries.get_mut("k").expect("k").content = "v2".to_owned();
        let plan = RefinementPlan {
            proposal: RefinementProposal {
                summary: "s".to_owned(),
                rationale: "r".to_owned(),
                expected_outcome: "o".to_owned(),
                edits: vec![RefinementEdit {
                    action: RefinementAction::Update,
                    kind: RefinementKind::Memory,
                    id: Some("k".to_owned()),
                    title: "k".to_owned(),
                    content: "v3".to_owned(),
                }],
            },
            id: generate_refinement_id(),
            rollback_of: None,
            rollback_scope: None,
        };
        let result =
            apply_refinement_plan(&mut state, plan, &RefineOptions::default(), Some(&baseline));
        assert!(!result.applied_edits[0].applied);
        assert_eq!(state.get("k").expect("k").content, "v2");
    }
}
