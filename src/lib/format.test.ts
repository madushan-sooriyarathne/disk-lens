import { describe, expect, it } from "vitest";
import { formatBytes } from "./format";

describe("formatBytes", () => {
	it("formats across units", () => {
		expect(formatBytes(0)).toBe("0 B");
		expect(formatBytes(999)).toBe("999 B");
		expect(formatBytes(52_500_000_000)).toBe("52.5 GB");
		expect(formatBytes(288_000_000_000)).toBe("288 GB");
	});
});
