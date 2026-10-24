//! Simulator devices whose runtime is no longer installed, from
//! `xcrun simctl list devices unavailable -j`, sized from their folders in the
//! snapshot.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Found, ProviderItem, child};
use crate::rules::guards::Env;

#[derive(Deserialize)]
struct List {
    devices: BTreeMap<String, Vec<Device>>,
}

#[derive(Deserialize)]
struct Device {
    udid: String,
    name: String,
}

pub fn items(at: &Found, env: &Env) -> Vec<ProviderItem> {
    let Some(xcrun) = env.tools.resolve("xcrun") else {
        return Vec::new();
    };
    let out = match env
        .runner
        .run(&xcrun, &["simctl", "list", "devices", "unavailable", "-j"])
    {
        Ok(out) if out.success => out,
        Ok(_) => return Vec::new(),
        Err(e) => {
            tracing::warn!("simctl list failed: {e}");
            return Vec::new();
        }
    };
    let Ok(list) = serde_json::from_str::<List>(&out.stdout) else {
        return Vec::new();
    };
    list.devices
        .iter()
        .flat_map(|(runtime, devices)| devices.iter().map(move |d| (runtime, d)))
        .filter_map(|(runtime, d)| {
            // Only devices whose folder is in the scan; the lookup is by exact child name.
            let node = child(at.snap, at.node, &d.udid)?;
            Some(ProviderItem {
                key: d.udid.clone(),
                name: format!("{} ({})", d.name, runtime_name(runtime)),
                path: at.path.join(&d.udid),
                size: at.snap.nodes[node as usize].size,
                node: Some(node),
                main_repo: None,
                project: None,
            })
        })
        .collect()
}

/// `com.apple.CoreSimulator.SimRuntime.iOS-14-5` -> `iOS 14.5`.
fn runtime_name(id: &str) -> String {
    let last = id.rsplit('.').next().unwrap_or(id);
    match last.split_once('-') {
        Some((os, version)) => format!("{os} {}", version.replace('-', ".")),
        None => last.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::SystemTime;

    use super::*;
    use crate::platform::{Facts, FakeRunner};
    use crate::providers::test_support::{node, snap, tree};
    use crate::rules::model::Provider;
    use crate::rules::tools::Tools;

    const LIST: &str = r#"{"devices":{
      "com.apple.CoreSimulator.SimRuntime.iOS-14-5":[
        {"udid":"AAAA","name":"iPhone 12","isAvailable":false,"availabilityError":"runtime profile not found"},
        {"udid":"GONE","name":"iPad","isAvailable":false}
      ],
      "com.apple.CoreSimulator.SimRuntime.watchOS-7-4":[
        {"udid":"BBBB","name":"Watch","isAvailable":false}
      ]}}"#;

    fn xcrun_tools(root: &std::path::Path, sandbox: bool) -> Tools {
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("xcrun"), "").unwrap();
        std::fs::set_permissions(bin.join("xcrun"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        Tools::new(vec![bin], sandbox)
    }

    #[test]
    fn unavailable_devices_with_folders() {
        let (_t, root) = tree(&[
            ("Devices/AAAA/data/a", &"x".repeat(8192)),
            ("Devices/BBBB/data/a", &"x".repeat(4096)),
            ("Devices/CCCC/data/a", "x"),
        ]);
        let s = snap(&root);
        let devices = root.join("Devices");
        let at = Found {
            snap: &s,
            node: node(&s, &devices),
            path: &devices,
        };
        let facts = Facts::default();
        let runner =
            FakeRunner::default().with("xcrun simctl list devices unavailable -j", true, LIST);
        let tools = xcrun_tools(&root, false);
        let env = Env {
            facts: &facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
        };
        let got = items(&at, &env);
        let names: Vec<&str> = got.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["iPhone 12 (iOS 14.5)", "Watch (watchOS 7.4)"]);
        assert!(got[0].size >= 8192);
        assert_eq!(got[0].path, devices.join("AAAA"));

        // Sandbox mode: the dispatcher returns nothing without running xcrun.
        let sandbox_tools = xcrun_tools(&root, true);
        let env = Env {
            tools: &sandbox_tools,
            ..env
        };
        let got = crate::providers::items(Provider::SimctlUnavailable, &at, &env);
        assert!(got.is_empty());
        assert_eq!(
            runner.calls.lock().unwrap().len(),
            1,
            "only the first, non-sandbox call"
        );
    }

    #[test]
    fn failures_yield_nothing() {
        let (_t, root) = tree(&[("Devices/AAAA/a", "x")]);
        let s = snap(&root);
        let devices = root.join("Devices");
        let at = Found {
            snap: &s,
            node: node(&s, &devices),
            path: &devices,
        };
        let facts = Facts::default();
        let tools = xcrun_tools(&root, false);
        for runner in [
            FakeRunner::default(),
            FakeRunner::default().with("xcrun simctl list devices unavailable -j", false, LIST),
            FakeRunner::default().with("xcrun simctl list devices unavailable -j", true, "nope"),
        ] {
            let env = Env {
                facts: &facts,
                runner: &runner,
                tools: &tools,
                now: SystemTime::now(),
            };
            assert!(items(&at, &env).is_empty());
        }
    }

    #[test]
    fn runtime_names() {
        assert_eq!(
            runtime_name("com.apple.CoreSimulator.SimRuntime.iOS-17-0"),
            "iOS 17.0"
        );
        assert_eq!(
            runtime_name("com.apple.CoreSimulator.SimRuntime.xrOS-1-0"),
            "xrOS 1.0"
        );
        assert_eq!(runtime_name("weird"), "weird");
    }
}
