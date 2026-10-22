import raw from "../mocks/snapshot.mock.json";
import type { Api } from "./api";
import type { Recommendation } from "./bindings/Recommendation";
import type { ScanEvent } from "./bindings/ScanEvent";
import type { SnapshotMeta } from "./bindings/SnapshotMeta";
import type { ViewNode } from "./bindings/ViewNode";

export interface MockSnapshot {
	meta: SnapshotMeta;
	view: ViewNode;
	recommendations: Recommendation[];
	preselected: string[];
}

export const mock = raw as unknown as MockSnapshot;

const LEVELS = 2;

function find(node: ViewNode, path: string): ViewNode | null {
	if (node.path === path) return node;
	for (const child of node.children) {
		if (child.kind === "Dir" && path.startsWith(`${child.path}`)) {
			const hit = find(child, path);
			if (hit) return hit;
		}
	}
	return null;
}

/** Keeps `levels` levels of children below `node`, like the backend's `get_view`. */
export function truncate(node: ViewNode, levels: number): ViewNode {
	return {
		...node,
		children:
			levels === 0 ? [] : node.children.map((c) => truncate(c, levels - 1)),
	};
}

let cancelled = false;

export const mockApi: Api = {
	loadLastSnapshot: async () => mock.meta,
	async startScan(onEvent: (e: ScanEvent) => void) {
		cancelled = false;
		const steps = 12;
		for (let i = 1; i <= steps; i++) {
			await new Promise((r) => setTimeout(r, 150));
			if (cancelled) {
				onEvent({ event: "failed", data: { message: "Scan cancelled" } });
				return;
			}
			onEvent({
				event: "progress",
				data: {
					files: i * 120_000,
					bytes: Math.round((mock.meta.scanned_size / steps) * i),
					current: `~/Library/Caches/step-${i}`,
				},
			});
		}
		onEvent({ event: "finished", data: { meta: mock.meta } });
	},
	cancelScan: async () => {
		cancelled = true;
	},
	async getView(nodePath) {
		const hit = find(mock.view, nodePath === "" ? mock.view.path : nodePath);
		if (!hit) throw new Error(`not in mock: ${nodePath}`);
		return truncate(hit, LEVELS);
	},
	getRecommendations: async () => mock.recommendations,
	revealInFinder: async (p) => {
		console.info("[mock] reveal in Finder:", p);
	},
};
