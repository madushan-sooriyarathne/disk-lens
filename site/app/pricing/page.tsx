import Link from "next/link";

import { FeatureList } from "@/components/FeatureList";
import { faq } from "@/content/faq";
import { formatPrice, getPrice } from "@/lib/price";
import { meta } from "@/lib/seo";

import styles from "./pricing.module.css";

export const metadata = meta(
  "Pricing",
  "DiskLens is free for every cleanup feature. Pro is a one-time purchase with paid major upgrades and a 7-day trial.",
  "/pricing",
);
// The price fetch is cached for 5 minutes; the page rebuilds on the same cadence.
export const revalidate = 300;

export default async function Pricing() {
  const price = await getPrice();
  return (
    <div className="wrap page">
      <h1>Pricing</h1>
      <p className="lead">Free covers scanning, Explore and every cleanup. Pro is for people who want more.</p>
      <section className={styles.card} aria-labelledby="pro">
        <h2 id="pro">DiskLens Pro{price ? ` ${price.maxMajor}.x` : ""}</h2>
        <p className={styles.price}>{price ? formatPrice(price) : "See checkout for price"}</p>
        <p>
          One-time purchase{price ? `. Covers every ${price.maxMajor}.x update` : ""}. New major versions are a paid
          upgrade, and your license keeps working on the version you bought.
        </p>
        <div className={styles.actions}>
          <Link className="btn btn-primary" href="/buy">
            Buy Pro
          </Link>
          <Link className="btn" href="/trial">
            Start a 7-day trial
          </Link>
        </div>
        <p className="muted">
          The trial is fully usable and is sent by email, once per address. Prices are in Sri Lankan rupees; your card
          issuer converts other currencies.
        </p>
      </section>
      <h2>What Pro includes</h2>
      <p className="muted">
        Today Pro includes palette batch commands and saved filters. Features marked &ldquo;Coming in 2.x&rdquo; are
        planned and are not in the app yet.
      </p>
      <FeatureList tier="pro" />
      <h2>Questions</h2>
      <div className={styles.faq}>
        {faq.slice(0, 3).map((f) => (
          <details key={f.q}>
            <summary>{f.q}</summary>
            <p>{f.a}</p>
          </details>
        ))}
        <p>
          <Link href="/faq">All questions</Link>
        </p>
      </div>
    </div>
  );
}
