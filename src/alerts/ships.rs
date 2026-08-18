//! What can jump, and how far.
//!
//! Jump range is a property of the hull class rather than the hull: every dreadnought
//! reaches the same distance, so the choice a person makes is "a dreadnought", not "a
//! Naglfar". Jump Drive Calibration then adds 20% of the base range per level.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JumpShip {
    Dreadnought,
    Carrier,
    ForceAuxiliary,
    Supercarrier,
    Titan,
    JumpFreighter,
    Rorqual,
    BlackOps,
}

impl JumpShip {
    pub const ALL: [JumpShip; 8] = [
        JumpShip::Dreadnought,
        JumpShip::Carrier,
        JumpShip::ForceAuxiliary,
        JumpShip::Supercarrier,
        JumpShip::Titan,
        JumpShip::JumpFreighter,
        JumpShip::Rorqual,
        JumpShip::BlackOps,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            JumpShip::Dreadnought => "dreadnought",
            JumpShip::Carrier => "carrier",
            JumpShip::ForceAuxiliary => "force_auxiliary",
            JumpShip::Supercarrier => "supercarrier",
            JumpShip::Titan => "titan",
            JumpShip::JumpFreighter => "jump_freighter",
            JumpShip::Rorqual => "rorqual",
            JumpShip::BlackOps => "black_ops",
        }
    }

    pub fn parse(value: &str) -> Option<JumpShip> {
        JumpShip::ALL.into_iter().find(|s| s.as_str() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            JumpShip::Dreadnought => "Dreadnought",
            JumpShip::Carrier => "Carrier",
            JumpShip::ForceAuxiliary => "Force Auxiliary",
            JumpShip::Supercarrier => "Supercarrier",
            JumpShip::Titan => "Titan",
            JumpShip::JumpFreighter => "Jump Freighter",
            JumpShip::Rorqual => "Rorqual",
            JumpShip::BlackOps => "Black Ops",
        }
    }

    /// Range at JDC 0, in light years.
    pub fn base_range_ly(self) -> f64 {
        match self {
            JumpShip::Dreadnought | JumpShip::Carrier | JumpShip::ForceAuxiliary => 3.5,
            JumpShip::Supercarrier | JumpShip::Titan => 3.0,
            JumpShip::JumpFreighter | JumpShip::Rorqual => 5.0,
            JumpShip::BlackOps => 4.0,
        }
    }

    /// Range at a given Jump Drive Calibration level: +20% of base per level.
    pub fn max_range_ly(self, jdc_level: i32) -> f64 {
        self.base_range_ly() * (1.0 + 0.2 * f64::from(jdc_level.clamp(0, 5)))
    }

    /// A representative hull, for Dotlan's range map links. Every hull in a class shares
    /// the range, so which one is arbitrary as long as it is in the class.
    pub fn dotlan_hull(self) -> &'static str {
        match self {
            JumpShip::Dreadnought => "Naglfar",
            JumpShip::Carrier => "Archon",
            JumpShip::ForceAuxiliary => "Apostle",
            JumpShip::Supercarrier => "Nyx",
            JumpShip::Titan => "Avatar",
            JumpShip::JumpFreighter => "Rhea",
            JumpShip::Rorqual => "Rorqual",
            JumpShip::BlackOps => "Redeemer",
        }
    }
}

/// Metres in a light year. The SDE stores positions in metres.
pub const METRES_PER_LIGHTYEAR: f64 = 9_460_730_472_580_800.0;

/// Straight-line distance between two systems, in light years.
pub fn distance_ly(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let (dx, dy, dz) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
    (dx * dx + dy * dy + dz * dz).sqrt() / METRES_PER_LIGHTYEAR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jdc_adds_a_fifth_of_the_base_range_per_level() {
        assert_eq!(JumpShip::Dreadnought.max_range_ly(0), 3.5);
        assert_eq!(JumpShip::Dreadnought.max_range_ly(5), 7.0);
        assert_eq!(JumpShip::JumpFreighter.max_range_ly(5), 10.0);
        // Titans are the shortest-ranged of the capitals, which is the whole reason the
        // hull matters rather than just "a capital".
        assert!(JumpShip::Titan.max_range_ly(5) < JumpShip::Dreadnought.max_range_ly(5));
    }

    #[test]
    fn an_impossible_skill_level_is_clamped_rather_than_believed() {
        assert_eq!(
            JumpShip::Carrier.max_range_ly(99),
            JumpShip::Carrier.max_range_ly(5)
        );
        assert_eq!(
            JumpShip::Carrier.max_range_ly(-1),
            JumpShip::Carrier.max_range_ly(0)
        );
    }

    #[test]
    fn distance_is_measured_in_light_years() {
        let origin = (0.0, 0.0, 0.0);
        let one_ly = (METRES_PER_LIGHTYEAR, 0.0, 0.0);
        assert!((distance_ly(origin, one_ly) - 1.0).abs() < 1e-9);
        assert_eq!(distance_ly(origin, origin), 0.0);

        // And in three dimensions, not two.
        let diagonal = (METRES_PER_LIGHTYEAR, METRES_PER_LIGHTYEAR, 0.0);
        assert!((distance_ly(origin, diagonal) - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn every_ship_round_trips_through_its_stored_name() {
        for ship in JumpShip::ALL {
            assert_eq!(JumpShip::parse(ship.as_str()), Some(ship));
        }
        assert_eq!(JumpShip::parse("frigate"), None);
    }
}
