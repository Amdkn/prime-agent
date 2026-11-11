//! Smoke — D-N052 R2, the three named proofs (A7).
//!
//! (a) A recursive sub-agent is spawned as a function in the
//! persistent REPL, and it returns (`rlm.spawn` + `rlm.collect`).
//! (b) An evidence-backed harness update is applied, session-local
//! by default (`/refine`).
//! (c) A refused update is annulled: the harness returns to the
//! previous state (rollback).
//!
//! The smoke runs on the adapter with a scripted provider — no model
//! call, no key, no network (D16: the harness V5 is the backend; the
//! Runtime's gateway provider replaces the scripted one in
//! production, the interface is the same).

use aspace_prime_agent_adapter::daemon_ref::DaemonRef;
use aspace_prime_agent_adapter::refine::{
    apply_refinement_plan, plan_refinement, HarnessScope, HarnessState, RefineOptions,
    RefinementAction, RefinementEdit, RefinementKind, RefinementProposal,
};
use aspace_prime_agent_adapter::rlm::{RlmEngine, RlmSpawnRequest};
use aspace_prime_agent_adapter::runtime_surface::RuntimeSurface;

fn scripted_provider(prompt: &str) -> Result<String, String> {
    Ok(format!("done: {prompt}"))
}

fn spawn_request(name: &str) -> RlmSpawnRequest {
    RlmSpawnRequest {
        prompt: "summarise the harness state".to_owned(),
        name: Some(name.to_owned()),
        model: Some("gateway/default".to_owned()),
        thinking: Some("low".to_owned()),
        cwd: None,
        spawned_by_request_id: None,
        factory: None,
    }
}

fn proposer(instructions: &str) -> Result<RefinementProposal, String> {
    Ok(RefinementProposal {
        summary: format!("Apply: {instructions}"),
        rationale: "evidence: the child reported the pattern twice".to_owned(),
        expected_outcome: "one memory entry created, session-local".to_owned(),
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
fn smoke_spawn_returns() {
    let mut engine = RlmEngine::new(scripted_provider);
    let handle = engine.spawn(spawn_request("summariser"), 0).expect("spawn");
    assert_eq!(handle.name, "summariser");
    let results = engine.collect(None, 0);
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert!(result.settled);
    assert_eq!(
        result.status,
        aspace_prime_agent_adapter::rlm::ChildStatus::Completed
    );
    assert_eq!(
        result.answer_text.as_deref(),
        Some("done: summarise the harness state")
    );
    assert_eq!(engine.child_count(), 1);
}

#[test]
fn smoke_refine_applies_session_local() {
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
        .expect("the memory entry was created");
    assert_eq!(entry.scope, HarnessScope::Local);
    assert!(result.applied_edits.iter().all(|edit| edit.applied));
}

#[test]
fn smoke_refused_update_rolls_back() {
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
    let applied = apply_refinement_plan(&mut state, plan, &RefineOptions::default(), None);
    assert_eq!(state.len(), 1);
    let history = vec![applied];
    let rollback = plan_refinement(
        &history,
        &RefineOptions {
            global: false,
            instructions: None,
            rollback_id: Some(history[0].id.clone()),
        },
        &proposer,
    )
    .expect("rollback plan");
    assert_eq!(rollback.rollback_of, Some(history[0].id.clone()));
    let undone = apply_refinement_plan(&mut state, rollback, &RefineOptions::default(), None);
    assert_eq!(state.len(), 0);
    assert!(undone.applied_edits.iter().all(|edit| edit.applied));
    assert_eq!(undone.rollback_of, Some(history[0].id.clone()));
}

#[test]
fn smoke_runtime_cycle_with_h1() {
    // R2-5, the Runtime cycle with prime-agent as the H1 engine:
    // spawn a child, collect its answer, refine the harness from
    // the answer, end the turn.
    let mut engine = RlmEngine::new(scripted_provider);
    let handle = engine.spawn(spawn_request("scout"), 0).expect("spawn");
    let results = engine.collect(Some(&[handle.rlm_child_id.as_str()]), 0);
    assert_eq!(results.len(), 1);
    let answer = results[0].answer_text.clone().expect("answer");
    let mut state = HarnessState::new();
    let plan = plan_refinement(
        &[],
        &RefineOptions {
            global: false,
            instructions: Some(answer),
            rollback_id: None,
        },
        &proposer,
    )
    .expect("plan");
    let result = apply_refinement_plan(&mut state, plan, &RefineOptions::default(), None);
    assert_eq!(state.len(), 1);
    assert_eq!(result.scope, Some(HarnessScope::Local));
    assert!(engine.end_turn().is_ok());
    assert!(engine.turn_ended());
}

#[test]
fn smoke_single_daemon_and_repointed_surface() {
    // H-F8-1: pa-daemon is a reference, never launched.
    assert!(DaemonRef::pa_daemon().launch_attempt().is_err());
    // H-F8-2: the Python runtime is a surface re-pointed at the
    // single daemon, never a second daemon.
    let surface = RuntimeSurface::repointed();
    assert!(surface.check().is_ok());
    assert!(!surface.spawns_second_daemon());
}
