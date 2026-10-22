import { useEffect } from "react";
import { useScan } from "./store/scan";

export default function App() {
	const phase = useScan((s) => s.phase);
	useEffect(() => {
		void useScan.getState().init();
	}, []);
	return (
		<main className="flex h-full flex-col gap-4 p-4">
			<h1 className="text-lg font-medium">DiskLens</h1>
			<p className="text-sm text-[var(--muted)]">phase: {phase}</p>
		</main>
	);
}
