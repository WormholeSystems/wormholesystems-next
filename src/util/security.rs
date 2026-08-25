//! Security status the way EVE rounds it, ported from Nohus' CCPRounding class
//! (https://gitlab.com/rift-intel-fusion-tool/) via the legacy app. Every comparison or
//! band check on a security status goes through this first; raw SDE values sit slightly
//! off the displayed tenths (Uedama is 0.46) and comparing them directly misclassifies.

/// Round a raw security status to the value the game shows: exactly 0.0 stays 0.0,
/// anything in (0, 0.05) rounds up to 0.1 (a positive true sec is always at least
/// lowsec), everything else rounds to the nearest tenth, halves away from zero.
pub fn ccp_round_security(security: f64) -> f64 {
    if security == 0.0 {
        0.0
    } else if security > 0.0 && security < 0.05 {
        0.1
    } else {
        (security * 10.0).round() / 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_to_the_displayed_tenth() {
        assert_eq!(ccp_round_security(0.9134), 0.9);
        assert_eq!(ccp_round_security(0.4552), 0.5);
        assert_eq!(ccp_round_security(0.4442), 0.4);
    }

    #[test]
    fn barely_positive_rounds_up_to_lowsec() {
        assert_eq!(ccp_round_security(0.02), 0.1);
        assert_eq!(ccp_round_security(0.0001), 0.1);
    }

    #[test]
    fn zero_stays_and_negatives_round_away_from_zero() {
        assert_eq!(ccp_round_security(0.0), 0.0);
        assert_eq!(ccp_round_security(-0.014694), 0.0);
        assert_eq!(ccp_round_security(-0.05), -0.1);
        assert_eq!(ccp_round_security(-0.987), -1.0);
    }
}
