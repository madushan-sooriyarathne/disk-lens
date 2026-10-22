import type { Recommendation } from "./bindings/Recommendation";

export type RowState = "none" | "some" | "all";

/** Items that can be selected: blocked items never can. */
export function readyIds(row: Recommendation): string[] {
	return row.items.filter((i) => i.status === "Ready").map((i) => i.item_id);
}

/** Tri-state of a row's checkbox over its ready items. */
export function rowState(
	row: Recommendation,
	selected: ReadonlySet<string>,
): RowState {
	const ids = readyIds(row);
	const n = ids.filter((id) => selected.has(id)).length;
	if (n === 0) return "none";
	return n === ids.length ? "all" : "some";
}

/** A fully selected row clears; otherwise all its ready items are selected. */
export function toggleRow(
	row: Recommendation,
	selected: ReadonlySet<string>,
): Set<string> {
	const next = new Set(selected);
	const ids = readyIds(row);
	if (rowState(row, selected) === "all") {
		for (const id of ids) next.delete(id);
	} else {
		for (const id of ids) next.add(id);
	}
	return next;
}

/** Adds every ready safe-tier item; review selections stay as they are. */
export function selectSafe(
	recs: Recommendation[],
	selected: ReadonlySet<string>,
): Set<string> {
	const next = new Set(selected);
	for (const r of recs) {
		if (r.tier === "Safe") for (const id of readyIds(r)) next.add(id);
	}
	return next;
}

/** Drops ids that vanished or became blocked, e.g. after a focus refresh. */
export function pruneSelection(
	recs: Recommendation[],
	selected: ReadonlySet<string>,
): Set<string> {
	const ready = new Set(recs.flatMap(readyIds));
	return new Set([...selected].filter((id) => ready.has(id)));
}

/** The reason shown on a row whose items are all blocked, else null. */
export function blockedReason(row: Recommendation): string | null {
	if (readyIds(row).length > 0) return null;
	const first = row.items.find((i) => i.status !== "Ready");
	return first && typeof first.status === "object"
		? first.status.Blocked.reason
		: null;
}
