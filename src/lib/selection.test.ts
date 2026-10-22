import { describe, expect, it } from "vitest";
import type { Recommendation } from "./bindings/Recommendation";
import type { ViewNode } from "./bindings/ViewNode";
import {
	blockedReason,
	pruneSelection,
	rowForNode,
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

describe("rowForNode", () => {
	const wt = (project: string, id: string): Recommendation => ({
		...rec(`wt:${project}`, "Review", [[id]]),
		rule_id: "wt",
	});
	const atlas = wt("atlas", "home/atlas/.claude/worktrees/a");
	const pebble = wt("pebble", "home/pebble/.claude/worktrees/b");
	const tile = (path: string, rule: string | null): ViewNode => ({
		id: path,
		name: path,
		path,
		size: 1,
		category: "code_projects",
		tier: rule ? "Review" : null,
		rule_id: rule,
		kind: "Dir",
		children: [],
	});

	it("picks the split row whose item contains or is inside the tile", () => {
		const recs = [atlas, pebble];
		expect(
			rowForNode(recs, tile("/home/pebble/.claude/worktrees/b", "wt")),
		).toBe(pebble);
		expect(
			rowForNode(recs, tile("/home/pebble/.claude/worktrees/b/src", "wt")),
		).toBe(pebble);
		expect(rowForNode(recs, tile("/home/atlas/.claude/worktrees", "wt"))).toBe(
			atlas,
		);
	});

	it("ignores unmatched tiles and prefix look-alikes", () => {
		expect(rowForNode([atlas], tile("/home/atlas", null))).toBeUndefined();
		expect(
			rowForNode(
				[atlas, pebble],
				tile("/home/pebble/.claude/worktrees/bb", "wt"),
			),
		).toBe(atlas);
	});
});
