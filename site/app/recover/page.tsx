import { EmailForm } from "@/components/EmailForm";
import { meta } from "@/lib/seo";

export const metadata = meta(
  "Recover your license",
  "Lost your DiskLens license key? Enter your email and we send every key for that address.",
  "/recover",
  true,
);

export default function Recover() {
  return (
    <div className="wrap page">
      <h1>Recover your license</h1>
      <p className="lead">Enter the email you used. We send every key for that address.</p>
      <EmailForm
        action="recover"
        label="Email my keys"
        busyLabel="Sending…"
        done="If we have licenses for that address, we've emailed them."
      />
    </div>
  );
}
