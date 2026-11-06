// The only source for what the site says DiskLens does. A feature is "available"
// only when it ships in the app today; everything else in PRD 13.10 is "coming".
export type Feature = {
  slug: string;
  title: string;
  summary: string;
  tier: "free" | "pro";
  status: "available" | "coming";
};

export const features: Feature[] = [
  {
    slug: "scan",
    title: "Fast home folder scan",
    summary:
      "A parallel scan that counts allocated size, counts hard links once, never follows symlinks and never opens iCloud placeholders.",
    tier: "free",
    status: "available",
  },
  {
    slug: "overview",
    title: "Overview",
    summary: "Disk capacity, how much is safe to clean, your largest folders and a plain-language category breakdown.",
    tier: "free",
    status: "available",
  },
  {
    slug: "explore",
    title: "Explore treemap",
    summary:
      "One readable level at a time. Textures show reclaimable space at a glance: striped tiles are disposable, plain translucent tiles are restorable.",
    tier: "free",
    status: "available",
  },
  {
    slug: "recommended",
    title: "Recommended cleanup",
    summary:
      "Caches, build output, simulators, models and worktrees, each with a plain explanation of what happens if it is removed. Safe items and review-first items stay apart.",
    tier: "free",
    status: "available",
  },
  {
    slug: "queue",
    title: "Staging queue",
    summary: "Stage items from Explore, Recommended or the command palette, review them in one list, then clean up once.",
    tier: "free",
    status: "available",
  },
  {
    slug: "cleanup",
    title: "Cleanup you confirm",
    summary:
      "Every cleanup shows what will run and how. Most items go to the Trash. Nothing starts until you click the confirm button.",
    tier: "free",
    status: "available",
  },
  {
    slug: "script",
    title: "Export as a shell script",
    summary: "Copy or save any cleanup as a readable script that runs the same steps the app would.",
    tier: "free",
    status: "available",
  },
  {
    slug: "git-guard",
    title: "Active-work guard",
    summary:
      "Build folders in a repository with uncommitted changes or unpushed commits move to review and carry an Active work badge.",
    tier: "free",
    status: "available",
  },
  {
    slug: "palette",
    title: "Command palette",
    summary: "Press ⌘K to jump to pages, folders and recommendations, filter by size or tier, and stage single items.",
    tier: "free",
    status: "available",
  },
  {
    slug: "keyboard",
    title: "Keyboard navigation",
    summary: "Move through the treemap with h, j, k, l or the arrow keys. Space stages, Return opens, Esc goes back.",
    tier: "free",
    status: "available",
  },
  {
    slug: "rescan",
    title: "Background rescan",
    summary: "Results stay on screen while DiskLens updates them, and the app rescans on its own when the data is a day old.",
    tier: "free",
    status: "available",
  },
  {
    slug: "onboarding",
    title: "Guided first run",
    summary: "A three-step welcome that explains Full Disk Access and runs your first scan.",
    tier: "free",
    status: "available",
  },
  {
    slug: "batch-commands",
    title: "Palette batch commands and saved filters",
    summary:
      "Stage every ready item of a rule, or everything that matches a query such as safe items over 1 GB, and save filters for reuse. Batch commands only stage; you still confirm.",
    tier: "pro",
    status: "available",
  },
  {
    slug: "lockfile-tags",
    title: "Lockfile tags and orphaned stores",
    summary:
      "See which build folders a lockfile can rebuild, and find packages in the pnpm, Bun, Cargo and Homebrew stores that no project uses.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "ai-models",
    title: "AI models hub",
    summary: "Local Ollama, Hugging Face and LM Studio models grouped by family, quantization and last use.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "containers",
    title: "Containers and virtual machines",
    summary: "Look inside Docker Desktop, OrbStack, Colima and Lima disk images and remove unused images and build cache.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "scratch-data",
    title: "Apple, mobile and creative scratch data",
    summary: "Extra scanners for unused simulator runtimes, per-project DerivedData, Gradle wrappers and creative app caches.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "snapshots",
    title: "APFS snapshot inspector",
    summary: "See the Time Machine local snapshots that hold purgeable space, and thin or delete them with an explicit prompt.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "menu-bar",
    title: "Menu bar agent",
    summary: "Free space in the menu bar, low-disk alerts and a note when a process is writing gigabytes.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "auto-sweep",
    title: "Auto-sweep rules",
    summary: "Scheduled rules that stage idle project artifacts and notify you. They never remove anything on their own.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "volumes",
    title: "Other folders and volumes",
    summary: "Scan folders outside your home folder and external drives.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "history",
    title: "History",
    summary: "What grew since the last scan, compared per folder.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "large-old",
    title: "Large and old files finder",
    summary: "Find the biggest files you have not touched in a long time.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "duplicates",
    title: "Duplicate finder",
    summary: "Find identical files and see how much space they take.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "local-rules",
    title: "Editable local rules",
    summary: "Add your own cleanup rules in a local file.",
    tier: "pro",
    status: "coming",
  },
  {
    slug: "clone-sizes",
    title: "Clone-aware sizes",
    summary: "Sizes that account for APFS clones, so shared data is not counted twice.",
    tier: "pro",
    status: "coming",
  },
];

export const COMING_LABEL = "Coming in 2.x";
