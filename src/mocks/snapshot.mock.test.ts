import { describe, expect, it } from "vitest";
import mock from "./snapshot.mock.json";

const GB = 1e9;
const sum = (tier: string) =>
	mock.recommendations
		.filter((r) => r.tier === tier)
		.reduce((s, r) => s + r.size, 0) / GB;

describe("snapshot.mock.json matches mockup numbers", () => {
	it("tier totals and counts", () => {
		expect(sum("Safe")).toBeCloseTo(41.7, 1);
		expect(sum("Review")).toBeCloseTo(56.0, 1);
		expect(mock.recommendations.filter((r) => r.tier === "Safe")).toHaveLength(
			5,
		);
		expect(
			mock.recommendations.filter((r) => r.tier === "Review"),
		).toHaveLength(7);
	});
	it("selection, free space and not-scanned", () => {
		const selected =
			mock.recommendations
				.filter((r) => mock.preselected.includes(r.row_id))
				.reduce((s, r) => s + r.size, 0) / GB;
		expect(selected).toBeCloseTo(52.5, 1);
		const { total, available } = mock.meta.volume;
		expect((available + selected * GB) / GB).toBeCloseTo(92, 0);
		expect((total - available - mock.meta.scanned_size) / GB).toBeCloseTo(
			288,
			0,
		);
		expect(mock.view.size / GB).toBeCloseTo(135, 1);
	});
});
