//! Pro license tokens (docs/PLAN.md section 3, Phase 15), verified offline.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;

/// Not the update key: a leaked license key must not be able to sign updates.
const PUBLIC_KEY: &str = include_str!("../assets/license.pub");

const TOKEN_PREFIX: &str = "DL1.";
const DEEP_LINK_PREFIX: &str = "disklens://activate?token=";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LicenseError {
    #[error("this is not a DiskLens license")]
    Malformed,
    #[error("the license signature does not match")]
    BadSignature,
    #[error("this license needs a newer DiskLens")]
    UnsupportedVersion,
    #[error("this license is for an earlier major version of DiskLens")]
    EarlierMajorVersion,
    #[error("this trial license has no end date")]
    TrialWithoutExpiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Pro,
    Trial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Free,
    Pro {
        id: String,
    },
    /// `ends` is unix seconds.
    Trial {
        id: String,
        ends: u64,
    },
    /// Validly signed, past its end date.
    Ended {
        kind: Kind,
        id: String,
    },
    Invalid(LicenseError),
}

impl Status {
    pub fn is_pro(&self) -> bool {
        matches!(self, Self::Pro { .. } | Self::Trial { .. })
    }
}

#[derive(Deserialize)]
struct Payload {
    v: u32,
    id: String,
    kind: Kind,
    max_major: u32,
    expires: Option<u64>,
}

/// `token` is what the user pasted or the stored one; `None` is a free install.
pub fn status(token: Option<&str>, key: &PublicKey, now: SystemTime, app_major: u32) -> Status {
    let Some(token) = token else {
        return Status::Free;
    };
    check(token.trim(), key, now, app_major).unwrap_or_else(Status::Invalid)
}

/// `status` with the embedded key, the real clock and this build's major version.
pub fn current_status(token: Option<&str>) -> Status {
    // The Cargo major version is a number by construction.
    let app_major = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(u32::MAX);
    match PublicKey::decode(PUBLIC_KEY) {
        Ok(key) => status(token, &key, SystemTime::now(), app_major),
        // Unreachable while `embedded_key_parses` passes; fails closed, never Pro.
        Err(_) => Status::Invalid(LicenseError::BadSignature),
    }
}

/// The token from `disklens://activate?token=DL1.…`; anything else is `None`.
pub fn parse_deep_link(url: &str) -> Option<String> {
    let token = url.strip_prefix(DEEP_LINK_PREFIX)?;
    let is_token_text = token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    (token.starts_with(TOKEN_PREFIX) && is_token_text).then(|| token.to_string())
}

fn check(
    token: &str,
    key: &PublicKey,
    now: SystemTime,
    app_major: u32,
) -> Result<Status, LicenseError> {
    let rest = token
        .strip_prefix(TOKEN_PREFIX)
        .ok_or(LicenseError::Malformed)?;
    let (payload_b64, sig_b64) = rest.split_once('.').ok_or(LicenseError::Malformed)?;
    let payload = decode(payload_b64)?;
    let sig_text = String::from_utf8(decode(sig_b64)?).map_err(|_| LicenseError::Malformed)?;
    let signature = Signature::decode(&sig_text).map_err(|_| LicenseError::Malformed)?;
    // Nothing in the payload is trusted before this passes.
    key.verify(&payload, &signature, false)
        .map_err(|_| LicenseError::BadSignature)?;

    let p: Payload = serde_json::from_slice(&payload).map_err(|_| LicenseError::Malformed)?;
    if p.v != 1 {
        return Err(LicenseError::UnsupportedVersion);
    }
    if app_major > p.max_major {
        return Err(LicenseError::EarlierMajorVersion);
    }
    if p.kind == Kind::Trial && p.expires.is_none() {
        return Err(LicenseError::TrialWithoutExpiry);
    }
    // A clock before 1970 counts as 0, so nothing has ended.
    let now = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    Ok(match (p.kind, p.expires) {
        (kind, Some(ends)) if now >= ends => Status::Ended { kind, id: p.id },
        (Kind::Trial, Some(ends)) => Status::Trial { id: p.id, ends },
        _ => Status::Pro { id: p.id },
    })
}

