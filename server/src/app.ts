import type { Database } from "bun:sqlite";

import { WebhookSignatureError, type PaymentsLk } from "@payments-lk/node";

import { createLimiter } from "./limit";
import { keyId, type SigningKey } from "./license";
import { errorName, maskEmail, type Mailer } from "./mail";
import { createCheckout, handleEvent, orderState } from "./orders";
import { recover, startTrial } from "./trials";

export type Config = {
  siteUrl: string;
  webhookSecret: string;
  priceCents: number;
  major: number;
  mode: "test" | "live";
  trustProxy: boolean;
};

export type Deps = {
  db: Database;
  lk: PaymentsLk;
  mailer: Mailer;
  key: SigningKey;
  /** Milliseconds since the epoch. */
  now: () => number;
  config: Config;
};

const HOUR = 3_600_000;
const MAX_BODY = 16 * 1024;
const REF = /^ord_[a-z2-7]{26}$/;
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

const json = (status: number, body: unknown) => Response.json(body, { status });
const fail = (status: number, error: string, message: string) => json(status, { error, message });

export function createApp(d: Deps) {
  const allowed = new URL(d.config.siteUrl).origin;
  const withinLimit = createLimiter(d.now);

  return {
    /** `ip` is the socket address; it is ignored when TRUST_PROXY is on. */
    async fetch(req: Request, ip = "unknown"): Promise<Response> {
      try {
        const url = new URL(req.url);
        if (url.pathname === "/webhooks/payments-lk") {
          return req.method === "POST"
            ? await webhook(req)
            : fail(405, "method_not_allowed", "POST only");
        }
        const cors: Record<string, string> =
          req.headers.get("origin") === allowed
            ? {
                "access-control-allow-origin": allowed,
                "access-control-allow-methods": "GET, POST, OPTIONS",
                "access-control-allow-headers": "content-type",
                "access-control-max-age": "600",
                vary: "Origin",
              }
            : {};
        const res =
          req.method === "OPTIONS"
            ? new Response(null, { status: 204 })
            : await route(req, url, clientIp(req, ip));
        for (const [name, value] of Object.entries(cors)) res.headers.set(name, value);
        return res;
      } catch (error) {
        console.error(`unhandled ${req.method} error (${errorName(error)})`);
        return fail(500, "internal", "Something went wrong");
      }
    },
  };

  function clientIp(req: Request, socketIp: string): string {
    if (!d.config.trustProxy) return socketIp;
    return req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || socketIp;
  }

  async function route(req: Request, url: URL, ip: string): Promise<Response> {
    const { pathname } = url;
    const key = `${req.method} ${pathname}`;
    switch (key) {
      case "GET /health":
        return json(200, { ok: true, keyId: keyId(d.key), mode: d.config.mode });
      case "GET /price":
        return json(200, {
          amountCents: d.config.priceCents,
          currency: "LKR",
          maxMajor: d.config.major,
        });
      case "POST /checkout":
        return checkout(req, ip);
      case "POST /trial":
        return emailed(req, ip, "trial", startTrial);
      case "POST /recover":
        return emailed(req, ip, "recover", recover);
    }
    const ref = req.method === "GET" && pathname.startsWith("/orders/") ? pathname.slice(8) : "";
    if (REF.test(ref)) {
      const state = await orderState(d, ref);
      if (state) return json(200, state);
    }
    return fail(404, "not_found", "Not found");
  }

  async function checkout(req: Request, ip: string): Promise<Response> {
    const email = await readEmail(req);
    if (!email) return fail(400, "invalid_email", "Enter a valid email address");
    if (!withinLimit(`checkout:ip:${ip}`, 10, HOUR)) return tooMany();
    try {
      return json(200, { url: await createCheckout(d, email) });
    } catch (error) {
      console.error(`checkout for ${maskEmail(email)} failed (${errorName(error)})`);
      return fail(502, "checkout_failed", "Could not start the payment, try again");
    }
  }

  // Always 202 so the answer never shows whether the address has a trial or licenses.
  async function emailed(
    req: Request,
    ip: string,
    name: string,
    action: (d: Deps, email: string) => void,
  ): Promise<Response> {
    const email = await readEmail(req);
    if (!email) return fail(400, "invalid_email", "Enter a valid email address");
    if (!withinLimit(`${name}:ip:${ip}`, 10, HOUR) || !withinLimit(`${name}:email:${email}`, 3, HOUR)) {
      return tooMany();
    }
    action(d, email);
    return json(202, { sent: true });
  }

  async function webhook(req: Request): Promise<Response> {
    const raw = await req.text();
    let event;
    try {
      event = d.lk.webhooks.constructEvent(
        raw,
        req.headers.get("payments-signature"),
        d.config.webhookSecret,
        { now: Math.floor(d.now() / 1000) },
      );
    } catch (error) {
      if (error instanceof WebhookSignatureError) {
        return fail(400, "bad_signature", "Signature check failed");
      }
      throw error;
    }
    await handleEvent(d, event);
    return json(200, { received: true });
  }
}

const tooMany = () => fail(429, "rate_limited", "Too many requests, try again later");

/** The normalized address, or null when the body is not a small JSON object with a plausible email. */
async function readEmail(req: Request): Promise<string | null> {
  const text = await req.text();
  if (text.length > MAX_BODY) return null;
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    return null;
  }
  const raw = (body as { email?: unknown } | null)?.email;
  if (typeof raw !== "string") return null;
  const email = raw.trim().toLowerCase();
  return email.length <= 254 && EMAIL.test(email) ? email : null;
}
