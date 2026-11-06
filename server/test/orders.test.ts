import { describe, expect, test } from "bun:test";

import {
  event,
  makeHarness,
  orderOf,
  pay,
  payment,
  postJson,
  request,
  startCheckout,
  times,
  webhook,
} from "./harness";

describe("POST /checkout", () => {
  test("returns the payments.lk checkout url", async () => {
    const h = makeHarness();
    const res = await postJson(h, "/checkout", { email: "buyer@example.com" });
    expect(await res.json()).toEqual({ url: "https://pay.test/chk_1" });
  });

  test("stores a pending order with the checkout id and price", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h, " Buyer@Example.com ");
    expect(orderOf(h, ref)).toMatchObject({
      email: "buyer@example.com",
      checkout_id: "chk_1",
      amount_cents: 500_000,
      mode: "test",
      status: "pending",
      token: null,
    });
  });

  test("sends the order ref as the payment reference", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    expect(h.apiCalls[0]!.body.reference).toBe(ref);
  });

  test("sends the order ref as the idempotency key", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    expect(h.apiCalls[0]!.headers["idempotency-key"]).toBe(ref);
  });

  test("sends the site return urls", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    expect(h.apiCalls[0]!.body).toMatchObject({
      successUrl: `https://site.test/success?ref=${ref}`,
      cancelUrl: "https://site.test/cancel",
    });
  });

  test("rejects a bad email", async () => {
    const h = makeHarness();
    const res = await postJson(h, "/checkout", { email: "not-an-email" });
    expect(res.status).toBe(400);
  });

  test("answers 502 and fails the order when payments.lk refuses", async () => {
    const h = makeHarness();
    h.state.apiDown = true;
    const res = await postJson(h, "/checkout", { email: "buyer@example.com" });
    expect(res.status).toBe(502);
  });

  test("rate limits the eleventh checkout from one address", async () => {
    const h = makeHarness();
    await times(10, (i) => postJson(h, "/checkout", { email: `b${i}@example.com` }));
    const res = await postJson(h, "/checkout", { email: "late@example.com" });
    expect(res.status).toBe(429);
  });
});

describe("POST /webhooks/payments-lk", () => {
  test("marks the order paid with a token on a signed payment.succeeded", async () => {
    const h = makeHarness();
    const ref = await pay(h);
    expect(orderOf(h, ref)).toMatchObject({ status: "paid", token: expect.stringMatching(/^DL1\./) });
  });

  test("emails the token to the buyer", async () => {
    const h = makeHarness();
    const ref = await pay(h);
    expect(h.mails[0]!.react.props.tokens[0]!.token).toBe(orderOf(h, ref).token);
  });

  test("mints once when the same event arrives twice", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    const evt = event("payment.succeeded", payment(ref));
    await webhook(h, evt);
    const first = orderOf(h, ref).token;
    await webhook(h, evt);
    expect(orderOf(h, ref).token).toBe(first);
  });

  test("sends one email when the same event arrives twice", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    const evt = event("payment.succeeded", payment(ref));
    await webhook(h, evt);
    await webhook(h, evt);
    expect(h.mails).toHaveLength(1);
  });

  test("answers 400 to a bad signature", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    const res = await webhook(h, event("payment.succeeded", payment(ref)), { secret: "whsec_other" });
    expect(res.status).toBe(400);
  });

  test("leaves the order pending after a bad signature", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.succeeded", payment(ref)), { secret: "whsec_other" });
    expect(orderOf(h, ref).status).toBe("pending");
  });

  test("rejects a signature older than the tolerance", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    const res = await webhook(h, event("payment.succeeded", payment(ref)), {
      t: Math.floor(h.state.now / 1000) - 3600,
    });
    expect(res.status).toBe(400);
  });

  test("ignores a live event on a test server", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.succeeded", payment(ref, { mode: "live" }), "live"));
    expect(orderOf(h, ref).status).toBe("pending");
  });

  test("acknowledges an event of the other mode with 200", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    const res = await webhook(h, event("payment.succeeded", payment(ref, { mode: "live" }), "live"));
    expect(res.status).toBe(200);
  });

  test("does not settle an amount below the order amount", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.succeeded", payment(ref, { amountCents: 499_999 })));
    expect(orderOf(h, ref).status).toBe("pending");
  });

  test("does not settle a payment whose reference is another order", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.succeeded", payment("ord_other", {})));
    expect(orderOf(h, ref).status).toBe("pending");
  });

  test("does not settle a payment that has not succeeded", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.succeeded", payment(ref, { status: "processing" })));
    expect(orderOf(h, ref).status).toBe("pending");
  });

  test("keeps the order paid when the email fails", async () => {
    const h = makeHarness();
    h.state.failMail = true;
    const ref = await pay(h);
    expect(orderOf(h, ref).status).toBe("paid");
  });

  test("marks a pending order failed on payment.failed", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.failed", payment(ref, { status: "failed" })));
    expect(orderOf(h, ref).status).toBe("failed");
  });

  test("settles an order after an earlier failed attempt", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("payment.failed", payment(ref, { status: "failed" })));
    await webhook(h, event("payment.succeeded", payment(ref)));
    expect(orderOf(h, ref).status).toBe("paid");
  });

  test("marks the order expired on checkout.expired", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    await webhook(h, event("checkout.expired", { object: "checkout", id: "chk_1", paymentId: "pay_1" }));
    expect(orderOf(h, ref).status).toBe("expired");
  });

  test("does not expire a paid order", async () => {
    const h = makeHarness();
    const ref = await pay(h);
    await webhook(h, event("checkout.expired", { object: "checkout", id: "chk_1", paymentId: "pay_1" }));
    expect(orderOf(h, ref).status).toBe("paid");
  });

  test("marks a paid order refunded on refund.succeeded", async () => {
    const h = makeHarness();
    const ref = await pay(h);
    await webhook(h, event("refund.succeeded", { object: "refund", id: "ref_1", payment: payment(ref) }));
    expect(orderOf(h, ref).status).toBe("refunded");
  });

  test("answers 200 to an event type it does not handle", async () => {
    const h = makeHarness();
    const res = await webhook(h, event("test.ping", { object: "ping", endpointId: "we_1" }));
    expect(res.status).toBe(200);
  });
});

