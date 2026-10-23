import { Channel, invoke } from "@tauri-apps/api/core";
import type { ItemDetails } from "./bindings/ItemDetails";
import type { Recommendation } from "./bindings/Recommendation";
import type { ScanEvent } from "./bindings/ScanEvent";
import type { SnapshotMeta } from "./bindings/SnapshotMeta";
import type { ViewMode } from "./bindings/ViewMode";
import type { ViewNode } from "./bindings/ViewNode";
import { mockApi } from "./mockApi";

/** Everything the UI needs from the backend; the mock implements the same shape. */
export interface Api {
	loadLastSnapshot(): Promise<SnapshotMeta | null>;
	startScan(onEvent: (e: ScanEvent) => void): Promise<void>;
	cancelScan(): Promise<void>;
	getView(nodePath: string, mode: ViewMode): Promise<ViewNode>;
	getRecommendations(): Promise<Recommendation[]>;
	getItemDetails(rowId: string): Promise<ItemDetails>;
	revealInFinder(nodePath: string): Promise<void>;
}

const tauriApi: Api = {
	loadLastSnapshot: () => invoke("load_last_snapshot"),
	async startScan(onEvent) {
		const channel = new Channel<ScanEvent>();
		channel.onmessage = onEvent;
		await invoke("start_scan", { channel });
	},
	cancelScan: () => invoke("cancel_scan"),
	getView: (nodePath, mode) => invoke("get_view", { nodePath, mode }),
	getRecommendations: () => invoke("get_recommendations"),
	getItemDetails: (rowId) => invoke("get_item_details", { rowId }),
	revealInFinder: (nodePath) => invoke("reveal_in_finder", { nodePath }),
};

export const isMock =
	!("__TAURI_INTERNALS__" in window) ||
	new URLSearchParams(window.location.search).has("mock");

/** `?mock&preselect` also applies the mock's preselected rows (the real default is empty). */
export const mockPreselect = new URLSearchParams(window.location.search).has(
	"preselect",
);

export const api: Api = isMock ? mockApi : tauriApi;
