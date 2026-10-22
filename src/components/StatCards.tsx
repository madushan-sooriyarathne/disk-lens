import { formatBytes } from "../lib/format";
import { computeStats } from "../lib/stats";
import { useScan } from "../store/scan";
import { useSelection } from "../store/selection";

interface CardProps {
	label: string;
	dot?: string;
	value: string;
	secondary?: { text: string; green?: boolean } | null;
}

function Card({ label, dot, value, secondary }: CardProps) {
	return (
		<div className="flex-1 px-6 py-5">
			<div className="flex items-center gap-1.5 text-[13px] text-[var(--muted)]">
				{dot && (
					<span
						className="size-1.5 rounded-full"
						style={{ background: dot }}
						aria-hidden
					/>
				)}
				{label}
			</div>
			<div className="mt-1.5 flex items-baseline gap-2.5">
				<span className="font-mono text-[30px] font-semibold leading-9">
					{value}
				</span>
				{secondary && (
					<span
						className={`font-mono text-[13px] ${secondary.green ? "text-[var(--safe)]" : "text-[var(--muted)]"}`}
					>
						{secondary.text}
					</span>
				)}
			</div>
		</div>
	);
}

export function StatCards() {
	const meta = useScan((s) => s.meta);
	const recs = useSelection((s) => s.recommendations);
	const selected = useSelection((s) => s.selected);
	if (!meta) return null;
	const s = computeStats(meta, recs, selected);
	const items = (n: number) => `${n} ${n === 1 ? "item" : "items"}`;
	return (
		<section
			aria-label="Summary"
			className="flex divide-x divide-[var(--divider)] rounded-xl border border-[var(--card-border)] bg-[var(--card)]"
		>
			<Card
				label="Free after cleanup"
				value={formatBytes(s.freeAfterCleanup)}
				secondary={
					s.selected > 0
						? { text: `+${formatBytes(s.selected)}`, green: true }
						: null
				}
			/>
			<Card
				label="Safe to remove"
				dot="var(--safe)"
				value={formatBytes(s.safe.size)}
				secondary={{ text: items(s.safe.count) }}
			/>
			<Card
				label="Needs review"
				dot="var(--review)"
				value={formatBytes(s.review.size)}
				secondary={{ text: items(s.review.count) }}
			/>
			<Card
				label="Not scanned"
				value={formatBytes(s.notScanned)}
				secondary={{ text: "system & apps" }}
			/>
		</section>
	);
}
