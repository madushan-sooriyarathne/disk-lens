"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";

import { TokenCard } from "@/components/TokenCard";
import { api, ApiError } from "@/lib/api";

const REF = /^ord_[a-z2-7]{26}$/;
const EVERY_MS = 2000;
const GIVE_UP_MS = 3 * 60 * 1000;

type Order = { status: "pending" | "paid" | "failed" | "expired" | "refunded"; token?: string };
type View = Order | { status: "timeout" | "unknown" };

export function Success() {
  const ref = useSearchParams().get("ref") ?? "";
  const [view, setView] = useState<View>({ status: "pending" });

  useEffect(() => {
    if (!REF.test(ref)) {
      setView({ status: "unknown" });
      return;
    }
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const started = Date.now();

    async function poll() {
      try {
        const order = await api<Order>(`/orders/${ref}`);
        if (stopped) return;
        if (order.status !== "pending") return setView(order);
      } catch (err) {
        if (stopped) return;
        if (err instanceof ApiError && err.status === 404) return setView({ status: "unknown" });
        // Other errors are transient: keep polling until the deadline.
      }
      if (Date.now() - started >= GIVE_UP_MS) return setView({ status: "timeout" });
      timer = setTimeout(poll, EVERY_MS);
    }

    poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [ref]);

  switch (view.status) {
    case "pending":
      return (
        <p role="status" className="lead">
          Confirming your payment. This usually takes a few seconds…
        </p>
      );
    case "paid":
      return view.token ? (
        <>
          <p className="lead">Payment received. Your key is below, and we have also emailed it to you.</p>
          <TokenCard token={view.token} />
        </>
      ) : (
        <Timeout />
      );
    case "failed":
    case "expired":
      return (
        <>
          <p className="lead">The payment didn&rsquo;t go through, so you haven&rsquo;t been charged for a license.</p>
          <p>
            <Link className="btn btn-primary" href="/buy">
              Try again
            </Link>
          </p>
        </>
      );
    case "refunded":
      return <p className="lead">This order was refunded.</p>;
    case "timeout":
      return <Timeout />;
    default:
      return (
        <p className="lead">
          We couldn&rsquo;t find that order. Use the link from your payment, or <Link href="/recover">recover your license by email</Link>.
        </p>
      );
  }
}

function Timeout() {
  return (
    <p className="lead">
      Still processing. We&rsquo;ll email your key as soon as the payment clears. If it doesn&rsquo;t arrive, use{" "}
      <Link href="/recover">recover your license</Link>.
    </p>
  );
}
