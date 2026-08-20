//! The compose stack: what to run, and what it says back.
//!
//! Every command is a value rather than a call, so a test can assert what a step would do
//! without a docker daemon.

use std::path::Path;

use anyhow::{Context, Result};

use crate::exec::Runner;

/// `docker compose --profile full <args>`, which is how the whole stack is addressed.
pub fn compose(args: &[&str]) -> Vec<String> {
    let mut out = vec!["compose".to_string(), "--profile".into(), "full".into()];
    out.extend(args.iter().map(|a| a.to_string()));
    out
}

fn as_refs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

pub fn build(runner: &mut dyn Runner, dir: &Path) -> Result<()> {
    runner.run(dir, "docker", &as_refs(&compose(&["build"])))
}

pub fn up(runner: &mut dyn Runner, dir: &Path) -> Result<()> {
    runner.run(dir, "docker", &as_refs(&compose(&["up", "-d"])))
}

pub fn restart(runner: &mut dyn Runner, dir: &Path, service: &str) -> Result<()> {
    runner.run(dir, "docker", &as_refs(&compose(&["restart", service])))
}

pub fn ps(runner: &mut dyn Runner, dir: &Path) -> Result<String> {
    runner.capture(dir, "docker", &as_refs(&compose(&["ps"])))
}

/// One-shot command in a throwaway api container, with no dependencies started.
pub fn api(runner: &mut dyn Runner, dir: &Path, args: &[&str]) -> Result<String> {
    let mut full = vec!["run", "--rm", "--no-deps", "-T", "api", "wormholesystems"];
    full.extend_from_slice(args);
    runner.capture(dir, "docker", &as_refs(&compose(&full)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdeStatus {
    pub loaded: String,
    pub latest: String,
    pub update_available: bool,
}

/// Parses the `key=value` lines `sde-status` prints.
pub fn parse_sde_status(output: &str) -> Option<SdeStatus> {
    let field = |name: &str| {
        output.lines().find_map(|line| {
            line.trim()
                .strip_prefix(&format!("{name}="))
                .map(|v| v.split_whitespace().next().unwrap_or(v).to_string())
        })
    };
    Some(SdeStatus {
        loaded: field("loaded")?,
        latest: field("latest").unwrap_or_default(),
        update_available: field("update_available").as_deref() == Some("yes"),
    })
}

pub fn sde_status(runner: &mut dyn Runner, dir: &Path) -> Result<SdeStatus> {
    let out = api(runner, dir, &["sde-status"])?;
    parse_sde_status(&out).context("could not read the SDE status; is the stack built?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::Recording;

    #[test]
    fn the_stack_is_always_addressed_through_the_full_profile() {
        // Without the profile, `up` starts the database alone and nothing else, which looks
        // like a broken install rather than a missing flag.
        let mut runner = Recording::default();
        up(&mut runner, Path::new(".")).unwrap();
        build(&mut runner, Path::new(".")).unwrap();
        assert_eq!(
            runner.commands,
            vec![
                "docker compose --profile full up -d",
                "docker compose --profile full build",
            ]
        );
    }

    #[test]
    fn a_one_shot_command_takes_no_dependencies_with_it() {
        let mut runner = Recording::default();
        api(&mut runner, Path::new("."), &["sde-status"]).unwrap();
        assert_eq!(
            runner.commands,
            vec![
                "docker compose --profile full run --rm --no-deps -T api wormholesystems sde-status"
            ]
        );
    }

    #[test]
    fn reads_the_sde_status_lines() {
        let out = "loaded=3409592 released=2026-06-25\nlatest=3473160\nupdate_available=yes";
        assert_eq!(
            parse_sde_status(out),
            Some(SdeStatus {
                loaded: "3409592".into(),
                latest: "3473160".into(),
                update_available: true,
            })
        );
    }

    #[test]
    fn a_current_build_is_not_an_update() {
        let out = "loaded=3473160 released=2026-08-19\nlatest=3473160\nupdate_available=no";
        assert!(!parse_sde_status(out).unwrap().update_available);
    }

    #[test]
    fn nothing_useful_reads_as_nothing() {
        assert_eq!(parse_sde_status("bash: no such container"), None);
    }
}
