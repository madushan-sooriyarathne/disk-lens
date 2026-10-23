import { create } from "zustand";
import { api } from "../lib/api";
import type { SnapshotMeta } from "../lib/bindings/SnapshotMeta";
import { useSelection } from "./selection";
import { useView } from "./view";

export type Phase = "loading" | "idle" | "scanning" | "ready" | "error";

interface Progress {
	files: number;
	bytes: number;
	current: string;
}

interface ScanState {
	phase: Phase;
	meta: SnapshotMeta | null;
	progress: Progress;
	error: string | null;
	init: () => Promise<void>;
	start: () => Promise<void>;
	cancel: () => Promise<void>;
}

const ZERO: Progress = { files: 0, bytes: 0, current: "" };

async function enter(meta: SnapshotMeta) {
	await useView.getState().reset(meta.root);
	await useSelection.getState().load();
	useScan.setState({ phase: "ready", meta, error: null });
}

export const useScan = create<ScanState>((set) => ({
	phase: "loading",
	meta: null,
	progress: ZERO,
	error: null,
	async init() {
		try {
			const meta = await api.loadLastSnapshot();
			if (meta) await enter(meta);
			else set({ phase: "idle" });
		} catch (e) {
			set({ phase: "error", error: String(e) });
		}
	},
	async start() {
		set({ phase: "scanning", progress: ZERO, error: null });
		try {
			await api.startScan((ev) => {
				if (ev.event === "progress") set({ progress: ev.data });
				else if (ev.event === "finished") void enter(ev.data.meta);
				else
					set((s) => ({
						phase: s.meta ? "ready" : "idle",
						error: ev.data.message,
					}));
			});
		} catch (e) {
			set({ phase: "error", error: String(e) });
		}
	},
	cancel: () => api.cancelScan(),
}));
