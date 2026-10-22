import { describe, expect, it } from "vitest";
import type { ViewNode } from "./bindings/ViewNode";
import { mock, truncate } from "./mockApi";
import {
	crumbs,
	hatchBackground,
	labelVisibility,
	layoutView,
	parentPath,
	sizeFontPx,
	tileColors,
} from "./treemap";

const node = (
	name: string,
	size: number,
	children: ViewNode[] = [],
	extra: Partial<ViewNode> = {},
): ViewNode => ({
	id: name,
	name,
	path: `/r/${name}`,
	size,
	category: "code_projects",
	tier: null,
	rule_id: null,
	kind: children.length ? "Dir" : "File",
	children,
	...extra,
});

const flat = (sizes: number[]) =>
	node(
		"r",
		sizes.reduce((a, b) => a + b, 0),
		sizes.map((s, i) => node(`n${i}`, s)),
	);

describe("layoutView", () => {
	it("areas are proportional to sizes without padding", () => {
		const rects = layoutView(flat([600, 300, 100]), 1000, 500, { gap: 0 });
		const total = rects.reduce((s, r) => s + r.w * r.h, 0);
		expect(total).toBeCloseTo(500_000, -3);
		const big = rects.find((r) => r.node.name === "n0")!;
		expect((big.w * big.h) / total).toBeCloseTo(0.6, 1);
	});

	it("keeps every rect inside the bounds and rects apart", () => {
		const rects = layoutView(flat([50, 40, 30, 20, 10, 5]), 800, 600);
		for (const r of rects) {
			expect(r.x).toBeGreaterThanOrEqual(0);
			expect(r.y).toBeGreaterThanOrEqual(0);
			expect(r.x + r.w).toBeLessThanOrEqual(800);
			expect(r.y + r.h).toBeLessThanOrEqual(600);
		}
		for (const a of rects) {
			for (const b of rects) {
				if (a === b) continue;
				const overlap =
					a.x < b.x + b.w &&
					b.x < a.x + a.w &&
					a.y < b.y + b.h &&
					b.y < a.y + a.h;
				expect(overlap).toBe(false);
			}
		}
	});

	it("nests tiles inside group rects below the header", () => {
		const view = node("r", 100, [
			node("g", 100, [node("a", 60), node("b", 40)], { kind: "Dir" }),
		]);
		const rects = layoutView(view, 400, 300, { header: 30, groupPad: 8 });
		const g = rects.find((r) => r.node.name === "g")!;
		const a = rects.find((r) => r.node.name === "a")!;
		expect(g.group).toBe(true);
		expect(a.parentId).toBe("g");
		expect(a.y).toBeGreaterThanOrEqual(g.y + 30);
		expect(a.x).toBeGreaterThanOrEqual(g.x + 8);
	});

	it("lays out the mock view", () => {
		const rects = layoutView(truncate(mock.view, 2), 760, 560);
		expect(rects.filter((r) => r.depth === 1)).toHaveLength(
			mock.view.children.length,
		);
		expect(rects.length).toBeGreaterThan(30);
	});

	it("returns nothing for empty views or sizes", () => {
		expect(layoutView(node("r", 0), 100, 100)).toEqual([]);
		expect(layoutView(flat([1]), 0, 100)).toEqual([]);
	});

	it("lays out 400 tiles in under 16 ms", () => {
		const groups = Array.from({ length: 20 }, (_, g) =>
			node(
				`g${g}`,
				1000,
				Array.from({ length: 20 }, (_, t) => node(`t${g}-${t}`, 50 + t)),
				{ kind: "Dir" },
			),
		);
		const view = node("r", 20_000, groups);
		layoutView(view, 760, 560); // warm up
		const runs = Array.from({ length: 15 }, () => {
			const t0 = performance.now();
			layoutView(view, 760, 560);
			return performance.now() - t0;
		}).sort((a, b) => a - b);
		expect(runs[7]).toBeLessThan(16); // median
	});
});

describe("labels", () => {
	it("hides name below 60x28 and size below 80x44", () => {
		expect(labelVisibility(59, 40)).toEqual({ name: false, size: false });
		expect(labelVisibility(60, 28)).toEqual({ name: true, size: false });
		expect(labelVisibility(80, 43)).toEqual({ name: true, size: false });
		expect(labelVisibility(80, 44)).toEqual({ name: true, size: true });
	});

	it("scales the size font from 13 to 22 px with area", () => {
		expect(sizeFontPx(80 * 44)).toBe(13);
		expect(sizeFontPx(1)).toBe(13);
		expect(sizeFontPx(1e7)).toBe(22);
		const mid = sizeFontPx(120 * 120);
		expect(mid).toBeGreaterThan(13);
		expect(mid).toBeLessThan(22);
		expect(sizeFontPx(150 * 150)).toBeGreaterThanOrEqual(mid);
	});
});

describe("tileColors", () => {
	const plain = node("p", 1, [], { category: "ai_models" });
	const safe = node("s", 1, [], { category: "package_caches", tier: "Safe" });
	const review = node("v", 1, [], { category: "mobile_sdks", tier: "Review" });

	it("category view colors by category and hatches matches", () => {
		expect(tileColors(plain, "Category")).toEqual({
			fill: "var(--cat-ai_models-fill)",
			border: "var(--cat-ai_models-border)",
			hatch: null,
		});
		const c = tileColors(safe, "Category");
		expect(c.fill).toBe("var(--cat-package_caches-fill)");
		expect(c.border).toBe("var(--safe)");
		expect(c.hatch).toBe("var(--safe)");
	});

	it("cleanup view colors by tier, neutral otherwise, still hatched", () => {
		expect(tileColors(plain, "Cleanup").fill).toBe("var(--cat-dev_tools-fill)");
		expect(tileColors(plain, "Cleanup").hatch).toBeNull();
		expect(tileColors(safe, "Cleanup").fill).toBe("var(--tier-safe-fill)");
		expect(tileColors(review, "Cleanup")).toMatchObject({
			border: "var(--review)",
			hatch: "var(--review)",
		});
	});

	it("hatch is 45 degree stripes, 2px every 7px", () => {
		expect(hatchBackground("red")).toBe(
			"repeating-linear-gradient(45deg, red 0 2px, transparent 2px 7px)",
		);
	});
});

describe("breadcrumbs", () => {
	it("builds crumbs from the root", () => {
		expect(crumbs("/Users/m", "/Users/m")).toEqual([
			{ label: "~ Home", path: "/Users/m" },
		]);
		expect(crumbs("/Users/m", "/Users/m/Desktop/atlas")).toEqual([
			{ label: "~ Home", path: "/Users/m" },
			{ label: "Desktop", path: "/Users/m/Desktop" },
			{ label: "atlas", path: "/Users/m/Desktop/atlas" },
		]);
		expect(crumbs("~", "~/Desktop").map((c) => c.label)).toEqual([
			"~ Home",
			"Desktop",
		]);
	});

	it("finds parent paths", () => {
		expect(parentPath("/Users/m/Desktop")).toBe("/Users/m");
		expect(parentPath("/Users")).toBe("/");
	});
});
