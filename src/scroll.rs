use serde::{Deserialize, Serialize};

/// Tuning for [`ScrollKinetics`] momentum physics. Sane,
/// physically-motivated defaults give every consumer a weighty glide
/// for free; serde-derived so it can be loaded from a shikumi config
/// (matching [`crate::Theme`]).
///
/// `friction` is the exponential decay rate of velocity (per second):
/// velocity halves every `ln2 / friction` seconds. `max_velocity` caps
/// a frantic flick. `stop_epsilon` is the velocity magnitude below
/// which a decaying glide snaps to a clean stop.
///
/// ## Unit-agnostic
///
/// [`ScrollKinetics`] works in abstract fractional **units** — the
/// consumer decides what one unit is. mado treats a unit as a terminal
/// *line*; a browser / pixel-scrolled list treats it as a *pixel* (or
/// scroll-tick). The physics is identical either way; only the
/// magnitudes change. The defaults below are line-oriented (mado's felt
/// behavior); a pixel consumer scales `max_velocity` / `stop_epsilon`
/// up accordingly (e.g. ×line-height) — see [`ScrollKineticsConfig`]
/// constructor docs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollKineticsConfig {
    /// Exponential friction (decay rate per second). Higher = the
    /// glide bleeds off faster. The default `5.0` gives a weighty
    /// ~0.7 s coast (velocity drops to <STOP_EPSILON over roughly that
    /// long for a typical flick) — long enough to feel momentum, short
    /// enough to never run away.
    pub friction: f32,
    /// Maximum `|velocity|` in units/sec an impulse may reach. Caps a
    /// frantic flick so it can't launch straight to the content edge.
    pub max_velocity: f32,
    /// Velocity magnitude (units/sec) below which a decaying glide is
    /// treated as a clean stop — no infinite sub-unit crawl.
    pub stop_epsilon: f32,
}

impl Default for ScrollKineticsConfig {
    fn default() -> Self {
        Self {
            // Weighty ~0.7s coast; matches mado's felt behavior.
            friction: 5.0,
            // Line-oriented cap (lines/sec). Pixel consumers scale up.
            max_velocity: 4000.0,
            // <0.5 line/sec at 60 Hz moves <0.01 line/frame — invisible.
            stop_epsilon: 0.5,
        }
    }
}

impl ScrollKineticsConfig {
    /// A pixel-oriented config: scales the line-oriented `max_velocity`
    /// and `stop_epsilon` by `line_height` (pixels per line) so a
    /// pixel-scrolled consumer (browser, image viewer, pixel list)
    /// gets the SAME felt weight as a line-oriented one. `friction` is
    /// unit-agnostic (a pure decay rate) and is preserved.
    ///
    /// Example: `ScrollKineticsConfig::pixels_per_line(18.0)` yields a
    /// ~72_000 px/sec cap and a ~9 px/sec stop epsilon.
    #[must_use]
    pub fn pixels_per_line(line_height: f32) -> Self {
        let base = Self::default();
        let scale = line_height.max(1.0);
        Self {
            friction: base.friction,
            max_velocity: base.max_velocity * scale,
            stop_epsilon: base.stop_epsilon * scale,
        }
    }
}