fn decode(b64: &str) -> Result<Vec<u8>, LicenseError> {
    URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|_| LicenseError::Malformed)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use minisign::KeyPair;

    use super::*;

    const NOW: u64 = 1_800_000_000;
    const APP_MAJOR: u32 = 1;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    struct Signer {
        pair: KeyPair,
    }

    impl Signer {
        fn new() -> Self {
            Self {
                pair: KeyPair::generate_unencrypted_keypair().unwrap(),
            }
        }

        fn key(&self) -> PublicKey {
            PublicKey::decode(&self.pair.pk.to_box().unwrap().into_string()).unwrap()
        }

        fn sign(&self, payload: &str) -> String {
            let sig = minisign::sign(
                Some(&self.pair.pk),
                &self.pair.sk,
                payload.as_bytes(),
                Some("lic_test"),
                None,
            )
            .unwrap()
            .into_string();
            token(payload, &sig)
        }
    }

    fn token(payload: &str, sig_text: &str) -> String {
        format!(
            "DL1.{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(sig_text)
        )
    }

    fn payload(v: u32, kind: &str, max_major: u32, expires: &str) -> String {
        format!(
            r#"{{"v":{v},"id":"lic_test","kind":"{kind}","issued":1,"max_major":{max_major},"expires":{expires}}}"#
        )
    }

    fn status_of(signer: &Signer, token: &str) -> Status {
        status(Some(token), &signer.key(), at(NOW), APP_MAJOR)
    }

    fn invalid(reason: LicenseError) -> Status {
        Status::Invalid(reason)
    }

    #[test]
    fn valid_pro_token_is_pro() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 1, "null"));
        assert_eq!(
            status_of(&s, &t),
            Status::Pro {
                id: "lic_test".into()
            }
        );
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        let s = Signer::new();
        let t = format!("  {}\n", s.sign(&payload(1, "pro", 1, "null")));
        assert!(status_of(&s, &t).is_pro());
    }

    #[test]
    fn trial_before_its_end_is_a_trial() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "trial", 1, &(NOW + 10).to_string()));
        assert_eq!(
            status_of(&s, &t),
            Status::Trial {
                id: "lic_test".into(),
                ends: NOW + 10
            }
        );
    }

    #[test]
    fn trial_after_its_end_has_ended() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "trial", 1, &(NOW - 1).to_string()));
        assert_eq!(
            status_of(&s, &t),
            Status::Ended {
                kind: Kind::Trial,
                id: "lic_test".into()
            }
        );
    }

    #[test]
    fn trial_ends_at_the_expiry_second() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "trial", 1, &NOW.to_string()));
        assert!(matches!(status_of(&s, &t), Status::Ended { .. }));
    }

    #[test]
    fn expired_pro_token_has_ended() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 1, &(NOW - 1).to_string()));
        assert!(matches!(
            status_of(&s, &t),
            Status::Ended {
                kind: Kind::Pro,
                ..
            }
        ));
    }

    #[test]
    fn ended_license_is_not_pro() {
        let ended = Status::Ended {
            kind: Kind::Trial,
            id: "lic_test".into(),
        };
        assert!(!ended.is_pro());
    }

    #[test]
    fn tampered_payload_is_invalid() {
        let s = Signer::new();
        let sig = {
            let t = s.sign(&payload(1, "pro", 1, "null"));
            let sig_b64 = t.rsplit('.').next().unwrap().to_string();
            String::from_utf8(URL_SAFE_NO_PAD.decode(sig_b64).unwrap()).unwrap()
        };
        let forged = token(&payload(1, "pro", 9, "null"), &sig);
        assert_eq!(status_of(&s, &forged), invalid(LicenseError::BadSignature));
    }

    #[test]
    fn tampered_signature_is_invalid() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 1, "null"));
        let (head, sig_b64) = t.rsplit_once('.').unwrap();
        let sig = String::from_utf8(URL_SAFE_NO_PAD.decode(sig_b64).unwrap()).unwrap();
        // The trusted comment is covered by the global signature.
        let forged = token_from(head, &sig.replace("lic_test", "lic_evil"));
        assert_eq!(status_of(&s, &forged), invalid(LicenseError::BadSignature));
    }

    fn token_from(head: &str, sig_text: &str) -> String {
        format!("{head}.{}", URL_SAFE_NO_PAD.encode(sig_text))
    }

    #[test]
    fn token_signed_by_another_key_is_invalid() {
        let ours = Signer::new();
        let theirs = Signer::new();
        let t = theirs.sign(&payload(1, "pro", 1, "null"));
        assert_eq!(status_of(&ours, &t), invalid(LicenseError::BadSignature));
    }

    #[test]
    fn legacy_signature_is_invalid() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 1, "null"));
        let (head, sig_b64) = t.rsplit_once('.').unwrap();
        let sig = String::from_utf8(URL_SAFE_NO_PAD.decode(sig_b64).unwrap()).unwrap();
        // Same bytes with the legacy algorithm marker "Ed" instead of "ED".
        let mut lines: Vec<String> = sig.lines().map(String::from).collect();
        let mut raw = base64_std_decode(&lines[1]);
        raw[1] = b'd';
        lines[1] = base64_std_encode(&raw);
        let forged = token_from(head, &lines.join("\n"));
        assert_eq!(status_of(&s, &forged), invalid(LicenseError::BadSignature));
    }

    fn base64_std_decode(s: &str) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD.decode(s).unwrap()
    }

    fn base64_std_encode(b: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(b)
    }

    #[test]
    fn license_for_an_earlier_major_version_is_invalid() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 0, "null"));
        assert_eq!(
            status_of(&s, &t),
            invalid(LicenseError::EarlierMajorVersion)
        );
    }

    #[test]
    fn license_for_a_later_major_version_is_pro() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 2, "null"));
        assert!(status_of(&s, &t).is_pro());
    }

    #[test]
    fn unknown_token_version_is_invalid() {
        let s = Signer::new();
        let t = s.sign(&payload(2, "pro", 1, "null"));
        assert_eq!(status_of(&s, &t), invalid(LicenseError::UnsupportedVersion));
    }

    #[test]
    fn trial_without_expiry_is_invalid() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "trial", 1, "null"));
        assert_eq!(status_of(&s, &t), invalid(LicenseError::TrialWithoutExpiry));
    }

    #[test]
    fn signed_payload_that_is_not_a_license_is_invalid() {
        let s = Signer::new();
        let t = s.sign(r#"{"hello":"world"}"#);
        assert_eq!(status_of(&s, &t), invalid(LicenseError::Malformed));
    }

    #[test]
    fn garbage_is_invalid() {
        let s = Signer::new();
        for text in ["", "hello", "DL1.", "DL1.a.b", "DL1.!!.??", "XX1.a.b"] {
            assert_eq!(
                status_of(&s, text),
                invalid(LicenseError::Malformed),
                "{text}"
            );
        }
    }

    #[test]
    fn no_token_is_free() {
        let s = Signer::new();
        assert_eq!(status(None, &s.key(), at(NOW), APP_MAJOR), Status::Free);
    }

    #[test]
    fn embedded_key_parses() {
        assert!(PublicKey::decode(PUBLIC_KEY).is_ok());
    }

    #[test]
    fn embedded_key_rejects_a_token_signed_elsewhere() {
        let s = Signer::new();
        let t = s.sign(&payload(1, "pro", 1, "null"));
        assert!(!current_status(Some(&t)).is_pro());
    }

    #[test]
    fn deep_link_gives_the_token() {
        assert_eq!(
            parse_deep_link("disklens://activate?token=DL1.abc-_.def"),
            Some("DL1.abc-_.def".into())
        );
    }

    #[test]
    fn deep_link_with_another_scheme_is_refused() {
        assert_eq!(parse_deep_link("https://activate?token=DL1.abc.def"), None);
    }

    #[test]
    fn deep_link_with_another_host_is_refused() {
        assert_eq!(parse_deep_link("disklens://open?token=DL1.abc.def"), None);
    }

    #[test]
    fn deep_link_without_a_token_is_refused() {
        assert_eq!(parse_deep_link("disklens://activate"), None);
    }

    #[test]
    fn deep_link_with_a_foreign_token_is_refused() {
        assert_eq!(parse_deep_link("disklens://activate?token=abc"), None);
    }

    #[test]
    fn deep_link_with_extra_parameters_is_refused() {
        assert_eq!(
            parse_deep_link("disklens://activate?token=DL1.abc.def&x=1"),
            None
        );
    }
}
