import { create } from "zustand";
import { api } from "../lib/api";
import type { ViewMode } from "../lib/bindings/ViewMode";
import type { ViewNode } from "../lib/bindings/ViewNode";

interface ViewState {
	mode: ViewMode;
	rootPath: string;
	path: string;
	view: ViewNode | null;
	hoveredId: string | null;
	reset: (rootPath: string) => Promise<void>;
	navigate: (path: string) => Promise<void>;
	up: () => Promise<void>;
	setMode: (mode: ViewMode) => Promise<void>;
	hover: (id: string | null) => void;
}

function parentOf(path: string): string {
	return path.slice(0, path.lastIndexOf("/")) || "/";
}

export const useView = create<ViewState>((set, get) => ({
	mode: "Category",
	rootPath: "",
	path: "",
	view: null,
	hoveredId: null,
	async reset(rootPath) {
		const view = await api.getView(rootPath, get().mode);
		set({ rootPath, path: rootPath, view, hoveredId: null });
	},
	async navigate(path) {
		const view = await api.getView(path, get().mode);
		set({ path, view, hoveredId: null });
	},
	async up() {
		const { path, rootPath, navigate } = get();
		if (path !== rootPath) await navigate(parentOf(path));
	},
	async setMode(mode) {
		const view = await api.getView(get().path, mode);
		set({ mode, view });
	},
	hover: (hoveredId) => set({ hoveredId }),
}));
