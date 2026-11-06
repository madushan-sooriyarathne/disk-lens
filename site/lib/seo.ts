import type { Metadata } from "next";

// Canonicals are relative and resolve against metadataBase in the root layout.
export function meta(title: string, description: string, path: string, noindex = false): Metadata {
  return {
    title,
    description,
    alternates: { canonical: path },
    openGraph: { title, description, url: path, type: "website" },
    ...(noindex && { robots: { index: false, follow: false } }),
  };
}
