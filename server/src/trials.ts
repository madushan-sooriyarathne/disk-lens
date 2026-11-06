import { createHash } from "node:crypto";

import { licenseEmail, type EmailToken } from "../emails/LicenseEmail";
import type { Deps } from "./app";
import { TRIAL_SECONDS, mint, newId } from "./license";
import { errorName } from "./mail";

type TrialRow = { token: string; created: number };

/** Mints the one trial for this address, or finds it, and emails it. Callers answer the same either way. */
export function startTrial(d: Deps, email: string): void {
  const hash = createHash("sha256").update(email).digest("hex");
  let row = d.db.query("SELECT token, created FROM trials WHERE email_hash = ?").get(hash) as
    | TrialRow
    | null;
  if (!row) {
    const issued = Math.floor(d.now() / 1000);
    const token = mint(d.key, {
      id: newId("lic_"),
      kind: "trial",
      issued,
      maxMajor: d.config.major,
      expires: issued + TRIAL_SECONDS,
    });
    d.db
      .query("INSERT OR IGNORE INTO trials (email_hash, email, token, created) VALUES (?, ?, ?, ?)")
      .run(hash, email, token, issued);
    row = d.db
      .query("SELECT token, created FROM trials WHERE email_hash = ?")
      .get(hash) as TrialRow;
  }
  deliver(d, email, "trial", [trialToken(row)]);
}

/** Emails every license held for this address, if any. */
export function recover(d: Deps, email: string): void {
  const paid = d.db
    .query("SELECT token FROM orders WHERE email = ? AND status = 'paid' AND token IS NOT NULL ORDER BY paid")
    .all(email) as { token: string }[];
  const trial = d.db
    .query("SELECT token, created FROM trials WHERE email_hash = ?")
    .get(createHash("sha256").update(email).digest("hex")) as TrialRow | null;
  const tokens: EmailToken[] = paid.map((o) => ({ kind: "pro", token: o.token }));
  if (trial) tokens.push(trialToken(trial));
  if (tokens.length > 0) deliver(d, email, "recover", tokens);
}

const trialToken = (row: TrialRow): EmailToken => ({
  kind: "trial",
  token: row.token,
  expires: row.created + TRIAL_SECONDS,
});

// Not awaited: the response must not reveal, by its latency, whether an address had licenses.
function deliver(d: Deps, to: string, kind: "trial" | "recover", tokens: EmailToken[]): void {
  d.mailer({ to, ...licenseEmail({ kind, tokens, siteUrl: d.config.siteUrl }) }).catch((error) =>
    console.error(`${kind} email failed (${errorName(error)})`),
  );
}
