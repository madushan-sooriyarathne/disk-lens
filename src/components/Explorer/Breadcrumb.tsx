import { formatBytes } from "../../lib/format";
import { crumbs } from "../../lib/treemap";
import { useView } from "../../store/view";

export function Breadcrumb() {
	const { rootPath, path, view, navigate } = useView();
	const trail = crumbs(rootPath, path);
	return (
		<nav
			aria-label="Folder path"
			className="flex items-center justify-between px-5 py-2.5 text-[13px] text-[var(--muted)]"
		>
			<ol className="flex items-center gap-1.5">
				{trail.map((c, i) => {
					const last = i === trail.length - 1;
					return (
						<li key={c.path} className="flex items-center gap-1.5">
							{i > 0 && <span aria-hidden>/</span>}
							{last ? (
								<span aria-current="page" className="text-[var(--text)]">
									{c.label}
								</span>
							) : (
								<button
									type="button"
									className="hover:text-[var(--text)] hover:underline"
									onClick={() => void navigate(c.path)}
								>
									{c.label}
								</button>
							)}
						</li>
					);
				})}
			</ol>
			{view && <span className="font-mono">{formatBytes(view.size)}</span>}
		</nav>
	);
}