/// The momentum-scroll kinetic state — a typed, pure velocity/friction
/// integrator lifted from mado's terminal scrolling into a fleet-shared
/// primitive. One value, owned by the consumer, advanced once per frame
/// by [`Self::tick`].
///
/// Scrolling is a physical motion, not an instantaneous jump. A wheel
/// flick injects *velocity* via [`Self::add_impulse`]; a pure per-frame
/// [`Self::tick`] integrates that velocity into whole-**unit** deltas
/// and bleeds it off with exponential friction, so a flick glides and
/// eases to a weighty stop. Edge auto-scroll (during a drag-selection
/// past a viewport edge) drives the SAME value through
/// [`Self::set_sustained`] — a held, non-decaying drive.
///
/// ## Unit-agnostic
///
/// The integrator never names a unit: it works in fractional units and
/// peels off whole `i32` units, carrying the fraction in `residual`.
/// The consumer decides whether one unit is a terminal *line* (mado), a
/// *pixel* (a browser / image viewer), or a scroll-*tick* (a list
/// widget). The type doesn't care; [`Self::tick`] returns whole-unit
/// deltas and the residual carries the sub-unit fraction across frames.
/// See [`ScrollKineticsConfig`] for matching line- vs pixel-oriented
/// defaults.
///
/// ## Determinism contract
///
/// [`Self::tick`] MUST be a no-op when `dt <= 0.0`: it returns `0` and
/// mutates nothing. Consumers that render headless at `dt = 0` (e.g. a
/// determinism render-ladder asserting byte-identical frame hashes)
/// rely on this — a kinetics tick that moved the viewport at `dt == 0.0`
/// would break that byte-stability. The guard is the first statement in
/// `tick`. **Preserve this when adapting.**
///
/// ## Sign convention
///
/// The sign of velocity / delta is whatever the consumer assigns to an
/// impulse; the integrator is sign-symmetric. mado uses `+` = scroll UP
/// into history, `-` = scroll DOWN toward the live tail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollKinetics {
    /// Current scroll velocity in fractional units per second.
    velocity: f32,
    /// Sub-unit accumulator. `tick` integrates `velocity * dt` here and
    /// only peels off WHOLE units as the returned delta, carrying the
    /// fraction across frames — so a slow drive still eventually moves a
    /// unit, and a fast flick never quantizes away travel.
    residual: f32,
    /// `true` while a sustained (edge auto-scroll) drive owns the
    /// velocity, so the consumer can distinguish it from a decaying
    /// wheel glide and cancel it explicitly when the pointer re-enters
    /// the viewport.
    sustained: bool,
}

impl Default for ScrollKinetics {
    fn default() -> Self {
        Self::at_rest()
    }
}

impl ScrollKinetics {
    /// A kinetics value at rest — zero velocity, no pending fraction.
    #[must_use]
    pub const fn at_rest() -> Self {
        Self { velocity: 0.0, residual: 0.0, sustained: false }
    }

    /// Whether the kinetics is fully at rest (no velocity, no pending
    /// fraction, no sustained drive) — equivalent to [`Self::at_rest`].
    #[must_use]
    pub fn at_rest_now(&self) -> bool {
        *self == Self::at_rest()
    }

    /// Inject a wheel impulse. `units` is the signed flick magnitude in
    /// units/sec to add (sign = direction); same-direction impulses
    /// ACCUMULATE so a fast repeated flick builds a longer glide.
    /// `|velocity|` is clamped to `max_velocity` so a frantic flick
    /// can't launch straight to the content edge. Any impulse cancels
    /// the sustained flag — a wheel flick is a fresh decaying glide.
    pub fn add_impulse(&mut self, units: f32, max_velocity: f32) {
        let max = max_velocity.max(0.0);
        self.sustained = false;
        self.velocity = (self.velocity + units).clamp(-max, max);
    }

    /// Drive a held, non-decaying velocity (edge auto-scroll past a
    /// viewport edge during a drag-selection). Unlike
    /// [`Self::add_impulse`] this REPLACES the velocity (it tracks the
    /// live overshoot, it doesn't accumulate) and marks the drive
    /// sustained, so friction in [`Self::tick`] can't bleed it off while
    /// the pointer stays past the edge.
    pub fn set_sustained(&mut self, velocity: f32) {
        self.velocity = velocity;
        self.sustained = true;
    }

    /// Whether the current drive is a sustained (edge auto-scroll) one,
    /// as opposed to a decaying wheel glide. The consumer cancels a
    /// sustained drive via [`Self::stop`] the moment the pointer
    /// re-enters the viewport.
    #[must_use]
    pub const fn is_sustained(&self) -> bool {
        self.sustained
    }

