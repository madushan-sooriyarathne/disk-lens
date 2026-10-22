import { useEffect } from "react";
import { api } from "../../lib/api";
import type { ViewNode } from "../../lib/bindings/ViewNode";
import { useSelection } from "../../store/selection";

export interface MenuState {
	x: number;
	y: number;
	node: ViewNode;
}

export function ContextMenu({
	menu,
	onClose,
}: {
	menu: MenuState;
	onClose: () => void;
}) {
	const recs = useSelection((s) => s.recommendations);
	const row = menu.node.rule_id
		? recs.find((r) => r.rule_id === menu.node.rule_id)
		: undefined;

	useEffect(() => {
		const close = () => onClose();
		window.addEventListener("pointerdown", close);
		window.addEventListener("blur", close);
		return () => {
			window.removeEventListener("pointerdown", close);
			window.removeEventListener("blur", close);
		};
	}, [onClose]);

	const item =
		"block w-full rounded-md px-3 py-1.5 text-left text-[13px] hover:bg-[var(--bg)]";
	return (
		<div
			role="menu"
			className="fixed z-50 min-w-48 rounded-lg border border-[var(--card-border)] bg-[var(--card)] p-1 shadow-lg"
			style={{ left: menu.x, top: menu.y }}
			onPointerDown={(e) => e.stopPropagation()}
		>
			{menu.node.kind !== "Other" && (
				<button
					type="button"
					role="menuitem"
					className={item}
					onClick={() => {
						void api.revealInFinder(menu.node.path);
						onClose();
					}}
				>
					Reveal in Finder
				</button>
			)}
			{row && (
				<button
					type="button"
					role="menuitem"
					className={item}
					onClick={() => {
						useSelection.getState().focusRow(row.row_id);
						onClose();
					}}
				>
					Show recommendation
				</button>
			)}
		</div>
	);
}
