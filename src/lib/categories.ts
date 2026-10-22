/** Legend order follows the mockup. Colors live in CSS tokens (`--cat-<id>-fill/border`). */
export const CATEGORIES = [
	{ id: "documents_media", label: "Documents & media" },
	{ id: "dev_tools", label: "Dev tools & other" },
	{ id: "code_projects", label: "Code projects" },
	{ id: "mobile_sdks", label: "iOS & Android SDKs" },
	{ id: "ai_models", label: "AI models" },
	{ id: "package_caches", label: "Package caches" },
] as const;

export function categoryLabel(id: string): string {
	return CATEGORIES.find((c) => c.id === id)?.label ?? id;
}
