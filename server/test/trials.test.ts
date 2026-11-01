import { describe, expect, test } from "bun:test";

import { makeHarness, pay, postJson, times, type Harness } from "./harness";

const trial = (h: Harness, email: string, ip?: string) =>
  postJson(h, "/trial", { email }, ip);

describe("POST /trial", () => {
  test("mints a trial and emails it on the first request", async () => {
    const h = makeHarness();
    await trial(h, "a@example.com");
    expect(h.mails[0]).toMatchObject({
      to: "a@example.com",
      react: { props: { kind: "trial", tokens: [{ kind: "trial" }] } },
    });
  });

  test("re-sends the same token for the same address in another case", async () => {
    const h = makeHarness();
    await trial(h, "a@example.com");
    await trial(h, "  A@Example.COM ");
    expect(h.mails[1]!.react.props.tokens[0]!.token).toBe(h.mails[0]!.react.props.tokens[0]!.token);
  });

  test("stores one trial per address", async () => {
    const h = makeHarness();
    await trial(h, "a@example.com");
    await trial(h, "A@example.com");
    expect(h.db.query("SELECT COUNT(*) AS n FROM trials").get()).toEqual({ n: 1 });
  });

  test("answers new and repeat addresses the same way", async () => {
    const h = makeHarness();
    const first = await trial(h, "a@example.com");
    const second = await trial(h, "a@example.com");
    expect([second.status, await second.text()]).toEqual([first.status, await first.text()]);
  });

  test("answers 202 sent", async () => {
    const h = makeHarness();
    const res = await trial(h, "a@example.com");
    expect([res.status, await res.json()]).toEqual([202, { sent: true }]);
  });

  test("rejects a bad email", async () => {
    const h = makeHarness();
    const res = await trial(h, "nope");
    expect(res.status).toBe(400);
  });

  test("rate limits the fourth request for one address in an hour", async () => {
    const h = makeHarness();
    await trial(h, "a@example.com", "1.1.1.1");
    await trial(h, "a@example.com", "2.2.2.2");
    await trial(h, "a@example.com", "3.3.3.3");
    const res = await trial(h, "a@example.com", "4.4.4.4");
    expect(res.status).toBe(429);
  });

  test("rate limits the eleventh request from one address", async () => {
    const h = makeHarness();
    await times(10, (i) => trial(h, `u${i}@example.com`));
    const res = await trial(h, "last@example.com");
    expect(res.status).toBe(429);
  });

  test("allows the address again after the window", async () => {
    const h = makeHarness();
    await trial(h, "a@example.com");
    await trial(h, "a@example.com");
    await trial(h, "a@example.com");
    h.state.now += 3_600_001;
    const res = await trial(h, "a@example.com");
    expect(res.status).toBe(202);
  });
});

describe("POST /recover", () => {
  test("emails every paid token plus the trial", async () => {
    const h = makeHarness();
    await pay(h, "a@example.com");
    await pay(h, "a@example.com");
    await trial(h, "a@example.com");
    h.mails.length = 0;
    await postJson(h, "/recover", { email: "A@example.com" });
    expect(h.mails[0]!.react.props.tokens.map((t) => t.kind)).toEqual(["pro", "pro", "trial"]);
  });

  test("sends one email for the address", async () => {
    const h = makeHarness();
    await pay(h, "a@example.com");
    h.mails.length = 0;
    await postJson(h, "/recover", { email: "a@example.com" });
    expect(h.mails).toHaveLength(1);
  });

  test("sends nothing for an unknown address", async () => {
    const h = makeHarness();
    await postJson(h, "/recover", { email: "nobody@example.com" });
    expect(h.mails).toHaveLength(0);
  });

  test("still answers 202 for an unknown address", async () => {
    const h = makeHarness();
    const res = await postJson(h, "/recover", { email: "nobody@example.com" });
    expect([res.status, await res.json()]).toEqual([202, { sent: true }]);
  });

  test("does not use up the trial limit", async () => {
    const h = makeHarness();
    await postJson(h, "/recover", { email: "a@example.com" });
    await postJson(h, "/recover", { email: "a@example.com" });
    await postJson(h, "/recover", { email: "a@example.com" });
    const res = await trial(h, "a@example.com");
    expect(res.status).toBe(202);
  });
});
