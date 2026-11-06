import { JsonLd } from "@/components/JsonLd";
import { faq } from "@/content/faq";
import { meta } from "@/lib/seo";

import styles from "../pricing/pricing.module.css";

export const metadata = meta(
  "FAQ",
  "Answers about DiskLens licensing, major upgrades, refunds, privacy, pricing in LKR and what cleanup can and cannot delete.",
  "/faq",
);

const jsonLd = {
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: faq.map((f) => ({
    "@type": "Question",
    name: f.q,
    acceptedAnswer: { "@type": "Answer", text: f.a },
  })),
};

export default function Faq() {
  return (
    <div className="wrap page">
      <JsonLd data={jsonLd} />
      <h1>Frequently asked questions</h1>
      <div className={styles.faq}>
        {faq.map((f) => (
          <details key={f.q}>
            <summary>{f.q}</summary>
            <p>{f.a}</p>
          </details>
        ))}
      </div>
    </div>
  );
}
