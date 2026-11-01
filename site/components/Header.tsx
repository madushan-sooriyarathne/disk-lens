import Link from "next/link";

import styles from "./Header.module.css";

const links = [
  { href: "/features", label: "Features" },
  { href: "/pricing", label: "Pricing" },
  { href: "/docs/getting-started", label: "Docs" },
  { href: "/faq", label: "FAQ" },
  { href: "/download", label: "Download" },
];

export function Header() {
  return (
    <header className={styles.header}>
      <div className={`wrap ${styles.bar}`}>
        <Link href="/" className={styles.brand}>
          <picture>
            <source media="(prefers-color-scheme: dark)" srcSet="/icon.png" />
            <img src="/icon-light.png" alt="" width={28} height={28} />
          </picture>
          DiskLens
        </Link>
        <nav aria-label="Main" className={styles.nav}>
          {links.map((l) => (
            <Link key={l.href} href={l.href}>
              {l.label}
            </Link>
          ))}
        </nav>
        <Link href="/buy" className={`btn btn-primary ${styles.cta}`}>
          Buy Pro
        </Link>
      </div>
    </header>
  );
}
