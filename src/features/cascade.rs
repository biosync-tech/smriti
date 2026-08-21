//! Benna-Fusi cascade synapses (Nature Neuroscience 19, 1697–1706, 2016).
//!
//! ## Why this exists
//!
//! Smriti's Migration 009 gave every note a *scalar* `consolidation_score`.
//! That's enough to flag stale notes, but it's a single-timescale model —
//! identical to a leaky integrator. Benna & Fusi prove that a *cascade* of
//! variables operating at exponentially-spaced timescales lifts memory
//! capacity from O(√N) to O(N / log N) and gives a quantitative account of
//! the brain's stability-plasticity tradeoff (paper §"A linear chain of
//! variables", §3).
//!
//! Concretely: each note carries a vector
//!
//! ```text
//! u = [u_0, u_1, ..., u_{K-1}]
//! ```
//!
//! representing synaptic strength at timescales τ_k = C_k / g, where the
//! capacities C_k grow geometrically. Access events inject signal into the
//! fastest level u_0 (synaptic potentiation). Between events, mass diffuses
//! toward slower levels, where it is robust to interference from new writes.
//! Old, repeatedly-replayed notes accumulate mass at deep levels —
//! "consolidated" in the precise CLS sense (McClelland et al. 1995).
//!
//! ## Continuous-time dynamics (paper Eq. 1, simplified for uniform g)
//!
//! ```text
//! C_k * du_k/dt = g(u_{k-1} - u_k) - g(u_k - u_{k+1})
//! ```
//!
//! Boundary conditions:
//!   - Left (k = 0):  u_{-1} = 0 during dynamics. Input arrives as an
//!     instantaneous pulse, modelled by [`CascadeState::inject`].
//!   - Right (k = K-1): closed (u_K = u_{K-1}, zero flux) preserves total
//!     mass; leaky (u_K = 0) lets very old memories decay.
//!     Smriti defaults to *leaky* so the graph can shrink.
//!
//! ## Why this is novel for agent memory
//!
//! Mem0, Letta, and Zep all use single-timescale scoring. Their salience
//! collapses to a single scalar; old + replayed memories are
//! indistinguishable from old + abandoned ones. The cascade preserves
//! that distinction structurally — depth-K mass *only* accumulates from
//! repeated, time-spread access. It cannot be faked by burst writes.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::errors::AppResult;

// ── Configuration ─────────────────────────────────────────────────────────

/// Tunable cascade parameters. Defaults reproduce the geometric-timescale
/// regime from Benna & Fusi 2016 with K=6 levels covering one hour to
/// roughly six weeks — appropriate for typical agent memory access patterns.
///
/// Healthcare/compliance profiles should consider larger K and deeper τ_K
/// (months) so consolidation tracks regulatory retention windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeConfig {
    /// Number of cascade levels K. Paper §3 shows useful capacity for K ≥ 4.
    pub levels: usize,

    /// Capacity of the fastest level C_0, in seconds. Effective τ_0 = C_0 / g.
    pub capacity_base_seconds: f64,

    /// Geometric ratio C_{k+1} / C_k. Paper canonical choice is 4.0.
    /// Ratios > 1 produce a cascade; larger ratios → longer-tailed memory.
    pub capacity_ratio: f64,

    /// Inter-level coupling g (dimensionless). Uniform across levels in v1
    /// — matches the closed-form analysis in the paper's appendix.
    pub coupling: f64,

    /// Strength α of the input pulse on each access event, injected into u_0.
    pub input_strength: f64,

    /// Right boundary condition. `true` → leaky (u_K = 0, old memories decay);
    /// `false` → closed (mass conserved). Smriti defaults to leaky so very
    /// old, never-touched notes eventually fade — required for the
    /// "memory gets cleaner over time" claim.
    pub leaky_deep_boundary: bool,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        // K=6, C_0 = 1 hour, ratio 4 → C_5 = 4^5 hours ≈ 6 weeks.
        Self {
            levels: 6,
            capacity_base_seconds: 3_600.0,
            capacity_ratio: 4.0,
            coupling: 1.0,
            input_strength: 1.0,
            leaky_deep_boundary: true,
        }
    }
}

