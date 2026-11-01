import Link from "next/link";

export const metadata = { title: "Page not found", robots: { index: false } };

export default function NotFound() {
  return (
    <div className="wrap page">
      <h1>Page not found</h1>
      <p className="lead">That page doesn't exist. Try the docs or head back home.</p>
      <p>
        <Link className="btn btn-primary" href="/">
          Back to DiskLens
        </Link>
      </p>
    </div>
  );
}
