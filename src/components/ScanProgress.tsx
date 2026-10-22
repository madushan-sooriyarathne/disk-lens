import { formatBytes } from "../lib/format";
import { useScan } from "../store/scan";

export function ScanProgress() {
	const { files, bytes, current } = useScan((s) => s.progress);
	return (
		<section
			aria-label="Scan progress"
			className="m-auto w-[480px] rounded-xl border border-[var(--card-border)] bg-[var(--card)] p-6"
		>
			<h1 className="text-[15px] font-semibold">Scanning…</h1>
			<div className="mt-1 h-1 overflow-hidden rounded-full bg-[var(--bg)]">
				<div className="h-full w-1/3 animate-[slide_1.2s_ease-in-out_infinite] rounded-full bg-[var(--cat-code_projects-border)]" />
			</div>
			<dl className="mt-5 grid grid-cols-2 gap-4">
				<div>
					<dt className="text-[13px] text-[var(--muted)]">Files counted</dt>
					<dd className="font-mono text-[22px] font-semibold">
						{files.toLocaleString()}
					</dd>
				</div>
				<div>
					<dt className="text-[13px] text-[var(--muted)]">Size so far</dt>
					<dd className="font-mono text-[22px] font-semibold">
						{formatBytes(bytes)}
					</dd>
				</div>
			</dl>
			<p
				className="mt-4 truncate font-mono text-xs text-[var(--muted)]"
				title={current}
				aria-live="off"
			>
				{current || "Starting…"}
			</p>
			<button
				type="button"
				onClick={() => void useScan.getState().cancel()}
				className="mt-5 rounded-lg border border-[var(--card-border)] px-4 py-1.5 text-[13px] font-medium hover:bg-[var(--bg)]"
			>
				Cancel
			</button>
		</section>
	);
}
