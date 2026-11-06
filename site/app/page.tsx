import type { Metadata } from "next";
import Link from "next/link";

import { FeatureList } from "@/components/FeatureList";
import { JsonLd } from "@/components/JsonLd";
import { Shot } from "@/components/Shot";
import { features } from "@/content/features";
import { DOWNLOAD_URL, SITE_URL } from "@/lib/env";
import { meta } from "@/lib/seo";

import styles from "./home.module.css";

const title = "DiskLens: disk space analyzer and safe cleaner for macOS";

export const metadata: Metadata = {
  ...meta(
    title,
    "See what fills your Mac as a treemap, understand every large folder, and free space from caches, build output and models only when you choose to.",
    "/",
  ),
  // The root layout's title template would double the brand here.
  title: { absolute: title },
};

const points = [
  { title: "See", text: "Your home folder as one readable treemap. Textures show which tiles are reclaimable before you read a label." },
  { title: "Understand", text: "Every large folder gets a plain explanation: what it is, what happens if it goes, and how it comes back." },
  { title: "Clean safely", text: "Stage items, review them in one list, and confirm once. Most items go to the Trash, and you can export the steps as a script." },
];

const safety = [
  "Nothing is removed until you click the confirm button. Shortcuts and the command palette only stage items.",
  "Each item is checked again right before it runs, and skipped if it changed since the scan.",
  "Most items move to the Trash. Cleanups can be exported as a readable shell script.",
  "Repositories are never deleted. Build output in a repository with uncommitted or unpushed work is flagged first.",
  "No administrator password, and no files leave your Mac. DiskLens reads sizes and names, not contents.",
  "The selection starts empty on every launch, and every action goes in an audit log.",
];

const jsonLd = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: "DiskLens",
  applicationCategory: "UtilitiesApplication",
  operatingSystem: "macOS 13 or later",
  description: "Disk space analyzer and safe cleaner for macOS developers and power users.",
  url: SITE_URL,
  downloadUrl: DOWNLOAD_URL,
  image: `${SITE_URL}/icon.png`,
};

export default function Home() {
  const highlights = features.filter((f) => f.tier === "free").length;
  return (
    <>
      <JsonLd data={jsonLd} />
      <section className={`wrap ${styles.hero}`}>
        <h1>
          See what fills your Mac.
          <br />
          Clear only what&rsquo;s <span className={`${styles.mark} stripes`}>safe</span>.
        </h1>
        <p className="lead">
          DiskLens maps your home folder as a treemap, explains each large folder in plain language, and frees space from
          caches, build output and models, only when you choose to.
        </p>
        <div className={styles.actions}>
          <a className="btn btn-primary" href={DOWNLOAD_URL}>
            Download for macOS
          </a>
          <Link className="btn" href="/pricing">
            See Pro
          </Link>
        </div>
        <p className="muted">Free to use. macOS 13 or later, Apple Silicon and Intel.</p>
        <Shot name="explore" priority alt="DiskLens Explore page: a treemap of the home folder with the inspector open on the Desktop folder." />
        <ul className={styles.legend} aria-label="Texture key">
          <li><i className={`${styles.swatch} ${styles.disposable} stripes`} />Disposable: caches and build output that rebuild</li>
          <li><i className={`${styles.swatch} ${styles.restorable}`} />Restorable: review first</li>
          <li><i className={`${styles.swatch} ${styles.kept}`} />Your data: never suggested</li>
        </ul>
      </section>

      <section className="section wrap">
        <div className={styles.points}>
          {points.map((p) => (
            <div key={p.title}>
              <h2>{p.title}</h2>
              <p>{p.text}</p>
            </div>
          ))}
        </div>
      </section>

      <section className={`section ${styles.band}`}>
        <div className="wrap">
          <h2>Built so nothing goes wrong</h2>
          <ul className={styles.safety}>
            {safety.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
          <p>
            <Link href="/docs/how-cleanup-works">How cleanup works</Link>
          </p>
        </div>
      </section>

      <section className="section wrap">
        <div className={styles.head}>
          <h2>Free covers all of cleanup</h2>
          <p className="lead">
            {highlights} features, with no limit on what you can clean. Pro adds new capabilities on top.
          </p>
        </div>
        <FeatureList tier="free" />
      </section>

      <section className={`section ${styles.band}`}>
        <div className={`wrap ${styles.cta}`}>
          <h2>Try Pro free for 7 days</h2>
          <p className="lead">One-time purchase. Licenses work offline, with no account and no sign-in.</p>
          <p className="muted">Today Pro adds palette batch commands and saved filters. More Pro features are planned for 2.x.</p>
          <div className={styles.actions}>
            <Link className="btn btn-primary" href="/trial">
              Start a trial
            </Link>
            <Link className="btn" href="/pricing">
              Pricing
            </Link>
          </div>
        </div>
      </section>
    </>
  );
}
