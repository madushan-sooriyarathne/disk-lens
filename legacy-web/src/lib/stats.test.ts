import { describe, expect, it } from "vitest";
import type { Recommendation } from "./bindings/Recommendation";
import type { SnapshotMeta } from "./bindings/SnapshotMeta";
import { mock } from "./mockApi";
import { computeStats } from "./stats";

const GB = 1e9;
const meta = (total: number, available: number, scanned: number) =>
	({
		...mock.meta,
		volume: { total, available },
		scanned_size: scanned,
	}) as SnapshotMeta;

const rec = (
	row: string,
	tier: "Safe" | "Review",
	items: [string, number, boolean?][],
): Recommendation => ({
	row_id: row,
	rule_id: row,
	label: row,
	paths_hint: "",
	tier,
	size: items.reduce((s, [, n]) => s + n, 0),
	size_is_estimate: false,
	items: items.map(([id, size, blocked]) => ({
		item_id: id,
		path: `/${id}`,
		name: id,
		size,
		status: blocked ? { Blocked: { reason: "busy" } } : "Ready",
		method: "Delete",
	})),
});

describe("computeStats", () => {
	it("matches the mockup numbers", () => {
		const selected = new Set(
			mock.recommendations
				.filter((r) => mock.preselected.includes(r.row_id))
				.flatMap((r) => r.items.map((i) => i.item_id)),
		);
		const s = computeStats(mock.meta, mock.recommendations, selected);
		expect(s.safe.size / GB).toBeCloseTo(41.7, 1);
		expect(s.safe.count).toBe(5);
		expect(s.review.size / GB).toBeCloseTo(56.0, 1);
		expect(s.review.count).toBe(7);
		expect(s.selected / GB).toBeCloseTo(52.5, 1);
		expect(s.selectedRows).toBe(5);
		expect(s.freeAfterCleanup / GB).toBeCloseTo(92, 0);
		expect(s.notScanned / GB).toBeCloseTo(288, 0);
	});

	it("selecting nothing leaves free space unchanged", () => {
		const s = computeStats(mock.meta, mock.recommendations, new Set());
		expect(s.selected).toBe(0);
		expect(s.freeAfterCleanup).toBe(mock.meta.volume.available);
	});

	it("counts only ready items and rows that have one", () => {
		const recs = [
			rec("a", "Safe", [
				["a1", 5 * GB],
				["a2", 3 * GB, true],
			]),
			rec("b", "Safe", [["b1", 2 * GB, true]]),
		];
		const s = computeStats(meta(100 * GB, 40 * GB, 20 * GB), recs, new Set());
		expect(s.safe).toEqual({ size: 5 * GB, count: 1 });
	});

	it("ignores a selected blocked item", () => {
		const recs = [rec("a", "Safe", [["a1", 5 * GB, true]])];
		const s = computeStats(
			meta(100 * GB, 40 * GB, 20 * GB),
			recs,
			new Set(["a1"]),
		);
		expect(s.selected).toBe(0);
		expect(s.selectedRows).toBe(0);
	});

	it("clamps not-scanned at zero", () => {
		const s = computeStats(meta(100 * GB, 40 * GB, 90 * GB), [], new Set());
		expect(s.notScanned).toBe(0);
	});
});
