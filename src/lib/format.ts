const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Decimal (1000-based) sizes, matching Finder. */
export function formatBytes(bytes: number): string {
	let value = bytes;
	let unit = 0;
	while (value >= 1000 && unit < UNITS.length - 1) {
		value /= 1000;
		unit++;
	}
	const digits = unit === 0 || value >= 100 ? 0 : 1;
	return `${value.toFixed(digits)} ${UNITS[unit]}`;
}
