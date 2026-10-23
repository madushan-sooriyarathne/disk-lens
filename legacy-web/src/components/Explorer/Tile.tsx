import { memo } from "react";
import type { ViewMode } from "../../lib/bindings/ViewMode";
import { formatBytes } from "../../lib/format";
import {
	HATCH_OPACITY,
	hatchBackground,
	labelVisibility,
	type Rect,
	sizeFontPx,
	tileColors,
} from "../../lib/treemap";

interface Props {
	rect: Rect;
	mode: ViewMode;
}

/** Positioned by absolute pixels; memoised so hover and tooltip state never re-render it. */
export const Tile = memo(function Tile({ rect, mode }: Props) {
	const { node, x, y, w, h, group } = rect;
	const colors = tileColors(node, mode);
	const clickable = node.kind === "Dir";
	const size = formatBytes(node.size);
	const show = labelVisibility(w, h);
	return (
		// biome-ignore lint/a11y/useAriaPropsSupportedByRole: the role is button or img, both take aria-label
		<div
			data-id={node.id}
			role={clickable ? "button" : "img"}
			tabIndex={clickable ? 0 : undefined}
			aria-label={`${node.name}, ${size}`}
			className={`absolute overflow-hidden ${group ? "rounded-[10px]" : "rounded-lg"} ${clickable ? "cursor-pointer hover:brightness-[0.97]" : "cursor-default"}`}
			style={{
				left: x,
				top: y,
				width: w,
				height: h,
				background: colors.fill,
				border: `1px solid ${colors.border}`,
			}}
		>
			{colors.hatch && (
				<div
					aria-hidden
					className="pointer-events-none absolute inset-0"
					style={{
						background: hatchBackground(colors.hatch),
						opacity: HATCH_OPACITY,
					}}
				/>
			)}
			{group ? (
				<div className="relative flex items-baseline justify-between gap-2 px-3 pt-2 text-[13px]">
					<span className="truncate font-semibold">{node.name}</span>
					{w >= 110 && (
						<span className="shrink-0 font-mono text-xs text-[var(--muted)]">
							{size}
						</span>
					)}
				</div>
			) : (
				<>
					{show.name && (
						<span className="absolute left-2.5 top-2 right-2 truncate text-[13px] font-medium">
							{node.name}
						</span>
					)}
					{show.size && (
						<span
							className="absolute bottom-2 left-2.5 font-mono font-medium"
							style={{ fontSize: sizeFontPx(w * h) }}
						>
							{size}
						</span>
					)}
				</>
			)}
		</div>
	);
});
