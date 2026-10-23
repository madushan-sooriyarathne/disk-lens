//! Ollama models from `manifests/` under the matched models folder. Reading the
//! manifests works whether or not the server runs; the `requires_running` guard
//! blocks removal while it is down. Blobs shared by several models are counted
//! once, for the first model by name.

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Found, ProviderItem};

const DEFAULT_REGISTRY: &str = "registry.ollama.ai";
const DEFAULT_NAMESPACE: &str = "library";
/// registry / namespace / model / tag
const MANIFEST_DEPTH: usize = 4;

#[derive(Deserialize)]
struct Manifest {
    config: Option<Layer>,
    #[serde(default)]
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
struct Layer {
    digest: String,
}

pub fn items(at: &Found) -> Vec<ProviderItem> {
    let root = at.path.join("manifests");
    let mut manifests = Vec::new();
    collect(&root, 0, &mut manifests);
    let mut models: Vec<(String, Vec<String>)> = manifests
        .into_iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(&root).ok()?;
            let name = model_name(rel)?;
            let text = fs::read_to_string(&path).ok()?;
            let m: Manifest = serde_json::from_str(&text).ok()?;
            let digests = m
                .config
                .into_iter()
                .chain(m.layers)
                .map(|l| l.digest)
                .collect();
            Some((name, digests))
        })
        .collect();
    models.sort_by(|a, b| a.0.cmp(&b.0));

    let mut seen = HashSet::new();
    models
        .into_iter()
        .map(|(name, digests)| {
            let size = digests
                .iter()
                .filter(|d| seen.insert((*d).clone()))
                .filter_map(|d| blob_path(at.path, d))
                .filter_map(|p| fs::symlink_metadata(p).ok())
                .map(|m| m.blocks() * 512)
                .sum();
            ProviderItem {
                key: name.clone(),
                name,
                // Models are removed by name; the models folder is what Finder can show.
                path: at.path.to_path_buf(),
                size,
                node: None,
                main_repo: None,
                project: None,
            }
        })
        .collect()
}

/// Manifest files exactly `MANIFEST_DEPTH` levels below `dir`; symlinks are skipped.
fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() && depth + 1 < MANIFEST_DEPTH {
            collect(&e.path(), depth + 1, out);
        } else if ft.is_file() && depth + 1 == MANIFEST_DEPTH {
            out.push(e.path());
        }
    }
}

/// `registry.ollama.ai/library/llama3/8b` -> `llama3:8b`, other namespaces and
/// registries keep their prefix, as `ollama list` shows them.
fn model_name(rel: &Path) -> Option<String> {
    let parts: Vec<&str> = rel.iter().map(|p| p.to_str()).collect::<Option<_>>()?;
    let [registry, namespace, model, tag] = parts[..] else {
        return None;
    };
    Some(match (registry, namespace) {
        (DEFAULT_REGISTRY, DEFAULT_NAMESPACE) => format!("{model}:{tag}"),
        (DEFAULT_REGISTRY, ns) => format!("{ns}/{model}:{tag}"),
        (reg, ns) => format!("{reg}/{ns}/{model}:{tag}"),
    })
}

/// `sha256:<hex>` -> `<models>/blobs/sha256-<hex>`; anything else is ignored, so
/// a manifest cannot point outside the blobs folder.
fn blob_path(models: &Path, digest: &str) -> Option<PathBuf> {
    let hex = digest.strip_prefix("sha256:")?;
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(models.join("blobs").join(format!("sha256-{hex}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::test_support::{node, snap, tree};

    fn manifest(config: &str, layers: &[&str]) -> String {
        let layers: Vec<String> = layers
            .iter()
            .map(|d| format!(r#"{{"mediaType":"x","digest":"{d}","size":1}}"#))
            .collect();
        format!(
            r#"{{"schemaVersion":2,"config":{{"digest":"{config}","size":1}},"layers":[{}]}}"#,
            layers.join(",")
        )
    }

    #[test]
    fn lists_models_and_counts_shared_blobs_once() {
        let blob = "x".repeat(8192);
        let small = "x".repeat(4096);
        let (_t, root) = tree(&[
            (
                "models/manifests/registry.ollama.ai/library/llama3/8b",
                &manifest("sha256:c1", &["sha256:aa", "sha256:bb"]),
            ),
            (
                "models/manifests/registry.ollama.ai/library/llama3/latest",
                &manifest("sha256:c1", &["sha256:aa", "sha256:bb"]),
            ),
            (
                "models/manifests/registry.ollama.ai/me/tiny/v1",
                &manifest("sha256:c2", &["sha256:dd"]),
            ),
            (
                "models/manifests/hf.co/org/model/q4",
                &manifest("sha256:c3", &["../../etc/passwd", "sha256:zz"]),
            ),
            (
                "models/manifests/registry.ollama.ai/library/broken/x",
                "not json",
            ),
            ("models/blobs/sha256-aa", &blob),
            ("models/blobs/sha256-bb", &small),
            ("models/blobs/sha256-c1", &small),
            ("models/blobs/sha256-dd", &small),
        ]);
        let s = snap(&root);
        let models = root.join("models");
        let at = Found {
            snap: &s,
            node: node(&s, &models),
            path: &models,
        };
        let got: Vec<(String, u64)> = items(&at).into_iter().map(|i| (i.name, i.size)).collect();
        assert_eq!(
            got,
            vec![
                ("hf.co/org/model:q4".to_string(), 0),
                ("llama3:8b".to_string(), 8192 + 4096 + 4096),
                ("llama3:latest".to_string(), 0),
                ("me/tiny:v1".to_string(), 4096), // config blob c2 is missing,
            ]
        );
    }

    #[test]
    fn missing_manifests_folder_is_empty() {
        let (_t, root) = tree(&[("models/blobs/sha256-aa", "x")]);
        let s = snap(&root);
        let models = root.join("models");
        let at = Found {
            snap: &s,
            node: node(&s, &models),
            path: &models,
        };
        assert!(items(&at).is_empty());
    }

    #[test]
    fn digests_are_validated() {
        let m = Path::new("/m");
        assert_eq!(
            blob_path(m, "sha256:ab12"),
            Some(PathBuf::from("/m/blobs/sha256-ab12"))
        );
        assert_eq!(blob_path(m, "sha256:../x"), None);
        assert_eq!(blob_path(m, "sha256:"), None);
        assert_eq!(blob_path(m, "md5:ab"), None);
    }
}
