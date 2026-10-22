//! Docker Desktop's reclaimable space from `docker system df --format json`:
//! images, containers and build cache. Volumes are kept by the prune command and
//! are not counted. When Docker does not answer (usually because it is not
//! running), the matched VM folder's size stands in as the upper bound and the
//! `requires_running` guard explains how to unblock it.

use serde::Deserialize;

use super::{Found, ProviderItem};
use crate::rules::guards::Env;

const NAME: &str = "Unused images, containers & build cache";

#[derive(Deserialize)]
struct Row {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "Reclaimable")]
    reclaimable: String,
}

pub fn items(at: &Found, env: &Env) -> Vec<ProviderItem> {
    let Some(docker) = env.tools.resolve("docker") else {
        return Vec::new();
    };
    let size = env
        .runner
        .run(&docker, &["system", "df", "--format", "json"])
        .ok()
        .filter(|o| o.success)
        .and_then(|o| reclaimable(&o.stdout))
        .unwrap_or(at.snap.nodes[at.node as usize].size);
    vec![ProviderItem {
        key: "unused".into(),
        name: NAME.into(),
        path: at.path.to_path_buf(),
        size,
        node: Some(at.node),
        main_repo: None,
        project: None,
    }]
}

/// Sum of the reclaimable sizes, volumes excluded. `None` if nothing parses.
fn reclaimable(stdout: &str) -> Option<u64> {
    let rows: Vec<Row> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if rows.is_empty() {
        return None;
    }
    rows.iter()
        .filter(|r| r.kind != "Local Volumes")
        .map(|r| parse_size(&r.reclaimable))
        .sum()
}

/// Docker's decimal sizes: `0B`, `512kB`, `1.5GB (40%)`.
fn parse_size(s: &str) -> Option<u64> {
    let s = s.split_whitespace().next()?;
    let split = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = s.split_at(split);
    let num: f64 = num.parse().ok()?;
    let mult = match unit.to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    Some((num * mult).round() as u64)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::SystemTime;

    use super::*;
    use crate::platform::{Facts, FakeRunner};
    use crate::providers::test_support::{node, snap, tree};
    use crate::rules::tools::Tools;

    const DF: &str = r#"{"Active":"2","Reclaimable":"1.5GB (60%)","Size":"2.5GB","TotalCount":"5","Type":"Images"}
{"Active":"0","Reclaimable":"512kB (100%)","Size":"512kB","TotalCount":"1","Type":"Containers"}
{"Active":"1","Reclaimable":"9GB (90%)","Size":"10GB","TotalCount":"3","Type":"Local Volumes"}
{"Active":"0","Reclaimable":"300MB","Size":"300MB","TotalCount":"12","Type":"Build Cache"}
"#;

    #[test]
    fn sums_reclaimable_without_volumes() {
        assert_eq!(reclaimable(DF), Some(1_500_000_000 + 512_000 + 300_000_000));
        assert_eq!(reclaimable(""), None);
        assert_eq!(reclaimable("garbage"), None);
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("1.5GB (60%)"), Some(1_500_000_000));
        assert_eq!(parse_size("2.1TB"), Some(2_100_000_000_000));
        assert_eq!(parse_size("12kB"), Some(12_000));
        assert_eq!(parse_size("12XB"), None);
        assert_eq!(parse_size(""), None);
    }

    #[test]
    fn falls_back_to_folder_size_when_docker_does_not_answer() {
        let (_t, root) = tree(&[("vms/0/Docker.raw", &"x".repeat(8192))]);
        let s = snap(&root);
        let vms = root.join("vms");
        let at = Found {
            snap: &s,
            node: node(&s, &vms),
            path: &vms,
        };
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("docker"), "").unwrap();
        std::fs::set_permissions(bin.join("docker"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let tools = Tools::new(vec![bin], false);
        let facts = Facts::default();
        let run = |runner: FakeRunner| {
            let env = Env {
                facts: &facts,
                runner: &runner,
                tools: &tools,
                now: SystemTime::now(),
            };
            items(&at, &env)
        };
        let up = run(FakeRunner::default().with("docker system df --format json", true, DF));
        assert_eq!(up.len(), 1);
        assert_eq!(up[0].size, 1_800_512_000);
        let down = run(FakeRunner::default());
        assert_eq!(down[0].size, s.nodes[at.node as usize].size);
        assert!(down[0].size >= 8192);
    }
}
