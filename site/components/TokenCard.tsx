"use client";

import { useRef, useState } from "react";

import styles from "./TokenCard.module.css";

export function TokenCard({ token }: { token: string }) {
  const field = useRef<HTMLTextAreaElement>(null);
  const [note, setNote] = useState("");

  async function copy() {
    try {
      await navigator.clipboard.writeText(token);
      setNote("Copied");
    } catch {
      // Clipboard API is missing on insecure origins; leave the text selected for a manual copy.
      field.current?.select();
      setNote("Press ⌘C to copy");
    }
  }

  return (
    <section className={styles.card} aria-labelledby="token-title">
      <h2 id="token-title">Your DiskLens license</h2>
      <div className={styles.actions}>
        <a className="btn btn-primary" href={`disklens://activate?token=${encodeURIComponent(token)}`}>
          Open in DiskLens
        </a>
        <button className="btn" onClick={copy}>
          Copy key
        </button>
        <span role="status" className="muted">
          {note}
        </span>
      </div>
      <label htmlFor="token" className="sr-only">
        License key
      </label>
      <textarea
        id="token"
        ref={field}
        className={styles.token}
        readOnly
        rows={5}
        value={token}
        spellCheck={false}
        onFocus={(e) => e.currentTarget.select()}
      />
      <ol className={styles.steps}>
        <li>Open DiskLens and choose DiskLens › Settings… (⌘,).</li>
        <li>Paste the key into the license field.</li>
        <li>Click Activate.</li>
      </ol>
      <p className="muted">
        “Open in DiskLens” needs the app to be installed. If nothing happens, use the steps above.
      </p>
    </section>
  );
}
