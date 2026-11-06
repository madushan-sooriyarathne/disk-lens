"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { TokenCard } from "@/components/TokenCard";

const TOKEN = /^DL1\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;

// The key lives in the URL fragment so it never reaches a server log. It is read here
// and shown; it is never sent anywhere.
export function Activate() {
  const [token, setToken] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    const read = () => {
      const value = location.hash.replace(/^#/, "");
      setToken(TOKEN.test(value) ? value : null);
    };
    read();
    addEventListener("hashchange", read);
    return () => removeEventListener("hashchange", read);
  }, []);

  if (token === undefined) return <p className="muted">Reading your key…</p>;
  if (token) return <TokenCard token={token} />;
  return (
    <div className="prose">
      <p>
        This page needs a license key in its link, and the link didn&rsquo;t contain a valid one. Use the link from your
        email, or get a key:
      </p>
      <ul>
        <li>
          <Link href="/pricing">Buy DiskLens Pro</Link>
        </li>
        <li>
          <Link href="/trial">Start a 7-day trial</Link>
        </li>
        <li>
          <Link href="/recover">Email me my keys</Link>
        </li>
      </ul>
    </div>
  );
}