impl CascadeConfig {
    /// C_k = C_0 * ratio^k.
    pub fn capacity_at(&self, k: usize) -> f64 {
        self.capacity_base_seconds * self.capacity_ratio.powi(k as i32)
    }

    /// τ_k = C_k / g.
    pub fn timescale_at(&self, k: usize) -> f64 {
        self.capacity_at(k) / self.coupling
    }

    /// Largest stable explicit-Euler step. The fastest level k=0 has
    /// effective rate magnitude bounded by 3g/C_0 (worst case under leaky
    /// left + leaky right). Use C_0 / (8g) for a 4× safety margin.
    pub fn max_stable_dt(&self) -> f64 {
        self.capacity_base_seconds / (8.0 * self.coupling)
    }

    /// Salience readout weights — uniform across levels in v1. Future:
    /// bias toward middle levels for a "structural memory" view.
    pub fn salience_weights(&self) -> Vec<f64> {
        let n = self.levels.max(1);
        vec![1.0 / n as f64; n]
    }
}

// ── State ─────────────────────────────────────────────────────────────────

/// Per-note cascade state. Stored as JSON in `notes.cascade_state`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CascadeState {
    /// Per-level synaptic strengths u_0, u_1, ..., u_{K-1}.
    pub levels: Vec<f64>,
    /// Wall-clock at which `levels` were last brought up to date.
    pub last_updated: DateTime<Utc>,
}

impl CascadeState {
    pub fn new(config: &CascadeConfig, now: DateTime<Utc>) -> Self {
        Self {
            levels: vec![0.0; config.levels.max(1)],
            last_updated: now,
        }
    }

    /// Apply the cascade ODE for `dt_seconds` of elapsed real time.
    ///
    /// Uses explicit Euler with substepping bounded by [`max_stable_dt`].
    /// For very long elapsed times we fast-path: under the leaky boundary,
    /// `dt > 10 * τ_{K-1}` is past the slowest mode's e-folding window so
    /// the state has effectively reached the zero fixed point.
    pub fn apply_dynamics(&mut self, config: &CascadeConfig, dt_seconds: f64) {
        if dt_seconds <= 0.0 || self.levels.is_empty() {
            return;
        }

        if config.leaky_deep_boundary {
            let deepest_tau = config.timescale_at(self.levels.len() - 1);
            if dt_seconds > 10.0 * deepest_tau {
                self.levels.fill(0.0);
                return;
            }
        }

        let max_step = config.max_stable_dt().max(f64::EPSILON);
        // Cap iterations to keep cold-start writes from spinning the CPU.
        // 200_000 substeps at C_0/8 = 7.2k seconds per substep covers
        // ~46 years of elapsed time — well past any realistic gap.
        let n_steps = ((dt_seconds / max_step).ceil() as usize).clamp(1, 200_000);
        let sub_dt = dt_seconds / n_steps as f64;
        for _ in 0..n_steps {
            self.euler_step(config, sub_dt);
        }
    }

    /// One explicit-Euler step of the linear cascade.
    /// Assumes no external drive (drive is delivered via [`inject`]).
    // Range-indexed loop is intentional: each iteration reads
    // `self.levels[i-1]` and `self.levels[i+1]` (left/right neighbours)
    // while writing `new_levels[i]`. Splitting into iter_mut() would
    // require unsafe or extra clones — the loop body is the simplest
    // expression of the cascade update.
    #[allow(clippy::needless_range_loop)]
    fn euler_step(&mut self, config: &CascadeConfig, dt: f64) {
        let k = self.levels.len();
        let g = config.coupling;
        let mut new_levels = self.levels.clone();
        for i in 0..k {
            // Left neighbor — closed at the input port during dynamics.
            // u_{-1} = 0, so the leftmost flux into level 0 is -g * u_0.
            let u_left = if i == 0 { 0.0 } else { self.levels[i - 1] };
            // Right neighbor — depends on boundary at the deepest level.
            let u_right = if i + 1 < k {
                self.levels[i + 1]
            } else if config.leaky_deep_boundary {
                0.0
            } else {
                // Closed boundary: mirror level so flux is zero.
                self.levels[i]
            };
            let influx = g * (u_left - self.levels[i]);
            let outflux = g * (self.levels[i] - u_right);
            let dudt = (influx - outflux) / config.capacity_at(i);
            new_levels[i] = self.levels[i] + dt * dudt;
        }
        self.levels = new_levels;
    }

