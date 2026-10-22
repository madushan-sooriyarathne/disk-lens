import { describe, expect, it } from "vitest";
import type { Recommendation } from "./bindings/Recommendation";
import {
	blockedReason,
	pruneSelection,
	rowState,
	selectSafe,
	toggleRow,
} from "./selection";

const rec = (
	row: string,
	tier: "Safe" | "Review",
	items: [string, string?][],
): Recommendation => ({
	row_id: row,
	rule_id: row,
	label: row,
	paths_hint: "",
	tier,
	size: 0,
	size_is_estimate: false,
	items: items.map(([id, blocked]) => ({
		item_id: id,
		path: `/${id}`,
		name: id,
		size: 1,
		status: blocked ? { Blocked: { reason: blocked } } : "Ready",
		method: "Delete",
	})),
});

const npm = rec("npm", "Safe", [["a"], ["b"], ["c", "Changed recently"]]);
const xcode = rec("xcode", "Safe", [["x", "Quit Xcode to clean"]]);
const trash = rec("trash", "Review", [["t"]]);

describe("rowState", () => {
	it("counts ready items only", () => {
		expect(rowState(npm, new Set())).toBe("none");
		expect(rowState(npm, new Set(["a"]))).toBe("some");
		expect(rowState(npm, new Set(["a", "b"]))).toBe("all");
		expect(rowState(xcode, new Set())).toBe("none");
	});
});

describe("toggleRow", () => {
	it("selects all ready items, then clears them", () => {
		const once = toggleRow(npm, new Set(["t"]));
		expect([...once].sort()).toEqual(["a", "b", "t"]);
		expect([...toggleRow(npm, once)]).toEqual(["t"]);
	});

	it("a partly selected row becomes fully selected", () => {
		expect([...toggleRow(npm, new Set(["a"]))].sort()).toEqual(["a", "b"]);
	});

	it("never selects blocked items", () => {
		expect(toggleRow(xcode, new Set()).size).toBe(0);
	});
});

describe("selectSafe", () => {
	it("adds every ready safe item and keeps review choices", () => {
		const s = selectSafe([npm, xcode, trash], new Set(["t"]));
		expect([...s].sort()).toEqual(["a", "b", "t"]);
	});
});

describe("pruneSelection", () => {
	it("drops vanished and newly blocked items", () => {
		const s = pruneSelection([npm, xcode], new Set(["a", "c", "x", "gone"]));
		expect([...s]).toEqual(["a"]);
	});
});

describe("blockedReason", () => {
	it("only for rows with nothing ready", () => {
		expect(blockedReason(xcode)).toBe("Quit Xcode to clean");
		expect(blockedReason(npm)).toBeNull();
	});
});
