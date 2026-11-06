import { COMING_LABEL, type Feature } from "@/content/features";

export function StatusBadge({ feature }: { feature: Feature }) {
  if (feature.status === "coming") return <span className="badge badge-review">{COMING_LABEL}</span>;
  return <span className="badge badge-safe">Available</span>;
}
