import { FeatureList } from "@/components/FeatureList";
import { Shot } from "@/components/Shot";
import { meta } from "@/lib/seo";

import styles from "./features.module.css";

export const metadata = meta(
  "Features",
  "Every DiskLens feature: the treemap, recommendations, staging queue, safe cleanup, command palette and the Pro features, marked as available or coming.",
  "/features",
);

const tour = [
  { name: "overview", title: "Overview", text: "Capacity, what is safe to clean and your largest folders in one glance.", alt: "DiskLens Overview page with disk capacity, safe-to-clean total and largest folders." },
  { name: "recommended", title: "Recommended", text: "Safe items and review-first items, each with an explanation and a blocked reason when an app is running.", alt: "DiskLens Recommended page with the inspector open on Next.js build output." },
  { name: "palette", title: "Command palette", text: "⌘K finds pages, folders and recommendations, and stages items from the keyboard.", alt: "The DiskLens command palette searching for Xcode." },
] as const;

export default function Features() {
  return (
    <div className="wrap page">
      <h1>Features</h1>
      <p className="lead">
        Everything below is listed as it stands in the app today. Features that are not built yet say so.
      </p>
      <div className={styles.tour}>
        {tour.map((t) => (
          <figure key={t.name}>
            <Shot name={t.name} alt={t.alt} />
            <figcaption>
              <strong>{t.title}</strong> {t.text}
            </figcaption>
          </figure>
        ))}
      </div>
      <h2 className={styles.h}>Free</h2>
      <p className="muted">Every cleanup feature. No limit on what you can clean.</p>
      <FeatureList tier="free" />
      <h2 className={styles.h}>Pro</h2>
      <p className="muted">Pro features marked &ldquo;Coming in 2.x&rdquo; are planned and are not in the app yet.</p>
      <FeatureList tier="pro" />
    </div>
  );
}
