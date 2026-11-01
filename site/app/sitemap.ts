import type { MetadataRoute } from "next";

import { docs } from "@/content/docs";
import { SITE_URL } from "@/lib/env";

// Flow pages (success, activate, recover, cancel) are noindex and stay out.
const paths = [
  "/",
  "/features",
  "/pricing",
  "/download",
  "/faq",
  "/changelog",
  "/buy",
  "/trial",
  "/privacy",
  "/terms",
  "/docs",
  ...docs.map((d) => `/docs/${d.slug}`),
];

export default function sitemap(): MetadataRoute.Sitemap {
  return paths.map((path) => ({ url: SITE_URL + path }));
}
