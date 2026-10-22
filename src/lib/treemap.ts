import { hierarchy, treemap, treemapSquarify } from "d3-hierarchy";
import type { ViewMode } from "./bindings/ViewMode";
import type { ViewNode } from "./bindings/ViewNode";

export const GROUP_HEADER = 30;
export const GROUP_PAD = 8;
export const TILE_GAP = 6;

export interface Rect {
	node: ViewNode;
	x: number;
	y: number;
	w: number;
	h: number;
	/** 1 = top-level group or tile, 2 = tile inside a group. */
	depth: 1 | 2;
	/** True when the rect is a group that draws a header and holds tiles. */
	group: boolean;
	parentId: string | null;
}

export interface LayoutOptions {
	header?: number;
	groupPad?: number;
	gap?: number;
}

/**
 * Squarified layout of a view (root plus two levels). Pure: called only when the
 * view or the container size changes, never on hover.
 */
export function layoutView(
	view: ViewNode,
	width: number,
	height: number,
	{
		header = GROUP_HEADER,
		groupPad = GROUP_PAD,
		gap = TILE_GAP,
	}: LayoutOptions = {},
): Rect[] {
	if (view.children.length === 0 || width <= 0 || height <= 0) return [];
	const root = hierarchy(view, (n) => n.children)
		.sum((n) => (n.children.length === 0 ? n.size : 0))
		.sort(
			(a, b) =>
				(b.value ?? 0) - (a.value ?? 0) ||
				a.data.name.localeCompare(b.data.name),
		);
	const isGroup = (d: { depth: number; children?: unknown }) =>
		d.depth === 1 && !!d.children;
	treemap<ViewNode>()
		.tile(treemapSquarify)
		.size([width, height])
		.paddingInner(gap)
		.paddingTop((d) => (isGroup(d) ? header : 0))
		.paddingLeft((d) => (isGroup(d) ? groupPad : 0))
		.paddingRight((d) => (isGroup(d) ? groupPad : 0))
		.paddingBottom((d) => (isGroup(d) ? groupPad : 0))
		.round(true)(root);

	const out: Rect[] = [];
	for (const d of root.descendants()) {
		if (d.depth === 0) continue;
		const r = d as typeof d & {
			x0: number;
			y0: number;
			x1: number;
			y1: number;
		};
		out.push({
			node: d.data,
			x: r.x0,
			y: r.y0,
			w: r.x1 - r.x0,
			h: r.y1 - r.y0,
			depth: d.depth as 1 | 2,
			group: isGroup(d),
			parentId: d.parent && d.parent.depth > 0 ? d.parent.data.id : null,
		});
	}
	return out;
}

const NAME_BAND = 28; // top inset plus one line of name
const SIZE_BAND = 21; // 13 px size label plus its bottom inset

/**
 * The name hides below 60x28 px and the size below 80x44 px. The size also hides
 * when it would overlap the name (name band + size band must fit in the height).
 */
export function labelVisibility(w: number, h: number) {
	const name = w >= 60 && h >= NAME_BAND;
	const room = name ? h >= NAME_BAND + SIZE_BAND : true;
	return { name, size: w >= 80 && h >= 44 && room };
}

const MIN_FONT = 13;
const MAX_FONT = 22;
const SMALL_AREA = 80 * 44;
const LARGE_AREA = 200 * 200;

/** Size label font grows with tile area, from 13 to 22 px. */
export function sizeFontPx(area: number): number {
	const t =
		(Math.sqrt(area) - Math.sqrt(SMALL_AREA)) /
		(Math.sqrt(LARGE_AREA) - Math.sqrt(SMALL_AREA));
	return Math.round(
		MIN_FONT + Math.min(1, Math.max(0, t)) * (MAX_FONT - MIN_FONT),
	);
}

export interface TileColors {
	fill: string;
	border: string;
	/** Tier color for the hatch overlay, when a recommendation matches. */
	hatch: string | null;
}

const TIER_COLOR = { Safe: "var(--safe)", Review: "var(--review)" } as const;

export function tileColors(node: ViewNode, mode: ViewMode): TileColors {
	const hatch = node.tier ? TIER_COLOR[node.tier] : null;
	if (mode === "Cleanup") {
		const tint = node.tier
			? `var(--tier-${node.tier.toLowerCase()}-fill)`
			: null;
		return {
			fill: tint ?? "var(--cat-dev_tools-fill)",
			border: hatch ?? "var(--cat-dev_tools-border)",
			hatch,
		};
	}
	return {
		fill: `var(--cat-${node.category}-fill)`,
		border: hatch ?? `var(--cat-${node.category}-border)`,
		hatch,
	};
}

/** Diagonal stripes: 45 degrees, 2 px lines every 7 px, at 35% opacity (via the layer). */
export function hatchBackground(color: string): string {
	return `repeating-linear-gradient(45deg, ${color} 0 2px, transparent 2px 7px)`;
}

export const HATCH_OPACITY = 0.35;

export interface Crumb {
	label: string;
	path: string;
}

export const ROOT_LABEL = "~ Home";

export function parentPath(path: string): string {
	return path.slice(0, path.lastIndexOf("/")) || "/";
}

/** Breadcrumb from the scan root down to `path`; the root reads "~ Home". */
export function crumbs(rootPath: string, path: string): Crumb[] {
	const out: Crumb[] = [{ label: ROOT_LABEL, path: rootPath }];
	if (path === rootPath || !path.startsWith(`${rootPath}/`)) return out;
	let acc = rootPath;
	for (const part of path.slice(rootPath.length + 1).split("/")) {
		acc = `${acc}/${part}`;
		out.push({ label: part, path: acc });
	}
	return out;
}
