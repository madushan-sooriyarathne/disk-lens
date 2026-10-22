import { ChevronRight } from "lucide-react";
import { useEffect, useRef } from "react";
import type { Recommendation } from "../../lib/bindings/Recommendation";
import { formatSize } from "../../lib/format";
import { blockedReason, rowState } from "../../lib/selection";
import { useSelection } from "../../store/selection";

export function Row({ row }: { row: Recommendation }) {
	const selected = useSelection((s) => s.selected);
	const state = rowState(row, selected);
	const reason = blockedReason(row);
	const box = useRef<HTMLInputElement>(null);
	useEffect(() => {
		if (box.current) box.current.indeterminate = state === "some";
	}, [state]);

	const paths = row.items.map((i) => i.path).join("\n");
	return (
		<li className="flex items-center gap-3 border-b border-[var(--divider)] px-5 py-3 last:border-b-0">
			<input
				ref={box}
				type="checkbox"
				aria-label={`Select ${row.label}`}
				checked={state === "all"}
				disabled={reason !== null}
				onChange={() => useSelection.getState().toggleRow(row.row_id)}
				className="size-[18px] shrink-0 accent-[var(--text)] disabled:opacity-40"
			/>
			<div className="min-w-0 flex-1">
				<div
					className={`truncate text-[15px] font-medium ${reason ? "text-[var(--muted)]" : ""}`}
				>
					{row.label}
				</div>
				{reason ? (
					<div className="truncate text-[12px] text-[var(--review)]">
						{reason}
					</div>
				) : (
					<div
						className="truncate font-mono text-[12px] text-[var(--muted)]"
						title={paths}
					>
						{row.paths_hint}
					</div>
				)}
			</div>
			<span className="shrink-0 font-mono text-[15px] font-medium">
				{formatSize(row.size, row.size_is_estimate)}
			</span>
			<button
				type="button"
				aria-label={`Details for ${row.label}`}
				onClick={() => useSelection.getState().focusRow(row.row_id)}
				className="-mr-2 shrink-0 rounded-md p-1 text-[var(--muted)] hover:bg-[var(--bg)] hover:text-[var(--text)]"
			>
				<ChevronRight className="size-4" />
			</button>
		</li>
	);
}
