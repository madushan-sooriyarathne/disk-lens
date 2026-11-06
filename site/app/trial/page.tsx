import { EmailForm } from "@/components/EmailForm";
import { meta } from "@/lib/seo";

export const metadata = meta(
  "Start a 7-day Pro trial",
  "Try every DiskLens Pro feature free for 7 days. We email the trial key once per address.",
  "/trial",
);

export default function Trial() {
  return (
    <div className="wrap page">
      <h1>Start a 7-day trial</h1>
      <p className="lead">Every Pro feature that exists today, free for 7 days. We email the key to you.</p>
      <EmailForm
        action="trial"
        label="Email my trial key"
        busyLabel="Sending…"
        done={
          <>
            <strong>Check your inbox.</strong> If you haven&rsquo;t had a trial before, the key is on its way. Open the link in
            the email, or paste the key into DiskLens Settings.
          </>
        }
      />
      <p className="muted">One trial per email address. Already have a key? Use the same address on the recover page.</p>
    </div>
  );
}
