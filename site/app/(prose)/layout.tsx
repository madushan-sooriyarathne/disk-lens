import type { ReactNode } from "react";

export default function ProseLayout({ children }: { children: ReactNode }) {
  return <article className="wrap prose prose-page">{children}</article>;
}
