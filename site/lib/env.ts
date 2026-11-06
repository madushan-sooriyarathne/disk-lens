// Public values, inlined at build time. The site holds no secrets.
export const SERVER_URL = process.env.NEXT_PUBLIC_LICENSE_SERVER_URL ?? "http://localhost:8787";
export const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? "http://localhost:3000";
export const DOWNLOAD_URL =
  process.env.NEXT_PUBLIC_DOWNLOAD_URL ?? "https://github.com/madushan-sooriyarathne/disk-lens/releases/latest";