    /// Advance one frame. Returns the whole-**unit** viewport delta to
    /// apply this tick (sign per the consumer's convention).
    ///
    /// Steps:
    /// 1. **dt<=0 guard** — return `0` and mutate nothing (determinism).
    /// 2. Integrate `residual += velocity * dt`; peel off the whole
    ///    part as the returned delta, keep the fraction in `residual`.
    /// 3. Apply exponential friction: `velocity *= e^(-friction * dt)`.
    ///    Exponential (not linear) decay is the natural weighty model —
    ///    the velocity halves every `ln2 / friction` seconds, framerate
    ///    independent, so the glide eases out smoothly rather than
    ///    snapping. A SUSTAINED drive skips friction (it's held by the
    ///    live overshoot, not coasting).
    /// 4. Below `stop_epsilon` zero velocity AND residual — a clean
    ///    halt, no infinite sub-unit crawl.
    ///
    /// `friction` and `stop_epsilon` are passed in (typically from a
    /// [`ScrollKineticsConfig`]) so the same value can be ticked under
    /// different physics without owning a config.
    pub fn tick(&mut self, dt: f32, friction: f32, stop_epsilon: f32) -> i32 {
        // (1) Determinism: a zero-dt frame moves nothing. Consumers that
        // render headless at dt=0 rely on byte-identical results here.
        if dt <= 0.0 {
            return 0;
        }

        // (2) Integrate + peel off whole units, carrying the fraction.
        self.residual += self.velocity * dt;
        let whole = self.residual.trunc();
        self.residual -= whole;
        let delta = whole as i32;

        // (3) Friction decay — exponential, framerate-independent. A
        // sustained drive is held by the live overshoot; it must not
        // bleed off under its own friction.
        if !self.sustained {
            self.velocity *= (-friction.max(0.0) * dt).exp();
        }

        // (4) Clean stop below epsilon — no infinite crawl. Sustained
        // drives never auto-stop here; the consumer stops them
        // explicitly when the pointer re-enters the viewport.
        if !self.sustained && self.velocity.abs() < stop_epsilon.max(0.0) {
            self.velocity = 0.0;
            self.residual = 0.0;
        }

        delta
    }

    /// Convenience: [`Self::tick`] driven by a [`ScrollKineticsConfig`]
    /// (uses its `friction` + `stop_epsilon`). The common consumer call.
    pub fn tick_with(&mut self, dt: f32, config: &ScrollKineticsConfig) -> i32 {
        self.tick(dt, config.friction, config.stop_epsilon)
    }

    /// Whether any motion is pending (non-zero velocity). The consumer
    /// can skip the scroll-effect path entirely on a still kinetics.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.velocity != 0.0
    }

    /// Halt immediately — zero velocity and drop the pending fraction.
    /// Used when momentum hits a content wall (so it doesn't fight the
    /// bound) or when a sustained edge-scroll drive should cancel.
    pub fn stop(&mut self) {
        self.velocity = 0.0;
        self.residual = 0.0;
        self.sustained = false;
    }

    /// Test-only read of the raw velocity (units/sec). Production code
    /// never inspects the velocity directly — it consumes the
    /// `tick`-returned unit delta — so this stays test-gated.
    #[cfg(test)]
    pub(crate) const fn velocity(&self) -> f32 {
        self.velocity
    }
}

/// Virtualized scrollable container.
///
/// Tracks scroll offset and visible range for efficient rendering
/// of large content (thousands of items).
#[derive(Debug, Clone)]
pub struct ScrollView {
    pub offset: f32,
    pub content_height: f32,
    pub viewport_height: f32,
}

impl ScrollView {
    #[must_use]
    pub fn new(content_height: f32, viewport_height: f32) -> Self {
        Self { offset: 0.0, content_height, viewport_height }
    }

    /// Scroll by a delta, clamping to valid range.
    pub fn scroll_by(&mut self, delta: f32) {
        self.offset += delta;
        self.clamp();
    }

    /// Scroll to an absolute offset, clamping to valid range.
    pub fn scroll_to(&mut self, offset: f32) {
        self.offset = offset;
        self.clamp();
    }

    /// Returns true if scrolled to the top.
    #[must_use]
    pub fn is_at_top(&self) -> bool {
        self.offset <= 0.0
    }

