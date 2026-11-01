import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";

import { Footer } from "@/components/Footer";
import { Header } from "@/components/Header";
import { SITE_URL } from "@/lib/env";

import "./globals.css";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: { default: "DiskLens: disk space analyzer and safe cleaner for macOS", template: "%s · DiskLens" },
  description:
    "See what fills your Mac as a treemap, understand every large folder, and free space from caches, build output and models only when you choose to.",
  applicationName: "DiskLens",
  icons: { icon: "/icon.png", apple: "/icon-light.png" },
  openGraph: { siteName: "DiskLens", type: "website", locale: "en_US" },
  twitter: { card: "summary_large_image" },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#ffffff" },
    { media: "(prefers-color-scheme: dark)", color: "#1c1c1e" },
  ],
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <Header />
        <main>{children}</main>
        <Footer />
      </body>
    </html>
  );
}
