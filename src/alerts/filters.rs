//! Which kills an alert cares about.
//!
//! A rule names a subject (a ship, a pilot, an organisation), a side of the killmail, and
//! whether matching it means include or exclude. Rules combine per the alert's match mode:
//! `any` fires when one include rule matches, `all` when every one does. An exclude rule
//! always vetoes, whatever the mode, and an alert with no rules matches every kill.

use serde::{Deserialize, Serialize};

/// What a rule is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Subject {
    ShipType,
    ShipGroup,
    Character,
    Corporation,
    Alliance,
}

/// Which end of the killmail to look at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Side {
    Victim,
    /// The killing blow. WormholeSystems keeps that one attacker, not the whole gang.
    Attacker,
    Either,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Mode {
    Include,
    Exclude,
}

/// How include rules combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Match {
    Any,
    All,
}

impl Match {
    pub fn as_str(self) -> &'static str {
        match self {
            Match::Any => "any",
            Match::All => "all",
        }
    }

    pub fn parse(value: &str) -> Option<Match> {
        match value {
            "any" => Some(Match::Any),
            "all" => Some(Match::All),
            _ => None,
        }
    }
}

/// One rule. The ids within a rule are an OR: "any of these alliances".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Rule {
    pub subject: Subject,
    pub side: Side,
    pub mode: Mode,
    pub ids: Vec<i64>,
}

/// The ids a killmail offers, per side.
#[derive(Debug, Clone, Default)]
pub struct Candidates {
    pub victim_character: Option<i64>,
    pub victim_corporation: Option<i64>,
    pub victim_alliance: Option<i64>,
    pub victim_ship_type: Option<i64>,
    pub victim_ship_group: Option<i64>,
    pub attacker_character: Option<i64>,
    pub attacker_corporation: Option<i64>,
    pub attacker_alliance: Option<i64>,
    pub attacker_ship_type: Option<i64>,
    pub attacker_ship_group: Option<i64>,
}

impl Candidates {
    fn ids(&self, subject: Subject, side: Side) -> Vec<i64> {
        let victim = match subject {
            Subject::ShipType => self.victim_ship_type,
            Subject::ShipGroup => self.victim_ship_group,
            Subject::Character => self.victim_character,
            Subject::Corporation => self.victim_corporation,
            Subject::Alliance => self.victim_alliance,
        };
        let attacker = match subject {
            Subject::ShipType => self.attacker_ship_type,
            Subject::ShipGroup => self.attacker_ship_group,
            Subject::Character => self.attacker_character,
            Subject::Corporation => self.attacker_corporation,
            Subject::Alliance => self.attacker_alliance,
        };
        match side {
            Side::Victim => victim.into_iter().collect(),
            Side::Attacker => attacker.into_iter().collect(),
            Side::Either => victim.into_iter().chain(attacker).collect(),
        }
    }
}

fn hits(rule: &Rule, candidates: &Candidates) -> bool {
    let found = candidates.ids(rule.subject, rule.side);
    found.iter().any(|id| rule.ids.contains(id))
}

/// Whether a killmail passes an alert's rules.
pub fn matches(rules: &[Rule], mode: Match, candidates: &Candidates) -> bool {
    if rules.is_empty() {
        return true;
    }
    // An exclusion is absolute: it is how you say "everything except the locals", and a
    // match mode that could out-vote it would make that unsayable.
    if rules
        .iter()
        .filter(|r| r.mode == Mode::Exclude)
        .any(|r| hits(r, candidates))
    {
        return false;
    }
    let includes: Vec<&Rule> = rules.iter().filter(|r| r.mode == Mode::Include).collect();
    if includes.is_empty() {
        return true;
    }
    match mode {
        Match::Any => includes.iter().any(|r| hits(r, candidates)),
        Match::All => includes.iter().all(|r| hits(r, candidates)),
    }
}

/// The rules that actually matched, narrowed to the ids that did, for the message.
pub fn matched(rules: &[Rule], candidates: &Candidates) -> Vec<Rule> {
    rules
        .iter()
        .filter(|r| r.mode == Mode::Include)
        .filter_map(|rule| {
            let found = candidates.ids(rule.subject, rule.side);
            let ids: Vec<i64> = rule
                .ids
                .iter()
                .copied()
                .filter(|id| found.contains(id))
                .collect();
            (!ids.is_empty()).then(|| Rule {
                ids,
                ..rule.clone()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(subject: Subject, side: Side, mode: Mode, ids: &[i64]) -> Rule {
        Rule {
            subject,
            side,
            mode,
            ids: ids.to_vec(),
        }
    }

    fn kill() -> Candidates {
        Candidates {
            victim_character: Some(11),
            victim_corporation: Some(22),
            victim_alliance: Some(33),
            victim_ship_type: Some(670),
            attacker_character: Some(44),
            attacker_corporation: Some(55),
            attacker_alliance: Some(66),
            attacker_ship_type: Some(29990),
            ..Default::default()
        }
    }

    #[test]
    fn no_rules_matches_everything() {
        assert!(matches(&[], Match::Any, &kill()));
    }

    #[test]
    fn sides_are_respected() {
        let victim = rule(Subject::Alliance, Side::Victim, Mode::Include, &[66]);
        assert!(!matches(&[victim], Match::Any, &kill()));
        let either = rule(Subject::Alliance, Side::Either, Mode::Include, &[66]);
        assert!(matches(&[either], Match::Any, &kill()));
    }

    #[test]
    fn any_needs_one_and_all_needs_every() {
        let rules = vec![
            rule(Subject::Alliance, Side::Victim, Mode::Include, &[33]),
            rule(Subject::ShipType, Side::Victim, Mode::Include, &[999]),
        ];
        assert!(matches(&rules, Match::Any, &kill()));
        assert!(!matches(&rules, Match::All, &kill()));
    }

    /// The point of an exclude: "anything near us except our own losses".
    #[test]
    fn an_exclusion_vetoes_a_match() {
        let rules = vec![
            rule(Subject::Alliance, Side::Either, Mode::Include, &[33]),
            rule(Subject::Corporation, Side::Victim, Mode::Exclude, &[22]),
        ];
        assert!(!matches(&rules, Match::Any, &kill()));
    }

    #[test]
    fn only_the_ids_that_matched_are_reported() {
        let rules = vec![rule(
            Subject::Alliance,
            Side::Either,
            Mode::Include,
            &[33, 66, 99],
        )];
        let found = matched(&rules, &kill());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].ids, vec![33, 66]);
    }
}
