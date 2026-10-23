import { CATEGORIES } from "../../lib/categories";

export function Legend() {
	return (
		<ul className="flex flex-wrap gap-x-5 gap-y-1 px-5 pb-4 pt-3 text-[13px] text-[var(--muted)]">
			{CATEGORIES.map((c) => (
				<li key={c.id} className="flex items-center gap-1.5">
					<span
						aria-hidden
						className="size-2.5 rounded-[3px]"
						style={{ background: `var(--cat-${c.id}-border)` }}
					/>
					{c.label}
				</li>
			))}
		</ul>
	);
}
