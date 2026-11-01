import {
  Body,
  Button,
  Container,
  Head,
  Heading,
  Html,
  Link,
  Preview,
  Section,
  Text,
} from "@react-email/components";

export type EmailKind = "purchase" | "trial" | "recover";

export type EmailToken = { kind: "pro" | "trial"; token: string; expires?: number };

type Props = { kind: EmailKind; tokens: EmailToken[]; siteUrl: string };

const subjects: Record<EmailKind, string> = {
  purchase: "Your DiskLens Pro license",
  trial: "Your DiskLens Pro trial",
  recover: "Your DiskLens licenses",
};

export function licenseEmail(props: Props) {
  return { subject: subjects[props.kind], react: <LicenseEmail {...props} /> };
}

const font = "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', Arial, sans-serif";

const styles = {
  body: { backgroundColor: "#f5f5f7", fontFamily: font, margin: 0, padding: "24px 0" },
  card: { backgroundColor: "#ffffff", borderRadius: 12, padding: "32px", maxWidth: 520 },
  h1: { color: "#1d1d1f", fontSize: 22, fontWeight: 600, margin: "0 0 16px" },
  text: { color: "#1d1d1f", fontSize: 15, lineHeight: "22px", margin: "0 0 12px" },
  muted: { color: "#6e6e73", fontSize: 13, lineHeight: "20px", margin: "0 0 8px" },
  token: {
    backgroundColor: "#f5f5f7",
    borderRadius: 8,
    color: "#1d1d1f",
    fontFamily: "SFMono-Regular, Menlo, Consolas, monospace",
    fontSize: 12,
    lineHeight: "18px",
    margin: "8px 0 16px",
    padding: "12px",
    wordBreak: "break-all" as const,
  },
  button: {
    backgroundColor: "#0a84ff",
    borderRadius: 8,
    color: "#ffffff",
    fontSize: 15,
    fontWeight: 600,
    padding: "10px 18px",
    textDecoration: "none",
  },
};

const heading: Record<EmailKind, string> = {
  purchase: "Thanks for buying DiskLens Pro",
  trial: "Your 7-day DiskLens Pro trial",
  recover: "Your DiskLens licenses",
};

const date = (seconds: number) => new Date(seconds * 1000).toISOString().slice(0, 10);

export default function LicenseEmail({ kind, tokens, siteUrl }: Props) {
  return (
    <Html lang="en">
      <Head />
      <Preview>{heading[kind]}</Preview>
      <Body style={styles.body}>
        <Container style={styles.card}>
          <Heading style={styles.h1}>{heading[kind]}</Heading>
          {tokens.map((t) => (
            <Section key={t.token}>
              <Text style={styles.text}>
                {t.kind === "trial"
                  ? `Trial license${t.expires ? `, valid until ${date(t.expires)} (UTC)` : ""}`
                  : "Pro license"}
              </Text>
              <Text style={styles.token}>{t.token}</Text>
              <Button href={`${siteUrl}/activate#${t.token}`} style={styles.button}>
                Activate in DiskLens
              </Button>
            </Section>
          ))}
          <Text style={{ ...styles.text, marginTop: 24 }}>To activate by hand:</Text>
          <Text style={styles.muted}>
            1. Open DiskLens and choose Settings (⌘,).
            <br />
            2. Paste the license into the license field.
          </Text>
          {kind === "trial" && (
            <Text style={styles.muted}>
              When the trial ends, DiskLens goes back to the free version. Buy Pro at{" "}
              <Link href={`${siteUrl}/pricing`}>{siteUrl}/pricing</Link>.
            </Text>
          )}
          <Text style={styles.muted}>
            Lost this email? Request it again at{" "}
            <Link href={`${siteUrl}/recover`}>{siteUrl}/recover</Link>.
          </Text>
        </Container>
      </Body>
    </Html>
  );
}
