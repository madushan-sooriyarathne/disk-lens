import { createHash, createPrivateKey, randomBytes, sign, type KeyObject } from "node:crypto";

export const TRIAL_SECONDS = 7 * 86400;

const PKCS8_ED25519_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const ID_ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";

export type SigningKey = { keyId: Buffer; privateKey: KeyObject };

export type Claims = {
  id: string;
  kind: "pro" | "trial";
  issued: number;
  maxMajor: number;
  expires: number | null;
};

const blake2b = (algorithm: "blake2b256" | "blake2b512", ...parts: Buffer[]) =>
  createHash(algorithm).update(Buffer.concat(parts)).digest();

/** Accepts the whole minisign secret key file or only its base64 line. */
export function parseSecretKey(text: string): SigningKey {
  // A multi-line value in an .env file arrives with literal \n sequences.
  const lines = text.replace(/\\n/g, "\n").trim().split(/\r?\n/);
  const raw = Buffer.from((lines[1] ?? lines[0] ?? "").trim(), "base64");
  if (raw.length !== 158 || raw.subarray(0, 2).toString() !== "Ed") {
    throw new Error("license secret key is not a minisign secret key");
  }
  if (raw.readUInt16LE(2) !== 0) {
    throw new Error("license secret key is encrypted; generate it with minisign -W");
  }
  if (raw.subarray(4, 6).toString() !== "B2") {
    throw new Error("license secret key uses an unsupported checksum algorithm");
  }
  const keyId = raw.subarray(54, 62);
  const sk = raw.subarray(62, 126);
  const expected = blake2b("blake2b256", raw.subarray(0, 2), keyId, sk);
  if (!expected.equals(raw.subarray(126, 158))) {
    throw new Error("license secret key checksum does not match");
  }
  const privateKey = createPrivateKey({
    key: Buffer.concat([PKCS8_ED25519_PREFIX, sk.subarray(0, 32)]),
    format: "der",
    type: "pkcs8",
  });
  return { keyId: Buffer.from(keyId), privateKey };
}

/** Hex in the order the minisign CLI prints it, which reverses the stored bytes. */
export function keyId(key: SigningKey): string {
  return Buffer.from(key.keyId).reverse().toString("hex").toUpperCase();
}

export function newId(prefix: "lic_" | "ord_"): string {
  const chars = Array.from(randomBytes(26), (byte) => ID_ALPHABET[byte & 31]);
  return prefix + chars.join("");
}

/** Same bytes as scripts/mint-license.sh, which the app verifies with minisign-verify. */
export function mint(key: SigningKey, claims: Claims): string {
  const payload = Buffer.from(
    JSON.stringify({
      v: 1,
      id: claims.id,
      kind: claims.kind,
      issued: claims.issued,
      max_major: claims.maxMajor,
      expires: claims.expires,
    }),
  );
  const signature = sign(null, blake2b("blake2b512", payload), key.privateKey);
  const globalSignature = sign(
    null,
    Buffer.concat([signature, Buffer.from(claims.id)]),
    key.privateKey,
  );
  const minisig =
    "untrusted comment: signature from minisign secret key\n" +
    `${Buffer.concat([Buffer.from("ED"), key.keyId, signature]).toString("base64")}\n` +
    `trusted comment: ${claims.id}\n` +
    `${globalSignature.toString("base64")}\n`;
  return `DL1.${payload.toString("base64url")}.${Buffer.from(minisig).toString("base64url")}`;
}
