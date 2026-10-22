import { useEffect, useRef, useState } from "react";
import type { ViewMode } from "../../lib/bindings/ViewMode";
import { useView } from "../../store/view";
import { Breadcrumb } from "./Breadcrumb";
import { Legend } from "./Legend";
import { Treemap } from "./Treemap";

const MODES: ViewMode[] = ["Category", "Cleanup"];

function ModeToggle() {
	const { mode, setMode } = useView();
	return (
		<div className="flex rounded-lg bg-[var(--bg)] p-0.5 text-[13px]">
			{MODES.map((m) => (
				<button
					key={m}
					type="button"
					aria-pressed={mode === m}
					onClick={() => void setMode(m)}
					className={`rounded-md px-3 py-1 ${mode === m ? "border border-[var(--card-border)] bg-[var(--card)] font-medium shadow-sm" : "text-[var(--muted)]"}`}
				>
					{m}
				</button>
			))}
		</div>
	);
}

/** Measures the treemap area, relaying out at most once per frame while resizing. */
function useSize() {
	const ref = useRef<HTMLDivElement>(null);
	const [size, setSize] = useState({ width: 0, height: 0 });
	useEffect(() => {
		const el = ref.current;
		if (!el) return;
		let frame = 0;
		const ro = new ResizeObserver(([entry]) => {
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(() => {
				const { width, height } = entry.contentRect;
				setSize({ width: Math.floor(width), height: Math.floor(height) });
			});
		});
		ro.observe(el);
		return () => {
			cancelAnimationFrame(frame);
			ro.disconnect();
		};
	}, []);
	return { ref, ...size };
}

export function Explorer() {
	const { view, mode } = useView();
	const { ref, width, height } = useSize();
	return (
		<section
			aria-label="Explorer"
			className="flex min-w-0 flex-1 flex-col rounded-xl border border-[var(--card-border)] bg-[var(--card)]"
		>
			<header className="flex items-start justify-between px-5 pt-5">
				<div>
					<h2 className="text-[15px] font-semibold">Explorer</h2>
					<p className="mt-0.5 text-[13px] text-[var(--muted)]">
						Your home folder, sized by space used. Click a tile to open it.
					</p>
				</div>
				<ModeToggle />
			</header>
			<div className="mt-3 border-t border-[var(--divider)]">
				<Breadcrumb />
			</div>
			<div ref={ref} className="min-h-0 flex-1 px-5">
				{view && width > 0 && (
					<Treemap view={view} mode={mode} width={width} height={height} />
				)}
			</div>
			<Legend />
		</section>
	);
}
