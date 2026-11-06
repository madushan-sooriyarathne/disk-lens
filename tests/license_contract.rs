//! The license server (`server/`) and the app agree on the token format: a token the
//! server's signer minted with a test-only key verifies here as Pro.
//! `server/test/fixtures/make-contract.ts` regenerates the fixture.

use std::time::SystemTime;

use disklens_lib::license::{Status, status};
use minisign_verify::PublicKey;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contract {
    public_key: String,
    token: String,
    id: String,
}

#[test]
fn server_minted_token_verifies_as_pro() {
    let c: Contract =
        serde_json::from_str(include_str!("../server/test/fixtures/contract.json")).unwrap();
    let key = PublicKey::decode(&c.public_key).unwrap();

    let got = status(Some(&c.token), &key, SystemTime::now(), 1);

    assert_eq!(got, Status::Pro { id: c.id });
}
