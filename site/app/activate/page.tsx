import { meta } from "@/lib/seo";

import { Activate } from "./Activate";

export const metadata = meta(
  "Activate DiskLens Pro",
  "Open your DiskLens license key in the app.",
  "/activate",
  true,
);

export default function Page() {
  return (
    <div className="wrap page">
      <h1>Activate DiskLens Pro</h1>
      <Activate />
    </div>
  );
}
