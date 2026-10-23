import { useScan } from "../store/scan";

interface Props {
	title: string;
	body: string;
	action: string;
	tone?: "error";
}

function Message({ title, body, action, tone }: Props) {
	return (
		<section
			aria-label={title}
			className="m-auto w-[480px] rounded-xl border border-[var(--card-border)] bg-[var(--card)] p-6"
		>
			<h1 className="text-[15px] font-semibold">{title}</h1>
			<p
				role={tone === "error" ? "alert" : undefined}
				className={`mt-1.5 text-[13px] ${tone === "error" ? "text-red-600" : "text-[var(--muted)]"}`}
			>
				{body}
			</p>
			<button
				type="button"
				onClick={() => void useScan.getState().start()}
				className="mt-5 rounded-lg bg-[var(--text)] px-4 py-1.5 text-[13px] font-medium text-white"
			>
				{action}
			</button>
		</section>
	);
}

/** No scan yet. A cancelled scan lands here with its notice. */
export function IdleState() {
	const notice = useScan((s) => s.error);
	return (
		<Message
			title="Scan your home folder"
			body={
				notice ??
				"DiskLens reads folder sizes only. It never opens your files and removes nothing until you choose."
			}
			action="Scan home folder"
		/>
	);
}

export function ErrorState() {
	const error = useScan((s) => s.error);
	return (
		<Message
			title="Scan failed"
			body={error ?? "Something went wrong."}
			action="Try again"
			tone="error"
		/>
	);
}
