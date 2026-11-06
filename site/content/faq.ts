export const faq: { q: string; a: string }[] = [
  {
    q: "How does licensing work without an account?",
    a: "A DiskLens license is a signed token. The app checks the signature offline against a public key built into the app, so it never contacts a license server and never needs a login. Paste the token in Settings, or open the link we give you.",
  },
  {
    q: "What does a major upgrade mean?",
    a: "A license unlocks Pro up to a major version. A Pro 1.x license covers every 1.x update. A new major version, such as 2.0, is a paid upgrade, and your existing license keeps working on the version you bought.",
  },
  {
    q: "Can I get a refund?",
    a: "A refund marks your order as refunded. Because licenses are verified offline, a token cannot be revoked remotely. The terms page has the details.",
  },
  {
    q: "Does DiskLens collect data?",
    a: "No telemetry. The app checks once per launch for a newer version by reading a file from GitHub Releases. It never uploads your folders, file names or scan results. Scans read sizes and names on your Mac and stay there.",
  },
  {
    q: "Why is the price in Sri Lankan rupees?",
    a: "Payments go through payments.lk, which supports LKR only. If your card is in another currency, your card issuer converts the amount at its own rate.",
  },
  {
    q: "Can DiskLens delete my projects?",
    a: "No. A cleanup can only run items that DiskLens's rules found in the current scan, and each item is checked again just before it runs. A folder that contains a .git directory is never deleted by path, and a repository with uncommitted or unpushed work is flagged before you can clean its build output.",
  },
  {
    q: "Does the free version limit how much I can clean?",
    a: "No. Every cleanup feature is free. Pro adds new capabilities on top. Safety checks run the same with and without a license.",
  },
  {
    q: "Which Macs does it support?",
    a: "macOS 13 or later, on Apple Silicon and Intel.",
  },
];
