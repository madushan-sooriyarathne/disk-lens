import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [react(), tailwindcss()],
	// Keep Rust errors visible in the terminal.
	clearScreen: false,
	// Tauri expects a fixed port.
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
		watch: { ignored: ["**/src-tauri/**", "**/.sandbox/**"] },
	},
	test: { include: ["src/**/*.test.ts"] },
});
