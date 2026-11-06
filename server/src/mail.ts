import { mkdir } from "node:fs/promises";
import type { ReactElement } from "react";

import { SESv2Client, SendEmailCommand } from "@aws-sdk/client-sesv2";
import { render } from "@react-email/render";

export type Mailer = (mail: { to: string; subject: string; react: ReactElement }) => Promise<void>;

/** `a***@domain`, for logs. */
export function maskEmail(email: string): string {
  const at = email.lastIndexOf("@");
  return at < 1 ? "***" : `${email[0]}***${email.slice(at)}`;
}

export function createMailer(opts: { from: string; dryRun: boolean; dir: URL }): Mailer {
  // Created on first send so dry runs and tests never need AWS credentials.
  let ses: SESv2Client | undefined;

  return async ({ to, subject, react }) => {
    const html = await render(react);
    if (opts.dryRun) {
      await mkdir(opts.dir, { recursive: true });
      const name = `${Date.now()}-${to.replace(/[^a-z0-9@.+-]/gi, "_")}.html`;
      await Bun.write(new URL(name, opts.dir), html);
      return;
    }
    const text = await render(react, { plainText: true });
    ses ??= new SESv2Client({});
    await ses.send(
      new SendEmailCommand({
        FromEmailAddress: opts.from,
        Destination: { ToAddresses: [to] },
        Content: {
          Simple: {
            Subject: { Data: subject, Charset: "UTF-8" },
            Body: {
              Html: { Data: html, Charset: "UTF-8" },
              Text: { Data: text, Charset: "UTF-8" },
            },
          },
        },
      }),
    );
  };
}

/** Error class name only: SES and API messages can quote the recipient address. */
export function errorName(error: unknown): string {
  return error instanceof Error ? error.name : "UnknownError";
}
