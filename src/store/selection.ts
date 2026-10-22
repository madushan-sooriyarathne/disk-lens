import { create } from "zustand";
import { api, isMock, mockPreselect } from "../lib/api";
import type { Recommendation } from "../lib/bindings/Recommendation";
import { mock } from "../lib/mockApi";

interface SelectionState {
	recommendations: Recommendation[];
	/** Selected item IDs. Empty by default (S-09). */
	selected: ReadonlySet<string>;
	/** Row to focus in the Recommendations panel (Phase 3). */
	focusedRow: string | null;
	load: () => Promise<void>;
	toggleItem: (itemId: string) => void;
	focusRow: (rowId: string | null) => void;
}

export const useSelection = create<SelectionState>((set) => ({
	recommendations: [],
	selected: new Set(),
	focusedRow: null,
	async load() {
		const recommendations = await api.getRecommendations();
		const selected = new Set<string>();
		if (isMock && mockPreselect) {
			for (const r of recommendations) {
				if (mock.preselected.includes(r.row_id)) {
					for (const i of r.items) selected.add(i.item_id);
				}
			}
		}
		set({ recommendations, selected, focusedRow: null });
	},
	toggleItem: (itemId) =>
		set((s) => {
			const next = new Set(s.selected);
			if (!next.delete(itemId)) next.add(itemId);
			return { selected: next };
		}),
	focusRow: (focusedRow) => set({ focusedRow }),
}));
