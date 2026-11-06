import { Database } from "bun:sqlite";
import { createHmac } from "node:crypto";
import type { ReactElement } from "react";

import { PaymentsLk } from "@payments-lk/node";

import type { EmailToken } from "../emails/LicenseEmail";
import { createApp, type Config } from "../src/app";
import { openDb } from "../src/db";
import type { Mailer } from "../src/mail";
import { testKey } from "./fixtures/make-contract";

export const WEBHOOK_SECRET = "whsec_test";
export const SITE = "https://site.test";
export const START = 1_800_000_000_000;

type Sent = { to: string; subject: string; react: ReactElement<{ tokens: EmailToken[] }> };

export function makeHarness(over: Partial<Config> = {}) {
  const mails: Sent[] = [];
  const apiCalls: { method: string; path: string; headers: Record<string, string>; body: any }[] = [];
  const state = { now: START, paymentStatus: "requires_payment_method", failMail: false, apiDown: false };

  const mailer: Mailer = async (mail) => {
    if (state.failMail) throw new Error("ses down");
    mails.push(mail as Sent);
  };

  const fakeFetch = async (input: URL | string, init: RequestInit) => {
    const path = new URL(input).pathname;
    const body = init.body ? JSON.parse(init.body as string) : undefined;
    apiCalls.push({ method: init.method!, path, headers: init.headers as Record<string, string>, body });
    if (state.apiDown) return new Response("{}", { status: 400 });
    const ref = body?.reference ?? lastRef();
    return Response.json({
      object: "checkout",
      id: "chk_1",
      mode: "test",
      status: "open",
      url: "https://pay.test/chk_1",
      expiresAt: "2026-11-01T00:00:00Z",
      payment: payment(ref, { status: state.paymentStatus }),
    });
  };

  const db = openDb(":memory:");
  const lk = new PaymentsLk("sk_test_fake", {
    fetch: fakeFetch as unknown as typeof fetch,
    maxRetries: 0,
  });
  const config: Config = {
    siteUrl: SITE,
    webhookSecret: WEBHOOK_SECRET,
    priceCents: 500_000,
    major: 1,
    mode: "test",
    trustProxy: false,
    ...over,
  };
  const app = createApp({ db, lk, mailer, key: testKey(), now: () => state.now, config });

  const lastRef = () => (db.query("SELECT ref FROM orders ORDER BY rowid DESC").get() as { ref: string }).ref;

  return { app, db, mails, apiCalls, state, lastRef };
}

export type Harness = ReturnType<typeof makeHarness>;

export function payment(ref: string, over: Record<string, unknown> = {}) {
  return {
    object: "payment",
    id: "pay_1",
    mode: "test",
    status: "succeeded",
    amountCents: 500_000,
    currency: "LKR",
    reference: ref,
    customer: { name: null, email: "buyer@example.com", phone: null },
    checkoutId: "chk_1",
    ...over,
  };
}

export function event(type: string, data: unknown, mode = "test") {
  return { id: "evt_1", object: "event", type, mode, created: "2026-11-01T00:00:00Z", data };
}

export const request = (path: string, init: RequestInit = {}) =>
  new Request(`http://api.test${path}`, init);

export const postJson = (h: Harness, path: string, body: unknown, ip = "1.1.1.1") =>
  h.app.fetch(request(path, { method: "POST", body: JSON.stringify(body) }), ip);

export function webhook(h: Harness, evt: unknown, opts: { secret?: string; t?: number } = {}) {
  const body = JSON.stringify(evt);
  const t = opts.t ?? Math.floor(h.state.now / 1000);
  const v1 = createHmac("sha256", opts.secret ?? WEBHOOK_SECRET).update(`${t}.${body}`).digest("hex");
  return h.app.fetch(
    request("/webhooks/payments-lk", {
      method: "POST",
      body,
      headers: { "payments-signature": `t=${t},v1=${v1}` },
    }),
  );
}

export async function startCheckout(h: Harness, email = "buyer@example.com") {
  await postJson(h, "/checkout", { email });
  return h.lastRef();
}

export async function pay(h: Harness, email = "buyer@example.com") {
  const ref = await startCheckout(h, email);
  await webhook(h, event("payment.succeeded", payment(ref)));
  return ref;
}

export const orderOf = (h: Harness, ref: string) =>
  h.db.query("SELECT * FROM orders WHERE ref = ?").get(ref) as any;

export const claimsOf = (token: string) =>
  JSON.parse(Buffer.from(token.split(".")[1]!, "base64url").toString());

export async function times(n: number, run: (i: number) => Promise<unknown>) {
  for (let i = 0; i < n; i++) await run(i);
}
