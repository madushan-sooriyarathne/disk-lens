//! `scan-bench <path>`: scan a folder (read-only), print stats, and compare the
//! result with `du -skx` (goal G5: within 1%). Writes nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime};

use disklens_lib::platform::{Facts, ProcessRunner};
use disklens_lib::rules::guards::Env;
use disklens_lib::rules::matcher::Matcher;
use disklens_lib::rules::model::Rules;
use disklens_lib::rules::tools::Tools;
use disklens_lib::scanner::{self, ScanOptions};

const TOLERANCE_PCT: f64 = 1.0;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: scan-bench <path>");
        return ExitCode::from(2);
    };
    let started = Instant::now();
    let snap = match scanner::scan(
        &path,
        &ScanOptions::from_rules(Rules::embedded()),
        &Arc::new(AtomicBool::new(false)),
        &mut |p| {
            eprintln!(
                "{} files, {} MB, {}",
                p.files,
                p.bytes / 1_000_000,
                p.current
            )
        },
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("scan failed: {e}");
            return ExitCode::from(2);
        }
    };
    let elapsed = started.elapsed();
    let root = &snap.nodes[0];
    println!("path:         {}", path.display());
    println!("duration:     {:.2} s", elapsed.as_secs_f64());
    println!("files:        {}", root.file_count);
    println!("directories:  {}", snap.nodes.len());
    println!("inaccessible: {}", snap.meta.inaccessible_count);
    println!(
        "size:         {} KiB ({:.2} GB)",
        root.size / 1024,
        root.size as f64 / 1e9
    );

    // Rule matching only: match-time guards stat files and read git history; no
    // providers run, so nothing beyond the scanned folder is queried.
    let started = Instant::now();
    let rules = Rules::embedded();
    let home = std::env::var("HOME").unwrap_or_default();
    let tools = Tools::discover(Path::new(&home), false);
    let runner = ProcessRunner {
        timeout: Duration::from_secs(30),
        home,
    };
    let facts = Facts::default();
    let env = Env {
        facts: &facts,
        runner: &runner,
        tools: &tools,
        now: SystemTime::now(),
    };
    let matches = Matcher::new(rules, &snap.meta.root).run(rules, &snap, &env);
    println!(
        "rule matches: {} in {:.2} s",
        matches.len(),
        started.elapsed().as_secs_f64()
    );

    let du = Command::new("/usr/bin/du").arg("-skx").arg(&path).output();
    let du_kib = du.ok().and_then(|o| {
        String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<f64>().ok())
    });
    let Some(du_kib) = du_kib else {
        eprintln!("could not run du");
        return ExitCode::from(2);
    };
    let ours = root.size as f64 / 1024.0;
    let diff = if du_kib > 0.0 {
        (ours - du_kib) / du_kib * 100.0
    } else {
        0.0
    };
    println!("du -skx:      {du_kib:.0} KiB");
    println!("difference:   {diff:+.3}% (limit ±{TOLERANCE_PCT}%)");
    if diff.abs() <= TOLERANCE_PCT {
        println!("PASS");
        ExitCode::SUCCESS
    } else {
        println!("FAIL");
        ExitCode::FAILURE
    }
}
