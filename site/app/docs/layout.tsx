import Link from "next/link";
import type { ReactNode } from "react";

import { docs } from "@/content/docs";

import styles from "./docs.module.css";

export default function DocsLayout({ children }: { children: ReactNode }) {
  return (
    <div className={`wrap ${styles.docs}`}>
      <nav aria-label="Docs" className={styles.nav}>
        <h2>Docs</h2>
        {docs.map((d) => (
          <Link key={d.slug} href={`/docs/${d.slug}`}>
            {d.title}
          </Link>
        ))}
      </nav>
      <article className="prose">{children}</article>
    </div>
  );
}
