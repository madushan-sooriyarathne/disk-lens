import { useEffect } from "react";
import { Banner } from "./components/Banner";
import { ErrorState, IdleState } from "./components/EmptyState";
import { Explorer } from "./components/Explorer/Explorer";
import { ScanProgress } from "./components/ScanProgress";
import { StatCards } from "./components/StatCards";
import { useScan } from "./store/scan";

function RecommendationsCard() {
	return (
		<section
			aria-label="Recommendations"
			className="flex w-[390px] shrink-0 flex-col rounded-xl border border-[var(--card-border)] bg-[var(--card)]"
		>
			<header className="px-5 pt-5">
				<h2 className="text-[15px] font-semibold">Recommendations</h2>
				<p className="mt-0.5 text-[13px] text-[var(--muted)]">
					Select items to build a cleanup script
				</p>
			</header>
			<div className="m-5 flex flex-1 items-center justify-center rounded-lg border border-dashed border-[var(--card-border)] text-[13px] text-[var(--muted)]">
				Coming in Phase 3
			</div>
		</section>
	);
}

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
				<RecommendationsCard />
			</div>
		</main>
	);
}
