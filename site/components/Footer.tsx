import Link from "next/link";

import styles from "./Footer.module.css";

export function Footer() {
  return (
    <footer className={styles.footer}>
      <div className={`wrap ${styles.cols}`}>
        <p className={styles.note}>
          <strong>DiskLens</strong>
          <br />
          A disk space analyzer and safe cleaner for macOS.
        </p>
        <nav aria-label="Product">
          <h2>Product</h2>
          <Link href="/features">Features</Link>
          <Link href="/pricing">Pricing</Link>
          <Link href="/download">Download</Link>
          <Link href="/changelog">Changelog</Link>
        </nav>
        <nav aria-label="Help">
          <h2>Help</h2>
          <Link href="/docs/getting-started">Docs</Link>
          <Link href="/faq">FAQ</Link>
          <Link href="/recover">Recover a license</Link>
          <Link href="/trial">Start a trial</Link>
        </nav>
        <nav aria-label="Legal">
          <h2>Legal</h2>
          <Link href="/privacy">Privacy</Link>
          <Link href="/terms">Terms</Link>
        </nav>
      </div>
    </footer>
  );
}
