import type { Tier } from "../../lib/bindings/Tier";
import { formatBytes } from "../../lib/format";
import { computeStats } from "../../lib/stats";
import { useScan } from "../../store/scan";
import { useSelection } from "../../store/selection";
import { DetailDrawer } from "./DetailDrawer";
import { Footer } from "./Footer";
import { Row } from "./Row";

const SECTIONS: { tier: Tier; label: string; color: string; fill: string }[] = [
	{
		tier: "Safe",
		label: "Safe to remove",
		color: "var(--safe)",
		fill: "var(--tier-safe-fill)",
	},
	{
		tier: "Review",
		label: "Review first",
		color: "var(--review)",
		fill: "var(--tier-review-fill)",
	},
];

export function Panel() {
	const meta = useScan((s) => s.meta);
	const recs = useSelection((s) => s.recommendations);
	const focused = useSelection((s) =>
		s.recommendations.find((r) => r.row_id === s.focusedRow),
	);
	if (!meta) return null;
	const stats = computeStats(meta, recs, new Set());
	const total = { Safe: stats.safe.size, Review: stats.review.size };
	return (
		<section
			aria-label="Recommendations"
			className="relative flex w-[390px] shrink-0 flex-col overflow-hidden rounded-xl border border-[var(--card-border)] bg-[var(--card)]"
		>
			<header className="px-5 pt-5 pb-3">
				<h2 className="text-[15px] font-semibold">Recommendations</h2>
				<p className="mt-0.5 text-[13px] text-[var(--muted)]">
					Select items to build a cleanup script
				</p>
			</header>
			<div className="min-h-0 flex-1 overflow-y-auto border-t border-[var(--divider)]">
				{recs.length === 0 && (
					<p className="px-5 py-8 text-center text-[13px] text-[var(--muted)]">
						Nothing to clean up in this scan.
					</p>
				)}
				{SECTIONS.map(({ tier, label, color, fill }) => {
					const rows = recs.filter((r) => r.tier === tier);
					if (rows.length === 0) return null;
					return (
						<div key={tier}>
							<div className="flex items-center justify-between border-b border-[var(--divider)] px-5 py-2">
								<span
									className="rounded-md px-2 py-0.5 text-[12px] font-medium"
									style={{ color, background: fill }}
								>
									{label}
								</span>
								<span className="font-mono text-[13px] text-[var(--muted)]">
									{formatBytes(total[tier])}
								</span>
							</div>
							<ul>
								{rows.map((r) => (
									<Row key={r.row_id} row={r} />
								))}
							</ul>
						</div>
					);
				})}
			</div>
			<Footer />
			{focused && <DetailDrawer row={focused} />}
		</section>
	);
}
