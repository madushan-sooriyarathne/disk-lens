// Regenerate with `bun test/fixtures/make-contract.ts`. tests/license_contract.rs verifies the token in Rust.
import { createHash, createPrivateKey, createPublicKey } from "node:crypto";

import { mint, parseSecretKey, type Claims } from "../../src/license";

// Public on purpose: this key signs only the fixture and is not the real license key.
export const TEST_ONLY_SEED = Buffer.from(
  "7a1c0e5d93b24f68a0d1c3e5f7092b4d6e8a1c3f5b7d9e0a2c4e6f8091a3b5c7",
  "hex",
);
const TEST_KEY_ID = Buffer.from("a1b2c3d4e5f60718", "hex");

export const FIXED_CLAIMS: Claims = {
  id: "lic_contracttestaaaaaaaaaaaaa",
  kind: "pro",
  issued: 1767225600,
  maxMajor: 1,
  expires: null,
};

const publicKeyBytes = () =>
  createPublicKey(
    createPrivateKey({
      key: Buffer.concat([Buffer.from("302e020100300506032b657004220420", "hex"), TEST_ONLY_SEED]),
      format: "der",
      type: "pkcs8",
    }),
  )
    .export({ format: "der", type: "spki" })
    .subarray(-32);

/** A minisign secret key file built in memory; the options break it on purpose for tests. */
export function secretKeyText(opts: { encrypted?: boolean; badChecksum?: boolean } = {}): string {
  const sigAlg = Buffer.from("Ed");
  const sk = Buffer.concat([TEST_ONLY_SEED, publicKeyBytes()]);
  const checksum = createHash("blake2b256")
    .update(Buffer.concat([sigAlg, TEST_KEY_ID, sk]))
    .digest();
  if (opts.badChecksum) checksum[0] = (checksum[0] ?? 0) ^ 0xff;
  const raw = Buffer.concat([
    sigAlg,
    Buffer.from(opts.encrypted ? "Sc" : "\0\0", "latin1"),
    Buffer.from("B2"),
    Buffer.alloc(32 + 8 + 8),
    TEST_KEY_ID,
    sk,
    checksum,
  ]);
  return `untrusted comment: minisign encrypted secret key\n${raw.toString("base64")}\n`;
}

export const publicKeyText = () =>
  `untrusted comment: minisign public key ${Buffer.from(TEST_KEY_ID).reverse().toString("hex").toUpperCase()}\n` +
  `${Buffer.concat([Buffer.from("Ed"), TEST_KEY_ID, publicKeyBytes()]).toString("base64")}\n`;

export const testKey = () => parseSecretKey(secretKeyText());

if (import.meta.main) {
  const contract = {
    publicKey: publicKeyText(),
    token: mint(testKey(), FIXED_CLAIMS),
    id: FIXED_CLAIMS.id,
  };
  await Bun.write(
    new URL("contract.json", import.meta.url),
    `${JSON.stringify(contract, null, 2)}\n`,
  );
}
