//! Motion presets (PRD 13.5, Motion): critically damped springs, so nothing
//! overshoots, and a short cross-fade in their place under Reduce Motion.
//! Phases 10 and 11 animate the drill-down zoom, the inspector and the sheet
//! with these.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// Time to reach about 99% of the way, in Apple's terms; not a fixed duration.
    pub response: Duration,
}

/// Drill-down zoom, inspector, sheet.
pub const NAV: Spring = Spring {
    response: Duration::from_millis(350),
};
/// Small state changes.
pub const SMALL: Spring = Spring {
    response: Duration::from_millis(250),
};
pub const CROSS_FADE: Duration = Duration::from_millis(150);

/// GPUI animations need an end; at 1.5 responses the spring is within 0.1%
/// of rest, which no one can see.
const SETTLE: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transition {
    Move { spring: Spring, duration: Duration },
    CrossFade { duration: Duration },
}

impl Transition {
    pub fn new(spring: Spring, reduce_motion: bool) -> Self {
        if reduce_motion {
            Transition::CrossFade {
                duration: CROSS_FADE,
            }
        } else {
            Transition::Move {
                spring,
                duration: spring.response.mul_f32(SETTLE),
            }
        }
    }
}

impl Transition {
    pub fn duration(self) -> Duration {
        match self {
            Transition::Move { duration, .. } | Transition::CrossFade { duration } => duration,
        }
    }
}

/// Progress of a critically damped spring at `t` (0 to 1 of the animation's
/// duration), scaled so it ends exactly at 1. Usable as a GPUI easing function.
pub fn ease(t: f32) -> f32 {
    let omega_end = std::f32::consts::TAU * SETTLE;
    let step = |x: f32| 1. - (1. + x) * (-x).exp();
    step(omega_end * t.clamp(0., 1.)) / step(omega_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_starts_at_zero() {
        assert_eq!(ease(0.), 0.);
    }

    #[test]
    fn ease_ends_at_one() {
        assert_eq!(ease(1.), 1.);
    }

    #[test]
    fn ease_never_goes_back_or_overshoots() {
        let samples: Vec<f32> = (0..=100).map(|i| ease(i as f32 / 100.)).collect();
        assert!(samples.windows(2).all(|w| w[0] <= w[1] && w[1] <= 1.));
    }

    #[test]
    fn reduce_motion_cross_fades() {
        assert_eq!(
            Transition::new(NAV, true),
            Transition::CrossFade {
                duration: Duration::from_millis(150)
            }
        );
    }

    #[test]
    fn navigation_settles_in_one_and_a_half_responses() {
        assert_eq!(
            Transition::new(NAV, false),
            Transition::Move {
                spring: NAV,
                duration: Duration::from_millis(525)
            }
        );
    }
}
