//! Running the commands a plan is made of.
//!
//! Behind a trait so the tests can assert exactly which commands a plan would run without
//! a docker daemon anywhere near them.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub trait Runner {
    /// Run to completion with the output going to the terminal, so a long build shows its
    /// progress rather than sitting silent.
    fn run(&mut self, dir: &Path, program: &str, args: &[&str]) -> Result<()>;

    /// Run quietly and hand back stdout, for the commands whose output we read.
    fn capture(&mut self, dir: &Path, program: &str, args: &[&str]) -> Result<String>;
}

pub struct Real;

impl Runner for Real {
    fn run(&mut self, dir: &Path, program: &str, args: &[&str]) -> Result<()> {
        let status = Command::new(program)
            .args(args)
            .current_dir(dir)
            .status()
            .with_context(|| format!("could not start `{program}`"))?;
        if !status.success() {
            bail!("`{program} {}` failed", args.join(" "));
        }
        Ok(())
    }

    fn capture(&mut self, dir: &Path, program: &str, args: &[&str]) -> Result<String> {
        let out = Command::new(program)
            .args(args)
            .current_dir(dir)
            .stderr(Stdio::null())
            .output()
            .with_context(|| format!("could not start `{program}`"))?;
        if !out.status.success() {
            bail!("`{program} {}` failed", args.join(" "));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

/// Records what it was asked to do and answers from a script, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct Recording {
    pub commands: Vec<String>,
    pub replies: std::collections::HashMap<String, String>,
}

#[cfg(test)]
impl Runner for Recording {
    fn run(&mut self, _dir: &Path, program: &str, args: &[&str]) -> Result<()> {
        self.commands.push(format!("{program} {}", args.join(" ")));
        Ok(())
    }

    fn capture(&mut self, _dir: &Path, program: &str, args: &[&str]) -> Result<String> {
        let line = format!("{program} {}", args.join(" "));
        self.commands.push(line.clone());
        Ok(self.replies.get(&line).cloned().unwrap_or_default())
    }
}