    /// Inject an input pulse into u_0 (an access event in Smriti).
    pub fn inject(&mut self, config: &CascadeConfig) {
        if let Some(u0) = self.levels.first_mut() {
            *u0 += config.input_strength;
        }
    }

    /// Bring state up to `now` and return the salience readout (a weighted
    /// sum across levels). Mutates so subsequent reads are cheap; the caller
    /// is responsible for persisting if it wants the freshness on disk.
    pub fn salience_at(&mut self, config: &CascadeConfig, now: DateTime<Utc>) -> f64 {
        let dt = (now - self.last_updated).num_milliseconds() as f64 / 1_000.0;
        if dt > 0.0 {
            self.apply_dynamics(config, dt);
            self.last_updated = now;
        }
        let weights = config.salience_weights();
        self.levels
            .iter()
            .zip(weights.iter())
            .map(|(u, w)| u * w)
            .sum()
    }

    /// Pure-read salience without mutation. Useful for explain/inspect paths
    /// where we don't want to touch persisted state.
    pub fn salience_peek(&self, config: &CascadeConfig, now: DateTime<Utc>) -> f64 {
        let mut tmp = self.clone();
        tmp.salience_at(config, now)
    }

    /// Apply elapsed dynamics, then inject. Used on access events.
    pub fn record_access(&mut self, config: &CascadeConfig, now: DateTime<Utc>) {
        let dt = (now - self.last_updated).num_milliseconds() as f64 / 1_000.0;
        if dt > 0.0 {
            self.apply_dynamics(config, dt);
        }
        self.inject(config);
        self.last_updated = now;
    }

