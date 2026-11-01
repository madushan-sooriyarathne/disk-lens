import { describe, expect, test } from "bun:test";

import { event, makeHarness, payment, request, SITE, webhook } from "./harness";

const preflight = (origin: string) =>
  request("/checkout", {
    method: "OPTIONS",
    headers: { origin, "access-control-request-method": "POST" },
  });

describe("CORS", () => {
  test("allows a preflight from the site", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(preflight(SITE));
    expect(res.headers.get("access-control-allow-origin")).toBe(SITE);
  });

  test("answers a preflight with 204", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(preflight(SITE));
    expect(res.status).toBe(204);
  });

  test("gives another origin no allow-origin header", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(preflight("https://evil.test"));
    expect(res.headers.get("access-control-allow-origin")).toBeNull();
  });

  test("adds the allow-origin header to a normal response for the site", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(request("/price", { headers: { origin: SITE } }));
    expect(res.headers.get("access-control-allow-origin")).toBe(SITE);
  });

  test("adds no allow-origin header to a response for another origin", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(request("/price", { headers: { origin: "https://evil.test" } }));
    expect(res.headers.get("access-control-allow-origin")).toBeNull();
  });

  test("never allows an origin on the webhook route", async () => {
    const h = makeHarness();
    const res = await webhook(h, event("payment.succeeded", payment("ord_x")));
    expect(res.headers.get("access-control-allow-origin")).toBeNull();
  });
});
