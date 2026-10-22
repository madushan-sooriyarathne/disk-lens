import { create } from "zustand";
import { api, isMock, mockPreselect } from "../lib/api";
import type { Recommendation } from "../lib/bindings/Recommendation";
import { mock } from "../lib/mockApi";
import {
	pruneSelection,
	readyIds,
	selectSafe,
	toggleRow,
} from "../lib/selection";

interface SelectionState {
	recommendations: Recommendation[];
	/** Selected item IDs. Empty by default (S-09); only ready items. */
	selected: ReadonlySet<string>;
	/** Row whose detail drawer is open. */
	focusedRow: string | null;
	/** After a scan or snapshot load: fresh rows, empty selection. */
	load: () => Promise<void>;
	/** Re-evaluates blocked states (window focus) and drops stale selections. */
	refresh: () => Promise<void>;
	toggleItem: (itemId: string) => void;
	toggleRow: (rowId: string) => void;
	selectSafe: () => void;
	focusRow: (rowId: string | null) => void;
}

export const useSelection = create<SelectionState>((set, get) => ({
	recommendations: [],
	selected: new Set(),
	focusedRow: null,
	async load() {
		const recommendations = await api.getRecommendations();
		const selected = new Set<string>();
		if (isMock && mockPreselect) {
			for (const r of recommendations) {
				if (mock.preselected.includes(r.row_id)) {
					for (const id of readyIds(r)) selected.add(id);
				}
			}
		}
		set({ recommendations, selected, focusedRow: null });
	},
	async refresh() {
		const recommendations = await api.getRecommendations();
		set((s) => ({
			recommendations,
			selected: pruneSelection(recommendations, s.selected),
		}));
	},
	toggleItem: (itemId) =>
		set((s) => {
			const ready = s.recommendations.some((r) => readyIds(r).includes(itemId));
			if (!ready) return {};
			const next = new Set(s.selected);
			if (!next.delete(itemId)) next.add(itemId);
			return { selected: next };
		}),
	toggleRow: (rowId) => {
		const row = get().recommendations.find((r) => r.row_id === rowId);
		if (row) set((s) => ({ selected: toggleRow(row, s.selected) }));
	},
	selectSafe: () =>
		set((s) => ({ selected: selectSafe(s.recommendations, s.selected) })),
	focusRow: (focusedRow) => set({ focusedRow }),
}));
