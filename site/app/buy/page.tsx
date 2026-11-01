import { EmailForm } from "@/components/EmailForm";
import { formatPrice, getPrice } from "@/lib/price";
import { meta } from "@/lib/seo";

export const metadata = meta(
  "Buy DiskLens Pro",
  "Buy DiskLens Pro with a one-time payment through payments.lk. Your license key appears right after payment and is also emailed.",
  "/buy",
);
export const revalidate = 300;

export default async function Buy() {
  const price = await getPrice();
  return (
    <div className="wrap page">
      <h1>Buy DiskLens Pro</h1>
      <p className="lead">
        {price ? `${formatPrice(price)}, one time, for every ${price.maxMajor}.x update.` : "See checkout for the price."}{" "}
        Enter your email and you will pay on the payments.lk checkout page.
      </p>
      <EmailForm action="checkout" label="Continue to payment" busyLabel="Opening checkout…" />
      <p className="muted">
        Your key is shown after payment and emailed to this address. Prices are in Sri Lankan rupees.
      </p>
    </div>
  );
}
