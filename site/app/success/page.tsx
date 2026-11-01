import { Suspense } from "react";

import { meta } from "@/lib/seo";

import { Success } from "./Success";

export const metadata = meta("Thank you", "Your DiskLens Pro payment status.", "/success", true);

export default function Page() {
  return (
    <div className="wrap page">
      <h1>Thank you</h1>
      {/* useSearchParams needs a Suspense boundary so the rest of the page can prerender. */}
      <Suspense fallback={<p className="muted">Checking your payment…</p>}>
        <Success />
      </Suspense>
    </div>
  );
}