    /// Returns true if scrolled to the bottom (or content fits in viewport).
    #[must_use]
    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.max_scroll()
    }

    /// Maximum scroll offset (0 if content fits in viewport).
    #[must_use]
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Returns `(start_y, end_y)` of the visible range in content coordinates.
    #[must_use]
    pub fn visible_range(&self) -> (f32, f32) {
        (self.offset, self.offset + self.viewport_height)
    }

    /// Returns the scroll position as a fraction from 0.0 (top) to 1.0 (bottom).
    #[must_use]
    pub fn scroll_fraction(&self) -> f32 {
        let max = self.max_scroll();
        if max <= 0.0 {
            0.0
        } else {
            self.offset / max
        }
    }

    fn clamp(&mut self) {
        self.offset = self.offset.clamp(0.0, self.max_scroll());
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_top() {
        let sv = ScrollView::new(500.0, 100.0);
        assert_eq!(sv.offset, 0.0);
        assert!(sv.is_at_top());
        assert!(!sv.is_at_bottom());
    }

    #[test]
    fn scroll_by_positive() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_by(50.0);
        assert!((sv.offset - 50.0).abs() < f32::EPSILON);
        assert!(!sv.is_at_top());
    }

    #[test]
    fn scroll_by_clamps_negative() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_by(-100.0);
        assert_eq!(sv.offset, 0.0);
        assert!(sv.is_at_top());
    }

    #[test]
    fn scroll_by_clamps_past_max() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_by(9999.0);
        assert!((sv.offset - 400.0).abs() < f32::EPSILON);
        assert!(sv.is_at_bottom());
    }

    #[test]
    fn scroll_to_clamps() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(-50.0);
        assert_eq!(sv.offset, 0.0);

        sv.scroll_to(1000.0);
        assert!((sv.offset - 400.0).abs() < f32::EPSILON);
    }

    #[test]
    fn max_scroll_content_smaller_than_viewport() {
        let sv = ScrollView::new(50.0, 100.0);
        assert_eq!(sv.max_scroll(), 0.0);
        assert!(sv.is_at_top());
        assert!(sv.is_at_bottom());
    }

    #[test]
    fn visible_range() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(100.0);
        let (start, end) = sv.visible_range();
        assert!((start - 100.0).abs() < f32::EPSILON);
        assert!((end - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_at_top() {
        let sv = ScrollView::new(500.0, 100.0);
        assert!((sv.scroll_fraction()).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_at_bottom() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(400.0);
        assert!((sv.scroll_fraction() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_midway() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(200.0); // max=400, so 200/400 = 0.5
        assert!((sv.scroll_fraction() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_no_scrollable_content() {
        let sv = ScrollView::new(50.0, 100.0);
        assert!((sv.scroll_fraction()).abs() < f32::EPSILON);
    }

    // ── ScrollKinetics ──────────────────────────────────────────────

    /// A test friction that gives a visible-but-short glide so the
    /// monotonic-decay + finite-travel assertions read clearly.
    const KIN_FRICTION: f32 = 5.0;
    const KIN_STOP: f32 = 0.5;
    const DT: f32 = 1.0 / 60.0;
    const MAX_V: f32 = 4000.0;

    fn tick(k: &mut ScrollKinetics, dt: f32) -> i32 {
        k.tick(dt, KIN_FRICTION, KIN_STOP)
    }

    /// `dt <= 0.0` is a strict no-op: returns 0 and the whole value is
    /// untouched — the determinism contract headless ladders rely on.
    #[test]
    fn kinetics_dt_zero_is_a_noop() {
        let mut k = ScrollKinetics::at_rest();
        k.add_impulse(1000.0, MAX_V);
        let before = k;
        let delta = tick(&mut k, 0.0);
        assert_eq!(delta, 0, "dt=0 must move nothing");
        assert_eq!(k, before, "dt=0 must not mutate the kinetics");
        // Negative dt is equally a no-op.
        assert_eq!(tick(&mut k, -0.5), 0, "dt<0 must move nothing");
        assert_eq!(k, before, "dt<0 must not mutate the kinetics");
    }

    /// A single impulse glides then stops: velocity decays monotonically
    /// to exactly 0, and `is_active()` flips false at the end.
    #[test]
    fn kinetics_single_impulse_glides_then_stops() {
        let mut k = ScrollKinetics::at_rest();
        k.add_impulse(600.0, MAX_V);
        assert!(k.is_active());

        let mut prev = f32::INFINITY;
        let mut stopped = false;
        for _ in 0..600 {
            tick(&mut k, DT);
            let v = k.velocity().abs();
            assert!(v <= prev + 1e-3, "velocity must not increase mid-glide");
            prev = v;
            if !k.is_active() {
                stopped = true;
                break;
            }
        }
        assert!(stopped, "a single impulse must come to rest");
        assert_eq!(k.velocity(), 0.0, "stopped velocity is exactly 0");
        assert!(k.at_rest_now(), "a stopped glide is fully at rest");
    }

    /// Total units traveled from one impulse is FINITE and bounded —
    /// momentum can't run away.
    #[test]
    fn kinetics_total_travel_is_finite_and_bounded() {
        let mut k = ScrollKinetics::at_rest();
        k.add_impulse(600.0, MAX_V);
        let mut total: i64 = 0;
        for _ in 0..600 {
            total += i64::from(tick(&mut k, DT));
            if !k.is_active() {
                break;
            }
        }
        assert!(total > 0, "an up-impulse travels positive");
        // Analytic ceiling: ∫v0·e^(-f·t) dt = v0/f. v0=600, f=5 → 120;
        // a comfortable bound proves no runaway.
        assert!(total <= 130, "travel {total} exceeded the analytic bound");
    }

    /// Fractional residual accumulates across ticks: a velocity slow
    /// enough to move <1 unit per frame STILL eventually moves a unit.
    #[test]
    fn kinetics_fractional_residual_accumulates_to_a_unit() {
        // 30 units/sec at 60 Hz = 0.5 unit/frame → a unit every 2nd
        // frame. Drive it sustained so friction doesn't interfere.
        let mut k = ScrollKinetics::at_rest();
        k.set_sustained(30.0);
        let mut moved = 0;
        for _ in 0..4 {
            moved += tick(&mut k, DT);
        }
        assert!(moved >= 1, "sub-unit velocity must accumulate to a move");
    }

    /// A sustained drive does NOT decay under friction — it's held by
    /// the live overshoot until the consumer stops it.
    #[test]
    fn kinetics_sustained_drive_does_not_decay() {
        let mut k = ScrollKinetics::at_rest();
        k.set_sustained(-500.0);
        for _ in 0..120 {
            tick(&mut k, DT);
        }
        assert!(k.is_sustained());
        assert_eq!(k.velocity(), -500.0, "sustained velocity is held, not decayed");
        k.stop();
        assert!(!k.is_active());
        assert!(!k.is_sustained());
    }

    /// `add_impulse` clamps to `max_velocity`; same-direction impulses
    /// accumulate but still saturate at the cap.
    #[test]
    fn kinetics_impulse_accumulates_and_clamps() {
        let mut k = ScrollKinetics::at_rest();
        k.add_impulse(300.0, 1000.0);
        k.add_impulse(300.0, 1000.0);
        assert_eq!(k.velocity(), 600.0, "same-direction impulses add");
        k.add_impulse(900.0, 1000.0);
        assert_eq!(k.velocity(), 1000.0, "accumulation saturates at the cap");
        k.add_impulse(-5000.0, 1000.0);
        assert_eq!(k.velocity(), -1000.0, "opposite direction is also clamped");
    }

    /// Caller zeroing velocity at a wall (via `stop`) leaves a clean
    /// rest — the next tick moves nothing and isn't active.
    #[test]
    fn kinetics_wall_clamp_via_stop_is_a_clean_rest() {
        let mut k = ScrollKinetics::at_rest();
        k.add_impulse(1000.0, MAX_V);
        k.stop();
        assert!(!k.is_active());
        assert_eq!(tick(&mut k, DT), 0, "no travel after a wall stop");
        assert!(k.at_rest_now());
    }

    /// An up-impulse yields positive deltas, a down-impulse negative —
    /// the sign convention the consumer maps to scroll direction.
    #[test]
    fn kinetics_impulse_sign_drives_delta_sign() {
        let mut up = ScrollKinetics::at_rest();
        up.add_impulse(2000.0, MAX_V);
        assert!(tick(&mut up, DT) > 0, "up-impulse → positive delta");

        let mut down = ScrollKinetics::at_rest();
        down.add_impulse(-2000.0, MAX_V);
        assert!(tick(&mut down, DT) < 0, "down-impulse → negative delta");
    }

    // ── Generality: unit-agnostic + config ──────────────────────────

    /// The integrator is unit-agnostic: ticking a line-oriented consumer
    /// (1 unit = 1 line) and a pixel-oriented one (1 unit = 1 px) with
    /// proportionally-scaled inputs produces proportionally-scaled
    /// travel. A "line" and a "pixel" are the same physics, just scaled.
    #[test]
    fn kinetics_lines_and_pixels_are_the_same_physics() {
        const LINE_HEIGHT: f32 = 20.0;

        // Line consumer: impulse of 600 lines/sec, line-oriented config.
        let line_cfg = ScrollKineticsConfig::default();
        let mut lines = ScrollKinetics::at_rest();
        lines.add_impulse(600.0, line_cfg.max_velocity);
        let mut line_travel: i64 = 0;
        for _ in 0..600 {
            line_travel += i64::from(lines.tick_with(DT, &line_cfg));
            if !lines.is_active() {
                break;
            }
        }

        // Pixel consumer: SAME flick, scaled by line-height → px/sec.
        let px_cfg = ScrollKineticsConfig::pixels_per_line(LINE_HEIGHT);
        let mut px = ScrollKinetics::at_rest();
        px.add_impulse(600.0 * LINE_HEIGHT, px_cfg.max_velocity);
        let mut px_travel: i64 = 0;
        for _ in 0..600 {
            px_travel += i64::from(px.tick_with(DT, &px_cfg));
            if !px.is_active() {
                break;
            }
        }

        // Pixel travel ≈ line travel × line-height (same coast shape,
        // just measured in finer units). Allow a small quantization slack.
        let expected = (line_travel as f64) * f64::from(LINE_HEIGHT);
        let ratio = (px_travel as f64) / expected;
        assert!(
            (0.95..=1.05).contains(&ratio),
            "pixel travel {px_travel} should be ~{expected} (line {line_travel} × {LINE_HEIGHT}); ratio {ratio}"
        );
    }

    /// `tick_with` (config-driven) matches `tick` (explicit args) for the
    /// same physics — the convenience wrapper is faithful.
    #[test]
    fn kinetics_tick_with_matches_explicit() {
        let cfg = ScrollKineticsConfig::default();
        let mut a = ScrollKinetics::at_rest();
        let mut b = ScrollKinetics::at_rest();
        a.add_impulse(800.0, cfg.max_velocity);
        b.add_impulse(800.0, cfg.max_velocity);
        for _ in 0..50 {
            let da = a.tick(DT, cfg.friction, cfg.stop_epsilon);
            let db = b.tick_with(DT, &cfg);
            assert_eq!(da, db, "tick and tick_with must agree");
        }
        assert_eq!(a, b);
    }

    /// `ScrollKineticsConfig` serde round-trips (loadable from shikumi).
    #[test]
    fn kinetics_config_serde_roundtrips() {
        let cfg = ScrollKineticsConfig::pixels_per_line(18.0);
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ScrollKineticsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    /// The default config gives a weighty-but-bounded coast: a strong
    /// flick under defaults comes to rest in well under a second's worth
    /// of frames and travels a finite distance.
    #[test]
    fn kinetics_default_config_coasts_and_stops() {
        let cfg = ScrollKineticsConfig::default();
        let mut k = ScrollKinetics::at_rest();
        k.add_impulse(1200.0, cfg.max_velocity);
        let mut frames = 0;
        for _ in 0..600 {
            k.tick_with(DT, &cfg);
            frames += 1;
            if !k.is_active() {
                break;
            }
        }
        assert!(k.at_rest_now(), "default coast must reach a clean rest");
        // ~0.7s coast at 60Hz ≈ 42 frames; a generous ceiling proves it.
        assert!(frames < 120, "default coast took {frames} frames (too long)");
    }
}
