import { useCallback, useEffect, useMemo, useState } from "react";
import type { ViewMode } from "../../lib/bindings/ViewMode";
import type { ViewNode } from "../../lib/bindings/ViewNode";
import { layoutView } from "../../lib/treemap";
import { useView } from "../../store/view";
import { ContextMenu, type MenuState } from "./ContextMenu";
import { Tile } from "./Tile";
import { Tooltip } from "./Tooltip";

interface Props {
	view: ViewNode;
	mode: ViewMode;
	width: number;
	height: number;
}

export function Treemap({ view, mode, width, height }: Props) {
	const [menu, setMenu] = useState<MenuState | null>(null);

	// Layout depends only on the data and the size; hover never touches it.
	const rects = useMemo(() => {
		if (import.meta.env.DEV) {
			const w = window as unknown as { __layoutCalls?: number };
			w.__layoutCalls = (w.__layoutCalls ?? 0) + 1;
		}
		return layoutView(view, width, height);
	}, [view, width, height]);
	const byId = useMemo(
		() => new Map(rects.map((r) => [r.node.id, r])),
		[rects],
	);

	const closeMenu = useCallback(() => setMenu(null), []);
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.key !== "Escape") return;
			if (menu) setMenu(null);
			else void useView.getState().up();
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	}, [menu]);

	const nodeAt = (target: EventTarget) => {
		const id = (target as HTMLElement).closest<HTMLElement>("[data-id]")
			?.dataset.id;
		return id ? byId.get(id)?.node : undefined;
	};

	return (
		// biome-ignore lint/a11y/useSemanticElements: a positioned canvas of tiles, not a form group
		<div
			role="group"
			aria-label="Folder treemap"
			className="relative"
			style={{ width, height }}
			onKeyDown={(e) => {
				if (e.key !== "Enter" && e.key !== " ") return;
				const node = nodeAt(e.target);
				if (node?.kind === "Dir") {
					e.preventDefault();
					void useView.getState().navigate(node.path);
				}
			}}
			onPointerOver={(e) =>
				useView.getState().hover(nodeAt(e.target)?.id ?? null)
			}
			onPointerLeave={() => useView.getState().hover(null)}
			onClick={(e) => {
				const node = nodeAt(e.target);
				if (node?.kind === "Dir") void useView.getState().navigate(node.path);
			}}
			onContextMenu={(e) => {
				const node = nodeAt(e.target);
				if (!node) return;
				e.preventDefault();
				setMenu({ x: e.clientX, y: e.clientY, node });
			}}
		>
			{rects.map((r) => (
				<Tile key={r.node.id} rect={r} mode={mode} />
			))}
			<Tooltip rects={byId} />
			{menu && <ContextMenu menu={menu} onClose={closeMenu} />}
		</div>
	);
}