describe("GET /orders/:ref", () => {
  test("settles a pending order whose checkout says the payment succeeded", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    h.state.paymentStatus = "succeeded";
    const res = await h.app.fetch(request(`/orders/${ref}`));
    expect(await res.json()).toEqual({ status: "paid", token: orderOf(h, ref).token });
  });

  test("emails once when the poll and the webhook both settle", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    h.state.paymentStatus = "succeeded";
    await h.app.fetch(request(`/orders/${ref}`));
    await webhook(h, event("payment.succeeded", payment(ref)));
    expect(h.mails).toHaveLength(1);
  });

  test("returns no token while the order is pending", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    const res = await h.app.fetch(request(`/orders/${ref}`));
    expect(await res.json()).toEqual({ status: "pending" });
  });

  test("returns no token for a refunded order", async () => {
    const h = makeHarness();
    const ref = await pay(h);
    await webhook(h, event("refund.succeeded", { object: "refund", id: "ref_1", payment: payment(ref) }));
    const res = await h.app.fetch(request(`/orders/${ref}`));
    expect(await res.json()).toEqual({ status: "refunded" });
  });

  test("answers 404 to a malformed ref", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(request("/orders/not-a-ref"));
    expect(res.status).toBe(404);
  });

  test("answers 404 to an unknown ref", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(request("/orders/ord_aaaaaaaaaaaaaaaaaaaaaaaaaa"));
    expect(res.status).toBe(404);
  });

  test("keeps the order pending when the checkout lookup fails", async () => {
    const h = makeHarness();
    const ref = await startCheckout(h);
    h.state.apiDown = true;
    const res = await h.app.fetch(request(`/orders/${ref}`));
    expect(await res.json()).toEqual({ status: "pending" });
  });
});

describe("GET /health and /price", () => {
  test("reports the key id and mode", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(request("/health"));
    expect(await res.json()).toEqual({ ok: true, keyId: "1807F6E5D4C3B2A1", mode: "test" });
  });

  test("reports the price from the config", async () => {
    const h = makeHarness();
    const res = await h.app.fetch(request("/price"));
    expect(await res.json()).toEqual({ amountCents: 500_000, currency: "LKR", maxMajor: 1 });
  });
});
