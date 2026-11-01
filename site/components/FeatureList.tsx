import { features } from "@/content/features";

import { StatusBadge } from "./StatusBadge";
import styles from "./FeatureList.module.css";

export function FeatureList({ tier }: { tier: "free" | "pro" }) {
  return (
    <ul className={styles.list}>
      {features
        .filter((f) => f.tier === tier)
        .map((f) => (
          <li key={f.slug} className={styles.item}>
            <div className={styles.head}>
              <h3>{f.title}</h3>
              {tier === "pro" && <StatusBadge feature={f} />}
            </div>
            <p>{f.summary}</p>
          </li>
        ))}
    </ul>
  );
}
