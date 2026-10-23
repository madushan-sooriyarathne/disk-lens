import { ArrowLeft, FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import type { ItemDetails } from "../../lib/bindings/ItemDetails";
import type { MethodView } from "../../lib/bindings/MethodView";
import type { Recommendation } from "../../lib/bindings/Recommendation";
import { formatBytes, formatSize } from "../../lib/format";
import { useScan } from "../../store/scan";
import { useSelection } from "../../store/selection";

function methodWords(m: MethodView): string {
	if (m === "Delete") return "Deleted permanently";
	if (m === "Trash") return "Moved to Trash";
	return `Runs \`${m.Command.display}\``;
}

/** `/Users/x/.npm` -> `~/.npm` under the scan root. */
function tilde(path: string, root: string): string {
	return path === root || path.startsWith(`${root}/`)
		? `~${path.slice(root.length)}`
		: path;
}

const close = () => useSelection.getState().focusRow(null);

function Section({
	title,
	children,
}: {
	title: string;
	children: React.ReactNode;
}) {
	return (
		<section className="mt-5">
			<h3 className="text-[12px] font-medium tracking-wide text-[var(--muted)] uppercase">
				{title}
			</h3>
			<div className="mt-1.5 text-[13px] leading-5">{children}</div>
		</section>
	);
}

/** Slides over the recommendations panel (PRD section 7.5). */
export function DetailDrawer({ row }: { row: Recommendation }) {
	const selected = useSelection((s) => s.selected);
	const root = useScan((s) => s.meta?.root ?? "");
	const [details, setDetails] = useState<ItemDetails | null>(null);
	const [error, setError] = useState<string | null>(null);
	// Item statuses come from the store row, which a focus refresh keeps current.
	useEffect(() => {
		let live = true;
		setError(null);
		api
			.getItemDetails(row.row_id)
			.then((d) => live && setDetails(d))
			.catch((e) => live && setError(String(e)));
		return () => {
			live = false;
		};
	}, [row.row_id]);

	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.key === "Escape") {
				e.stopPropagation();
				close();
			}
		};
		window.addEventListener("keydown", onKey, true);
		return () => window.removeEventListener("keydown", onKey, true);
	}, []);

	const safe = row.tier === "Safe";
	const methods = [...new Set(row.items.map((i) => methodWords(i.method)))];
	return (
		<div
			role="dialog"
			aria-label={`${row.label} details`}
			className="drawer absolute inset-0 z-10 flex flex-col bg-[var(--card)]"
		>
			<header className="flex items-start gap-2 border-b border-[var(--divider)] px-5 pt-5 pb-4">
				<button
					type="button"
					aria-label="Back to recommendations"
					onClick={close}
					className="-ml-1.5 rounded-md p-1 text-[var(--muted)] hover:bg-[var(--bg)] hover:text-[var(--text)]"
				>
					<ArrowLeft className="size-4" />
				</button>
				<div className="min-w-0 flex-1">
					<h2 className="text-[15px] font-semibold">{row.label}</h2>
					<div className="mt-1 flex items-center gap-2">
						<span
							className="rounded-md px-2 py-0.5 text-[12px] font-medium"
							style={{
								color: safe ? "var(--safe)" : "var(--review)",
								background: safe
									? "var(--tier-safe-fill)"
									: "var(--tier-review-fill)",
							}}
						>
							{safe ? "Safe to remove" : "Review first"}
						</span>
						<span className="font-mono text-[13px]">
							{formatSize(row.size, row.size_is_estimate)}
						</span>
					</div>
				</div>
			</header>
			<div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5">
				{error && (
					<p className="mt-4 text-[13px] text-[var(--review)]">{error}</p>
				)}
				{details && (
					<>
						<Section title="What it is">{details.description}</Section>
						<Section title="What happens if removed">
							{details.regenerates}
						</Section>
					</>
				)}
				<Section title="How it is removed">
					{methods.map((m) => (
						<div key={m}>{m}</div>
					))}
				</Section>
				<Section title="Items">
					<ul className="-mx-1">
						{row.items.map((i) => {
							const blocked =
								typeof i.status === "object" ? i.status.Blocked.reason : null;
							return (
								<li
									key={i.item_id}
									className="flex items-center gap-2.5 rounded-md px-1 py-1.5"
								>
									<input
										type="checkbox"
										aria-label={`Select ${i.name}`}
										checked={selected.has(i.item_id)}
										disabled={blocked !== null}
										onChange={() =>
											useSelection.getState().toggleItem(i.item_id)
										}
										className="size-4 shrink-0 accent-[var(--text)] disabled:opacity-40"
									/>
									<div className="min-w-0 flex-1">
										<div className="truncate">{i.name}</div>
										<div
											className={`truncate text-[12px] ${blocked ? "text-[var(--review)]" : "font-mono text-[var(--muted)]"}`}
											title={i.path}
										>
											{blocked ?? tilde(i.path, root)}
										</div>
									</div>
									<span className="shrink-0 font-mono text-[13px]">
										{formatSize(i.size, row.size_is_estimate)}
									</span>
									<button
										type="button"
										aria-label={`Reveal ${i.name} in Finder`}
										title="Reveal in Finder"
										onClick={() => void api.revealInFinder(i.path)}
										className="shrink-0 rounded-md p-1 text-[var(--muted)] hover:bg-[var(--bg)] hover:text-[var(--text)]"
									>
										<FolderOpen className="size-4" />
									</button>
								</li>
							);
						})}
					</ul>
				</Section>
				{details && details.largest_subfolders.length > 0 && (
					<Section title="Largest subfolders">
						<ul>
							{details.largest_subfolders.map((f) => (
								<li key={f.path} className="flex gap-2 py-0.5">
									<span
										className="min-w-0 flex-1 truncate font-mono text-[12px] text-[var(--muted)]"
										title={f.path}
									>
										{tilde(f.path, root)}
									</span>
									<span className="shrink-0 font-mono text-[12px]">
										{formatBytes(f.size)}
									</span>
								</li>
							))}
						</ul>
					</Section>
				)}
			</div>
		</div>
	);
}
