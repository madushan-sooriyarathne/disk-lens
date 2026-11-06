import type { Payment, WebhookEvent, WebhookTestPing } from "@payments-lk/node";

import { licenseEmail } from "../emails/LicenseEmail";
import type { Deps } from "./app";
import type { Order, OrderStatus } from "./db";
import { mint, newId } from "./license";
import { errorName } from "./mail";

const seconds = (d: Deps) => Math.floor(d.now() / 1000);

/** Returns the payments.lk checkout URL for a new pending order. */
export async function createCheckout(d: Deps, email: string): Promise<string> {
  const { siteUrl, priceCents, major, mode } = d.config;
  const ref = newId("ord_");
  d.db
    .query(
      "INSERT INTO orders (ref, email, amount_cents, mode, status, created) VALUES (?, ?, ?, ?, 'pending', ?)",
    )
    .run(ref, email, priceCents, mode, seconds(d));
  try {
    const checkout = await d.lk.checkouts.create(
      {
        amountCents: priceCents,
        description: `DiskLens Pro ${major}.x license`,
        reference: ref,
        // The SDK types require a name; the buyer gives only an email.
        customer: { name: "DiskLens customer", email },
        successUrl: `${siteUrl}/success?ref=${ref}`,
        cancelUrl: `${siteUrl}/cancel`,
      },
      { idempotencyKey: ref },
    );
    d.db.query("UPDATE orders SET checkout_id = ? WHERE ref = ?").run(checkout.id, ref);
    return checkout.url;
  } catch (error) {
    mark(d, ref, "failed", "pending");
    throw error;
  }
}

export async function handleEvent(d: Deps, event: WebhookEvent | WebhookTestPing): Promise<void> {
  // A test payment must never mint a live license.
  if (event.mode !== d.config.mode) return;
  switch (event.type) {
    case "payment.succeeded":
      return settle(d, event.data.reference ?? "", event.data);
    case "payment.failed":
      return mark(d, event.data.reference ?? "", "failed", "pending");
    case "checkout.expired":
      d.db
        .query("UPDATE orders SET status = 'expired' WHERE checkout_id = ? AND status = 'pending'")
        .run(event.data.id);
      return;
    case "refund.succeeded":
      // Licenses are offline, so a refund only records the order.
      return mark(d, event.data.payment.reference ?? "", "refunded", "paid");
  }
}

/** Used by the webhook and the poll: whichever arrives first mints, the other is a no-op. */
export async function settle(d: Deps, ref: string, payment: Payment): Promise<void> {
  const order = d.db.query("SELECT * FROM orders WHERE ref = ?").get(ref) as Order | null;
  if (
    !order ||
    payment.status !== "succeeded" ||
    payment.currency !== "LKR" ||
    payment.amountCents < order.amount_cents ||
    payment.mode !== d.config.mode ||
    payment.reference !== ref
  ) {
    return;
  }
  const issued = seconds(d);
  const token = mint(d.key, {
    id: newId("lic_"),
    kind: "pro",
    issued,
    maxMajor: d.config.major,
    expires: null,
  });
  const claimed = d.db
    .query("UPDATE orders SET status = 'paid', token = ?, paid = ? WHERE ref = ? AND token IS NULL")
    .run(token, issued, ref);
  if (claimed.changes !== 1) return;

  console.log(`order ${ref} paid`);
  try {
    await d.mailer({
      to: order.email,
      ...licenseEmail({
        kind: "purchase",
        tokens: [{ kind: "pro", token }],
        siteUrl: d.config.siteUrl,
      }),
    });
  } catch (error) {
    // The success page shows the token and /recover can send it again.
    console.error(`order ${ref}: purchase email failed (${errorName(error)})`);
  }
}

export async function orderState(
  d: Deps,
  ref: string,
): Promise<{ status: OrderStatus; token?: string } | null> {
  let order = find(d, ref);
  if (!order) return null;
  if (order.status === "pending" && order.checkout_id) {
    // Covers a missing webhook (local development) and a late one.
    try {
      const checkout = await d.lk.checkouts.retrieve(order.checkout_id);
      await settle(d, ref, checkout.payment);
      order = find(d, ref) ?? order;
    } catch (error) {
      console.error(`order ${ref}: checkout lookup failed (${errorName(error)})`);
    }
  }
  return order.status === "paid" && order.token
    ? { status: order.status, token: order.token }
    : { status: order.status };
}

const find = (d: Deps, ref: string) =>
  d.db.query("SELECT * FROM orders WHERE ref = ?").get(ref) as Order | null;

function mark(d: Deps, ref: string, to: OrderStatus, from: OrderStatus): void {
  d.db.query("UPDATE orders SET status = ? WHERE ref = ? AND status = ?").run(to, ref, from);
}
