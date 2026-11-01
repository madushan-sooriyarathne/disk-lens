import Link from "next/link";

import { meta } from "@/lib/seo";

export const metadata = meta("Payment cancelled", "Your DiskLens Pro payment was cancelled.", "/cancel", true);

export default function Cancel() {
  return (
    <div className="wrap page">
      <h1>Payment cancelled</h1>
      <p className="lead">You haven&rsquo;t been charged. You can start again whenever you like.</p>
      <p>
        <Link className="btn btn-primary" href="/pricing">
          Back to pricing
        </Link>
      </p>
    </div>
  );
}
