import { useEffect } from "react";
import { Banner } from "./components/Banner";
import { ErrorState, IdleState } from "./components/EmptyState";
import { Explorer } from "./components/Explorer/Explorer";
import { Panel } from "./components/Recommendations/Panel";
import { ScanProgress } from "./components/ScanProgress";
import { StatCards } from "./components/StatCards";
import { useScan } from "./store/scan";

export default function App() {
	const phase = useScan((s) => s.phase);
	useEffect(() => {
		void useScan.getState().init();
	}, []);
	if (phase !== "ready") {
		return (
			<main className="flex h-full p-4">
				{phase === "idle" && <IdleState />}
				{phase === "scanning" && <ScanProgress />}
				{phase === "error" && <ErrorState />}
			</main>
		);
	}
	return (
		<main className="flex h-full flex-col gap-4 p-4">
			<Banner />
			<StatCards />
			<div className="flex min-h-0 flex-1 gap-4">
				<Explorer />
				<Panel />
			</div>
		</main>
	);
}
