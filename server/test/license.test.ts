import { describe, expect, test } from "bun:test";

import { keyId, mint, newId, parseSecretKey } from "../src/license";
import contract from "./fixtures/contract.json";
import { FIXED_CLAIMS, secretKeyText, testKey } from "./fixtures/make-contract";
import { claimsOf, makeHarness, postJson } from "./harness";

describe("license signer", () => {
  test("mints the contract token for the fixed claims", () => {
    expect(mint(testKey(), FIXED_CLAIMS)).toBe(contract.token);
  });

  test("refuses an encrypted key", () => {
    expect(() => parseSecretKey(secretKeyText({ encrypted: true }))).toThrow("encrypted");
  });

  test("refuses a key with a bad checksum", () => {
    expect(() => parseSecretKey(secretKeyText({ badChecksum: true }))).toThrow("checksum");
  });

  test("accepts only the base64 line of the key file", () => {
    const line = secretKeyText().split("\n")[1]!;
    expect(keyId(parseSecretKey(line))).toBe(keyId(testKey()));
  });

  test("accepts a key file with literal backslash-n separators from an .env file", () => {
    const escaped = secretKeyText().trim().replace("\n", "\\n");
    expect(keyId(parseSecretKey(escaped))).toBe(keyId(testKey()));
  });

  test("shows the key id the way the minisign CLI does", () => {
    expect(contract.publicKey.split("\n")[0]).toEndWith(keyId(testKey()));
  });

  test("makes ids from the lowercase base32 alphabet", () => {
    expect(newId("lic_")).toMatch(/^lic_[a-z2-7]{26}$/);
  });

  test("ends a trial token seven days after it was issued", async () => {
    const h = makeHarness();
    await postJson(h, "/trial", { email: "a@example.com" });
    const { issued, expires } = claimsOf(h.mails[0]!.react.props.tokens[0]!.token);
    expect(expires - issued).toBe(604_800);
  });
});
