import Link from "next/link";

import { DOWNLOAD_URL } from "@/lib/env";
import { meta } from "@/lib/seo";

export const metadata = meta(
  "Download DiskLens for macOS",
  "Download DiskLens for macOS 13 or later, on Apple Silicon and Intel. Free to use.",
  "/download",
);

export default function Download() {
  return (
    <div className="wrap page">
      <h1>Download DiskLens</h1>
      <p className="lead">Free to use. One universal app for Apple Silicon and Intel.</p>
      <p>
        <a className="btn btn-primary" href={DOWNLOAD_URL}>
          Download for macOS
        </a>
      </p>
      <div className="prose">
        <h2>Requirements</h2>
        <ul>
          <li>macOS 13 or later</li>
          <li>Apple Silicon or Intel</li>
        </ul>
        <h2>First launch</h2>
        <p>
          Open the disk image and drag DiskLens to Applications. Depending on how the build is signed, macOS may ask you
          to confirm the first launch: open System Settings, choose Privacy &amp; Security, and click Open Anyway.
        </p>
        <h2>Full Disk Access</h2>
        <p>
          DiskLens works without it, but macOS hides some folders until you allow it, so their sizes are not counted.
          The first-run guide walks you through it. See <Link href="/docs/full-disk-access">Full Disk Access</Link>.
        </p>
        <h2>Updates</h2>
        <p>DiskLens checks once per launch for a newer version and offers to install it. There is no other network use.</p>
      </div>
    </div>
  );
}
