import { useScan } from "../store/scan";

/** Notices above the summary: unreadable folders, or a cancelled rescan. */
export function Banner() {
	const meta = useScan((s) => s.meta);
	const notice = useScan((s) => s.error);
	const unreadable = meta?.inaccessible_count ?? 0;
	if (!notice && unreadable === 0) return null;
	return (
		<div
			role="status"
			className="rounded-lg border border-[var(--review)] bg-[var(--tier-review-fill)] px-4 py-2 text-[13px]"
		>
			{notice ??
				`${unreadable} ${unreadable === 1 ? "folder" : "folders"} couldn't be read. Granting Full Disk Access lets DiskLens see them.`}
		</div>
	);
}
