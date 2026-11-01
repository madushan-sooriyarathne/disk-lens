import type { MDXComponents } from "mdx/types";
import Link from "next/link";

// Internal links go through next/link; everything else stays a plain element.
const components: MDXComponents = {
  a: ({ href = "", children, ...rest }) =>
    href.startsWith("/") ? (
      <Link href={href}>{children}</Link>
    ) : (
      <a href={href} {...rest}>
        {children}
      </a>
    ),
};

export function useMDXComponents(): MDXComponents {
  return components;
}
