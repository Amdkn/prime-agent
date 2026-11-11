//! Lineage — the measure zero and the upstream surface pins (H-M1/H-M2).
//!
//! The adapter pins the upstream surfaces it mirrors so a drift in
//! the fork is visible as a broken pin, not a silent behaviour change
//! (the pas-5 pattern: 10 upstream surface pins, H-P5-1).

/// The fork.
pub const FORK: &str = "Amdkn/prime-agent";
/// The upstream.
pub const UPSTREAM: &str = "PrimeIntellect-ai/prime-agent";
/// The gel — the fork's main HEAD at R2.
pub const GEL: &str = "74591cddef28d11249c3171112114fc4755375d7";
/// The gel, short.
pub const GEL_SHORT: &str = "74591cd";
/// The bind — one file (`aspace-plugin.json`), parent = the gel.
pub const BIND: &str = "3ba6791f723635ce89dcf79f69bdfa101e340576";
/// The bind, short.
pub const BIND_SHORT: &str = "3ba6791";
/// The upstream head at R2 — drift 21, a revisit candidate, not a gate.
pub const UPSTREAM_HEAD: &str = "6aaa3a25518b1d01389540eabb4e3a89bca666bb";
/// The upstream head, short.
pub const UPSTREAM_HEAD_SHORT: &str = "6aaa3a2";
/// The drift at R2 (commits the gel is behind the upstream head).
pub const DRIFT: u32 = 21;
/// The licence in the file (the API says NOASSERTION).
pub const LICENCE: &str = "MIT";
/// The version at the gel.
pub const VERSION: &str = "0.10.2";
/// The manifest address (A3).
pub const MANIFEST_ADDRESS: &str = "fiber:a0:prime-agent";
/// The state the manifest is created with (A3 — never 0).
pub const MANIFEST_STATE: u8 = 2;

/// One pinned upstream surface: where the adapter's mirror comes
/// from, so the pin can be re-checked against the fork.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfacePin {
    pub name: &'static str,
    pub path: &'static str,
    pub line: &'static str,
}

/// The upstream surfaces the adapter mirrors (H-M1/H-M2).
#[must_use]
pub fn surface_pins() -> Vec<SurfacePin> {
    vec![
        SurfacePin {
            name: "RlmSpawnRequest",
            path: "crates/pa-core/src/session_engine/rlm_host.rs",
            line: "138",
        },
        SurfacePin {
            name: "rlm.spawn",
            path: "prime-agent-runtime/src/rlm/__init__.py",
            line: "212",
        },
        SurfacePin {
            name: "rlm.collect",
            path: "prime-agent-runtime/src/rlm/__init__.py",
            line: "463",
        },
        SurfacePin {
            name: "rlm.end_turn",
            path: "prime-agent-runtime/src/rlm/__init__.py",
            line: "196",
        },
        SurfacePin {
            name: "plan_refinement",
            path: "crates/pa-core/src/refinement/executor.rs",
            line: "194",
        },
        SurfacePin {
            name: "apply_refinement_plan",
            path: "crates/pa-core/src/refinement/executor.rs",
            line: "304",
        },
        SurfacePin {
            name: "rollback_proposal",
            path: "crates/pa-core/src/refinement/planner.rs",
            line: "505",
        },
        SurfacePin {
            name: "telemetry_switch (default ON)",
            path: "crates/pa-core/src/session_engine/telemetry/status.rs",
            line: "12",
        },
        SurfacePin {
            name: "ANALYTICS_ENDPOINT",
            path: "crates/pa-telemetry/src/sinks/analytics.rs",
            line: "20",
        },
        SurfacePin {
            name: "refine.run / refine.status",
            path: "skills/refine/SKILL.md",
            line: "1",
        },
    ]
}

/// The measure zero, as a single record (H-M1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureZero {
    pub fork: &'static str,
    pub upstream: &'static str,
    pub gel: &'static str,
    pub bind: &'static str,
    pub upstream_head: &'static str,
    pub drift: u32,
    pub licence: &'static str,
    pub version: &'static str,
    pub manifest_address: &'static str,
    pub manifest_state: u8,
}

#[must_use]
pub fn measure_zero() -> MeasureZero {
    MeasureZero {
        fork: FORK,
        upstream: UPSTREAM,
        gel: GEL,
        bind: BIND,
        upstream_head: UPSTREAM_HEAD,
        drift: DRIFT,
        licence: LICENCE,
        version: VERSION,
        manifest_address: MANIFEST_ADDRESS,
        manifest_state: MANIFEST_STATE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pins_are_ten_and_the_state_is_two() {
        assert_eq!(surface_pins().len(), 10);
        let zero = measure_zero();
        assert_eq!(zero.gel, GEL);
        assert_eq!(zero.bind, BIND);
        assert_eq!(zero.drift, 21);
        assert_eq!(zero.manifest_state, 2);
        assert_eq!(zero.manifest_address, "fiber:a0:prime-agent");
    }

    #[test]
    fn the_adapter_never_imports_the_fork() {
        // H-M2, structural: the adapter's manifest carries no
        // dependency at all (std only) — no pa-* crate can be pulled
        // in. The test reads the manifest it ships with.
        let manifest =
            std::fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR")))
                .expect("read adapter Cargo.toml");
        assert!(
            !manifest.contains("pa-core")
                && !manifest.contains("pa-agent")
                && !manifest.contains("pa-daemon")
                && !manifest.contains("pa-telemetry"),
            "the adapter must not depend on any fork crate"
        );
        assert!(
            !manifest.contains("[dependencies]"),
            "the adapter is std only: no [dependencies] section"
        );
    }

    // The telemetry-endpoint guard lives in tests/telemetry_guard.rs
    // (not here): the guard literal must not sit in a file the guard
    // itself scans, or the scan would match its own check.
}
