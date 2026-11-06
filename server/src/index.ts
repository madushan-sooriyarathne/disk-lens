import { PaymentsLk } from "@payments-lk/node";

import { createApp } from "./app";
import { openDb } from "./db";
import { keyId, parseSecretKey } from "./license";
import { createMailer } from "./mail";

// Names the variable, never its value: the values are secrets.
function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set (see server/.env.example)`);
  return value;
}

function integer(name: string, fallback: string | undefined, min: number, max: number): number {
  const value = Number(process.env[name] ?? fallback ?? required(name));
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${name} must be a whole number from ${min} to ${max}`);
  }
  return value;
}

const secret = required("PAYMENTS_LK_SECRET_KEY");
const mode = secret.startsWith("sk_test_") ? "test" : secret.startsWith("sk_live_") ? "live" : null;
if (!mode) throw new Error("PAYMENTS_LK_SECRET_KEY must start with sk_test_ or sk_live_");

const dryRun = process.env.MAIL_DRY_RUN === "1";
if (mode === "live" && dryRun) throw new Error("MAIL_DRY_RUN=1 is not allowed with a live key");

const key = parseSecretKey(required("LICENSE_SECRET_KEY"));
const port = integer("PORT", "8787", 1, 65535);
const siteUrl = (process.env.SITE_URL ?? "http://localhost:3000").replace(/\/+$/, "");

const app = createApp({
  db: openDb(process.env.DATABASE_PATH ?? "./disklens.db"),
  lk: new PaymentsLk(secret),
  mailer: createMailer({
    from: required("MAIL_FROM"),
    dryRun,
    dir: new URL("../.mail/", import.meta.url),
  }),
  key,
  now: () => Date.now(),
  config: {
    siteUrl,
    webhookSecret: required("PAYMENTS_LK_WEBHOOK_SECRET"),
    priceCents: integer("PRO_PRICE_CENTS", undefined, 1000, 100_000_000),
    major: integer("LICENSE_MAJOR", "1", 1, 1000),
    mode,
    trustProxy: process.env.TRUST_PROXY === "1",
  },
});

Bun.serve({
  port,
  maxRequestBodySize: 16 * 1024,
  fetch: (req, server) => app.fetch(req, server.requestIP(req)?.address),
});

console.log(
  `license server on :${port}, mode ${mode}, key ${keyId(key)}, mail ${dryRun ? "dry run" : "SES"}`,
);
