import { formatBytes } from "../../lib/format";
import { computeStats } from "../../lib/stats";
import { useScan } from "../../store/scan";
import { useSelection } from "../../store/selection";

const secondary =
	"rounded-lg border border-[var(--card-border)] px-3 py-1.5 text-[13px] font-medium hover:bg-[var(--bg)] disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent";

export function Footer() {
	const meta = useScan((s) => s.meta);
	const recs = useSelection((s) => s.recommendations);
	const selected = useSelection((s) => s.selected);
	if (!meta) return null;
	const s = computeStats(meta, recs, selected);
	return (
		<footer className="border-t border-[var(--divider)] px-5 py-4">
			<div className="text-[13px] text-[var(--muted)]">
				<span className="font-mono font-medium text-[var(--text)]">
					{formatBytes(s.selected)}
				</span>{" "}
				selected · {s.selectedRows} {s.selectedRows === 1 ? "item" : "items"}
			</div>
			<div className="mt-3 flex gap-2">
				<button
					type="button"
					className={secondary}
					onClick={() => useSelection.getState().selectSafe()}
				>
					Select safe
				</button>
				{/* Script export and cleanup arrive with the cleanup engine (Phase 4). */}
				<button
					type="button"
					className={secondary}
					disabled
					title="Available once the cleanup engine ships"
				>
					Copy script
				</button>
				<button
					type="button"
					className="ml-auto rounded-lg bg-[var(--text)] px-3 py-1.5 text-[13px] font-medium text-white disabled:cursor-not-allowed disabled:opacity-40"
					disabled
					title="Available once the cleanup engine ships"
				>
					Clean selected
				</button>
			</div>
		</footer>
	);
}
