import { useEffect, useRef } from "react";
import { categoryLabel } from "../../lib/categories";
import { formatBytes } from "../../lib/format";
import type { Rect } from "../../lib/treemap";
import { useSelection } from "../../store/selection";
import { useView } from "../../store/view";

/** Follows the pointer by writing styles directly, so moving never re-renders React. */
export function Tooltip({ rects }: { rects: Map<string, Rect> }) {
	const ref = useRef<HTMLDivElement>(null);
	const id = useView((s) => s.hoveredId);
	const recs = useSelection((s) => s.recommendations);

	useEffect(() => {
		const move = (e: PointerEvent) => {
			const el = ref.current;
			if (!el) return;
			const pad = 14;
			const x = Math.min(
				e.clientX + pad,
				window.innerWidth - el.offsetWidth - 8,
			);
			const y = Math.min(
				e.clientY + pad,
				window.innerHeight - el.offsetHeight - 8,
			);
			el.style.transform = `translate(${x}px, ${y}px)`;
		};
		window.addEventListener("pointermove", move);
		return () => window.removeEventListener("pointermove", move);
	}, []);

	const node = id ? rects.get(id)?.node : null;
	if (!node) return null;
	const rule = node.rule_id
		? recs.find((r) => r.rule_id === node.rule_id)?.label
		: null;
	return (
		<div
			ref={ref}
			role="tooltip"
			className="pointer-events-none fixed left-0 top-0 z-50 max-w-80 rounded-lg border border-[var(--card-border)] bg-[var(--card)] px-3 py-2 text-xs shadow-lg"
		>
			<div className="break-all font-mono text-[11px] text-[var(--muted)]">
				{node.path}
			</div>
			<div className="mt-1 flex items-baseline gap-2">
				<span className="font-mono text-sm font-semibold">
					{formatBytes(node.size)}
				</span>
				<span className="text-[var(--muted)]">
					{categoryLabel(node.category)}
				</span>
			</div>
			{rule && <div className="mt-1">{rule}</div>}
		</div>
	);
}
