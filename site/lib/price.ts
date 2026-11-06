import { SERVER_URL } from "./env";

export type Price = { amountCents: number; currency: string; maxMajor: number };

// Never throws: the build and the page must survive a server that is down.
export async function getPrice(): Promise<Price | null> {
  try {
    const res = await fetch(`${SERVER_URL}/price`, {
      next: { revalidate: 300 },
      signal: AbortSignal.timeout(5000),
    });
    return res.ok ? ((await res.json()) as Price) : null;
  } catch {
    return null;
  }
}

export function formatPrice({ amountCents, currency }: Price): string {
  return new Intl.NumberFormat("en-LK", { style: "currency", currency }).format(amountCents / 100);
}
