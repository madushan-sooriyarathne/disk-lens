import type { Recommendation } from "./bindings/Recommendation";
import type { SnapshotMeta } from "./bindings/SnapshotMeta";
import type { Tier } from "./bindings/Tier";

export interface TierStat {
	size: number;
	/** Number of recommendation rows with at least one ready item. */
	count: number;
}

export interface Stats {
	available: number;
	selected: number;
	selectedRows: number;
	freeAfterCleanup: number;
	safe: TierStat;
	review: TierStat;
	notScanned: number;
}

const ready = (r: Recommendation) =>
	r.items.filter((i) => i.status === "Ready");

function tierStat(recs: Recommendation[], tier: Tier): TierStat {
	let size = 0;
	let count = 0;
	for (const r of recs) {
		if (r.tier !== tier) continue;
		const items = ready(r);
		if (items.length === 0) continue;
		count++;
		for (const i of items) size += i.size;
	}
	return { size, count };
}

/** Every path belongs to one recommendation, so totals never double count. */
export function computeStats(
	meta: SnapshotMeta,
	recs: Recommendation[],
	selected: ReadonlySet<string>,
): Stats {
	let selectedSize = 0;
	let selectedRows = 0;
	for (const r of recs) {
		const chosen = ready(r).filter((i) => selected.has(i.item_id));
		if (chosen.length === 0) continue;
		selectedRows++;
		for (const i of chosen) selectedSize += i.size;
	}
	const { total, available } = meta.volume;
	return {
		available,
		selected: selectedSize,
		selectedRows,
		freeAfterCleanup: available + selectedSize,
		safe: tierStat(recs, "Safe"),
		review: tierStat(recs, "Review"),
		notScanned: Math.max(0, total - available - meta.scanned_size),
	};
}
