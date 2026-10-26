//! In-app update: checks the release feed and installs a signed build over the running app.

use cargo_packager_updater::semver::Version;
use cargo_packager_updater::{Config, check_update};

pub(crate) use cargo_packager_updater::Update;

use crate::error::{Error, Result};

const DEFAULT_ENDPOINT: &str =
    "https://github.com/madushan-sooriyarathne/disk-lens/releases/latest/download/latest.json";

/// Verifies every download, so the key is part of the binary.
const PUBLIC_KEY: &str = include_str!("../assets/update.pub");

/// Whether to look for updates at startup. `DISKLENS_UPDATE_URL` (dev and QA) forces it on.
pub(crate) fn is_enabled(is_mock: bool, is_sandboxed: bool) -> bool {
    decide(
        cfg!(debug_assertions),
        is_mock,
        is_sandboxed,
        endpoint_override().is_some(),
    )
}

/// `Ok(None)` when the installed version is current.
pub(crate) fn check() -> Result<Option<Update>> {
    check_at(&endpoint_override().unwrap_or_else(|| DEFAULT_ENDPOINT.into()))
}

/// Replaces the running app bundle; takes effect after a restart.
pub(crate) fn install(update: &Update) -> Result<()> {
    update
        .download_and_install()
        .map_err(|e| Error::Update(format!("installing update: {e}")))
}

fn decide(is_debug: bool, is_mock: bool, is_sandboxed: bool, has_override: bool) -> bool {
    has_override || !(is_debug || is_mock || is_sandboxed)
}

fn endpoint_override() -> Option<String> {
    std::env::var("DISKLENS_UPDATE_URL")
        .ok()
        .filter(|u| !u.is_empty())
}

fn check_at(endpoint: &str) -> Result<Option<Update>> {
    let fail = |e: &dyn std::fmt::Display| Error::Update(format!("checking for updates: {e}"));
    let config = Config {
        endpoints: vec![endpoint.parse().map_err(|e| fail(&e))?],
        pubkey: PUBLIC_KEY.trim().into(),
        windows: None,
    };
    let version = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| fail(&e))?;
    check_update(version, config).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;

    /// Answers one HTTP request with `response`; returns the URL to ask.
    fn serve_once(response: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest.json", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Replying before the headers are read would reset the connection.
            let request = BufReader::new(stream.try_clone().unwrap());
            for line in request.lines() {
                if line.unwrap().is_empty() {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
        });
        url
    }

    fn release_json(version: &str) -> String {
        let platform = r#"{"url":"http://127.0.0.1/app.tar.gz","signature":"c2ln","format":"app"}"#;
        let body = format!(
            r#"{{"version":"{version}","notes":"n","pub_date":"2026-01-01T00:00:00Z","platforms":{{"macos-aarch64":{platform},"macos-x86_64":{platform}}}}}"#
        );
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn debug_builds_do_not_check() {
        assert!(!decide(true, false, false, false));
    }

    #[test]
    fn mock_mode_does_not_check() {
        assert!(!decide(false, true, false, false));
    }

    #[test]
    fn sandboxed_runs_do_not_check() {
        assert!(!decide(false, false, true, false));
    }

    #[test]
    fn live_release_builds_check() {
        assert!(decide(false, false, false, false));
    }

    #[test]
    fn the_url_override_turns_checking_on_everywhere() {
        assert!(decide(true, true, true, true));
    }

    #[test]
    fn a_newer_release_is_found() {
        let url = serve_once(release_json("9.9.9"));
        let found = check_at(&url).unwrap().map(|u| u.version);
        assert_eq!(found.as_deref(), Some("9.9.9"));
    }

    #[test]
    fn no_content_means_up_to_date() {
        let url = serve_once("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".into());
        assert!(check_at(&url).unwrap().is_none());
    }

    #[test]
    fn an_unreachable_feed_is_an_update_error() {
        let url = serve_once(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        );
        let message = check_at(&url).unwrap_err().to_string();
        assert!(message.starts_with("checking for updates: "));
    }
}