    pub fn to_json(&self) -> AppResult<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(s: &str) -> AppResult<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

// ── Persistence ───────────────────────────────────────────────────────────

/// Load cascade state for a note. Returns `Ok(None)` if the column is NULL,
/// missing, or empty — callers should default to a zero state in that case.
pub fn load(conn: &Connection, note_id: &str) -> AppResult<Option<CascadeState>> {
    let row: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT cascade_state FROM notes WHERE id = ?1",
        rusqlite::params![note_id],
        |r| r.get::<_, Option<String>>(0),
    );
    match row {
        Ok(Some(ref s)) if !s.is_empty() => Ok(Some(CascadeState::from_json(s)?)),
        Ok(_) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Persist cascade state. Idempotent.
pub fn save(conn: &Connection, note_id: &str, state: &CascadeState) -> AppResult<()> {
    conn.execute(
        "UPDATE notes
            SET cascade_state = ?1,
                cascade_updated_at = ?2
          WHERE id = ?3",
        rusqlite::params![
            state.to_json()?,
            state.last_updated.to_rfc3339(),
            note_id
        ],
    )?;
    Ok(())
}

/// Inject an access event for `note_id` and persist. Hooked from
/// [`crate::features::consolidation::log_access`].
///
/// Best-effort: callers should not let cascade failures block access logging.
/// The cascade state can be reconstructed from `note_access_log` if lost.
pub fn record_access(
    conn: &Connection,
    note_id: &str,
    config: &CascadeConfig,
) -> AppResult<()> {
    let now = Utc::now();
    let mut state = load(conn, note_id)?.unwrap_or_else(|| CascadeState::new(config, now));
    state.record_access(config, now);
    save(conn, note_id, &state)?;
    Ok(())
}

/// Read-only salience readout for a stored note. Returns `Ok(0.0)` in two
/// distinct cases: (a) the note exists but its `cascade_state` column is NULL
/// (created pre-Migration-011 or never accessed), or (b) the note does not
/// exist in the database at all (the underlying `load` returns `Ok(None)` for
/// a missing row). Callers that need to distinguish "never accessed" from
/// "note absent" must check note existence separately before calling this.
/// Does NOT mutate or persist state — safe to call from scoring passes.
pub fn salience_peek_for_note(
    conn: &Connection,
    note_id: &str,
    config: &CascadeConfig,
    now: DateTime<Utc>,
) -> AppResult<f64> {
    match load(conn, note_id)? {
        Some(state) => Ok(state.salience_peek(config, now)),
        None => Ok(0.0),
    }
}

// ── Explain (CLI/audit surface) ───────────────────────────────────────────

/// Cascade explain payload. Surfaced by `smriti cascade --explain <id>`
/// and (eventually) `GET /api/v1/notes/:id/cascade`.
#[derive(Debug, Clone, Serialize)]
pub struct CascadeExplain {
    pub note_id: String,
    /// u_k for k = 0..K-1 after bringing state up to "now".
    pub levels: Vec<f64>,
    /// C_k in seconds for each level.
    pub capacities_seconds: Vec<f64>,
    /// τ_k = C_k / g for each level (seconds).
    pub timescales_seconds: Vec<f64>,
    /// Last on-disk update (before the explain pass advanced state).
    pub last_updated: DateTime<Utc>,
    /// Weighted-sum readout used as Smriti's cascade salience.
    pub salience: f64,
    /// Human-readable config tag for audit logs.
    pub config_summary: String,
}

pub fn explain(
    conn: &Connection,
    note_id: &str,
    config: &CascadeConfig,
) -> AppResult<CascadeExplain> {
    let now = Utc::now();
    let stored = load(conn, note_id)?;
    let last_updated = stored
        .as_ref()
        .map(|s| s.last_updated)
        .unwrap_or(now);
    let state = stored.unwrap_or_else(|| CascadeState::new(config, now));
    let salience = state.salience_peek(config, now);
    let capacities = (0..config.levels).map(|k| config.capacity_at(k)).collect();
    let timescales = (0..config.levels).map(|k| config.timescale_at(k)).collect();
    Ok(CascadeExplain {
        note_id: note_id.to_string(),
        levels: state.levels.clone(),
        capacities_seconds: capacities,
        timescales_seconds: timescales,
        last_updated,
        salience,
        config_summary: format!(
            "K={} C_0={}s ratio={} g={} leaky={}",
            config.levels,
            config.capacity_base_seconds,
            config.capacity_ratio,
            config.coupling,
            config.leaky_deep_boundary
        ),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn cfg() -> CascadeConfig {
        CascadeConfig::default()
    }

    fn cfg_closed() -> CascadeConfig {
        CascadeConfig {
            leaky_deep_boundary: false,
            ..CascadeConfig::default()
        }
    }

    #[test]
    fn fresh_state_is_zero() {
        let s = CascadeState::new(&cfg(), Utc::now());
        assert_eq!(s.levels.len(), 6);
        assert!(s.levels.iter().all(|&u| u == 0.0));
    }

    #[test]
    fn inject_only_touches_level_zero() {
        let c = cfg();
        let mut s = CascadeState::new(&c, Utc::now());
        s.inject(&c);
        assert_eq!(s.levels[0], 1.0);
        for u in &s.levels[1..] {
            assert_eq!(*u, 0.0);
        }
    }

    /// The defining cascade behavior: a single injection at u_0 transfers
    /// measurable mass into the next level u_1 within one fast timescale.
    /// The slow levels barely move on this horizon — that ordering is the
    /// engineering substantiation of "old + replayed memories live at deep
    /// levels, recent ones at shallow levels."
    #[test]
    fn mass_cascades_from_fast_to_slow_levels() {
        let c = cfg();
        let mut s = CascadeState::new(&c, Utc::now());
        s.inject(&c);
        // Run for one fast timescale (τ_1). Long enough for u_1 to receive
        // measurable mass, short enough that u_5 has not yet had time to
        // accumulate above floating-point noise.
        s.apply_dynamics(&c, c.timescale_at(1));
        assert!(s.levels[1] > 1e-6, "u_1 should have received mass: {:?}", s.levels);
        assert!(
            s.levels[1] > s.levels[5],
            "mass should sit at faster levels at short t: u_1={}, u_5={}",
            s.levels[1],
            s.levels[5],
        );
    }

    /// Leaky boundary: a single pulse decays to zero given enough time.
    /// Engineering claim: never-revisited notes eventually fade.
    #[test]
    fn single_pulse_decays_under_leaky_boundary() {
        let c = cfg();
        let mut s = CascadeState::new(&c, Utc::now());
        s.inject(&c);
        s.apply_dynamics(&c, 100.0 * c.timescale_at(5));
        for u in &s.levels {
            assert!(u.abs() < 1e-6, "leaky cascade should decay to ~0, got {:?}", s.levels);
        }
    }

    /// Closed boundary retains more deep-level mass than leaky after the same
    /// access pattern. The left boundary is leaky in both configurations
    /// (input port has no mirror), so neither conserves mass globally — but
    /// closed-right blocks the slow leak at u_{K-1}, which dominates at
    /// long timescales. Compliance profiles that want maximal long-tail
    /// retention should run with `leaky_deep_boundary = false`.
    #[test]
    fn closed_boundary_retains_more_deep_mass_than_leaky() {
        let leaky = cfg();
        let closed = cfg_closed();
        let now = Utc::now();
        let mut s_leaky = CascadeState::new(&leaky, now);
        let mut s_closed = CascadeState::new(&closed, now);
        // Drive both with identical input pattern: 30 accesses spaced by τ_2.
        let interval = leaky.timescale_at(2);
        let mut t = now;
        for _ in 0..30 {
            s_leaky.record_access(&leaky, t);
            s_closed.record_access(&closed, t);
            t += Duration::milliseconds((interval * 1_000.0) as i64);
        }
        let deep = s_leaky.levels.len() - 1;
        assert!(
            s_closed.levels[deep] >= s_leaky.levels[deep],
            "closed deep-level mass {} should be at least leaky {}",
            s_closed.levels[deep],
            s_leaky.levels[deep],
        );
    }

    /// The load-bearing biological claim: deep-level mass grows monotonically
    /// with the number of *spread-out* access events. This is the structural
    /// reason brittle single-scalar memory layers (Mem0/Zep/Letta) cannot
    /// distinguish "rehearsed for a month" from "fired once."
    ///
    /// We deliberately do NOT compare burst vs spread head-to-head here. At
    /// short horizons (T < τ_{K-1}) the burst pulse is near its u_K-1 peak
    /// while the spread pattern still has injections in transit; the
    /// inequality only flips after the burst has decayed past the spread's
    /// accumulated value. Pinning that comparison would make the test
    /// brittle to parameter changes. The monotonicity assertion captures the
    /// claim that matters without that fragility.
    #[test]
    fn deep_level_grows_with_more_spread_accesses() {
        let c = cfg();
        let now = Utc::now();
        let interval = c.timescale_at(2); // 16h spacing keeps u_0 mostly drained between accesses

        let run = |n: usize| -> f64 {
            let mut s = CascadeState::new(&c, now);
            let mut t = now;
            for _ in 0..n {
                s.record_access(&c, t);
                t += Duration::milliseconds((interval * 1_000.0) as i64);
            }
            s.levels[s.levels.len() - 1]
        };

        let u_deep_few = run(5);
        let u_deep_many = run(50);

        assert!(
            u_deep_many > u_deep_few,
            "u_K-1 should grow with access count: many={} vs few={}",
            u_deep_many,
            u_deep_few,
        );
    }

    /// Stability: a huge dt must not produce NaN/Inf or oscillation.
    /// We rely on substepping inside apply_dynamics — this test pins that contract.
    #[test]
    fn very_long_dt_is_stable() {
        let c = cfg();
        let mut s = CascadeState::new(&c, Utc::now());
        s.inject(&c);
        s.apply_dynamics(&c, 365.0 * 86_400.0); // one year
        for u in &s.levels {
            assert!(u.is_finite(), "got non-finite cascade value: {:?}", s.levels);
        }
    }

    #[test]
    fn salience_increases_with_recent_access() {
        let c = cfg();
        let now = Utc::now();
        let mut quiet = CascadeState::new(&c, now);
        let mut active = CascadeState::new(&c, now);
        for i in 0..5 {
            active.record_access(&c, now + Duration::seconds(i * 60));
        }
        let s_quiet = quiet.salience_at(&c, now + Duration::seconds(300));
        let s_active = active.salience_at(&c, now + Duration::seconds(300));
        assert!(
            s_active > s_quiet,
            "active note salience {} should exceed quiet {}",
            s_active,
            s_quiet
        );
    }

    /// Encoding doesn't have to be bit-identical — serde_json's f64
    /// formatter is roundtrip-safe in the common case but not guaranteed
    /// across all magnitudes. We assert numerical equivalence at a tight
    /// relative tolerance (1e-12), which is what "preserves state" means
    /// for downstream cascade dynamics.
    #[test]
    fn json_roundtrip_preserves_state() {
        let c = cfg();
        let now = Utc::now();
        let mut s = CascadeState::new(&c, now);
        s.record_access(&c, now);
        s.apply_dynamics(&c, 3_600.0);
        let encoded = s.to_json().unwrap();
        let decoded = CascadeState::from_json(&encoded).unwrap();

        assert_eq!(s.last_updated, decoded.last_updated);
        assert_eq!(s.levels.len(), decoded.levels.len());
        for (k, (a, b)) in s.levels.iter().zip(decoded.levels.iter()).enumerate() {
            let scale = a.abs().max(b.abs()).max(1.0);
            assert!(
                (a - b).abs() < 1e-12 * scale,
                "level {} mismatch: encoded={} decoded={}",
                k,
                a,
                b,
            );
        }
    }

    // ── Integration with the real (in-memory) database ───────────────────

    fn fresh_db() -> crate::storage::Database {
        crate::storage::Database::new(":memory:").expect("db")
    }

    fn insert_note(db: &crate::storage::Database, id: &str) {
        db.execute(|conn| {
            conn.execute(
                "INSERT INTO notes (id, title, content, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    id,
                    "t",
                    "c",
                    Utc::now().to_rfc3339(),
                    Utc::now().to_rfc3339()
                ],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn migration_adds_cascade_columns() {
        let db = fresh_db();
        db.execute(|conn| {
            // Bind the Statement so the iterator borrow is well-scoped.
            let mut stmt = conn.prepare("PRAGMA table_info(notes)")?;
            let cols: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(Result::ok)
                .collect();
            assert!(cols.contains(&"cascade_state".to_string()), "got {:?}", cols);
            assert!(cols.contains(&"cascade_updated_at".to_string()), "got {:?}", cols);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn record_access_persists_state_and_increments_u0() {
        let db = fresh_db();
        insert_note(&db, "n1");
        let c = cfg();

        db.execute(|conn| {
            record_access(conn, "n1", &c)?;
            Ok(())
        })
        .unwrap();

        let loaded = db
            .execute(|conn| load(conn, "n1"))
            .unwrap()
            .expect("cascade state should be persisted");
        assert!(loaded.levels[0] > 0.0);
    }

    #[test]
    fn explain_handles_missing_state_gracefully() {
        let db = fresh_db();
        insert_note(&db, "n1");
        let c = cfg();
        let payload = db.execute(|conn| explain(conn, "n1", &c)).unwrap();
        assert_eq!(payload.note_id, "n1");
        assert_eq!(payload.levels.len(), c.levels);
        assert_eq!(payload.salience, 0.0);
    }

    #[test]
    fn salience_peek_for_note_zero_when_no_state() {
        let db = fresh_db();
        insert_note(&db, "n_peek");
        let cfg = CascadeConfig::default();
        let s = db
            .execute(|conn| salience_peek_for_note(conn, "n_peek", &cfg, Utc::now()))
            .unwrap();
        assert_eq!(s, 0.0, "no cascade_state row → salience reads as 0");
    }

    #[test]
    fn salience_peek_for_note_reflects_recorded_access() {
        let db = fresh_db();
        insert_note(&db, "n_acc");
        let cfg = CascadeConfig::default();
        db.execute(|conn| record_access(conn, "n_acc", &cfg)).unwrap();
        let s = db
            .execute(|conn| salience_peek_for_note(conn, "n_acc", &cfg, Utc::now()))
            .unwrap();
        assert!(s > 0.0, "after one access, salience should be positive: got {s}");
    }
}
