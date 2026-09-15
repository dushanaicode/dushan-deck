import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  cacheDir: "Temp/tooling/vite",
  clearScreen: false,
  optimizeDeps: { entries: ["index.html", "float.html"] },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    fs: {
      allow: [
        "src",
        "index.html",
        "float.html",
        "node_modules",
        "Temp/tooling/frontend/node_modules",
        "Temp/tooling/vite",
      ].map((path) => resolve(path)),
      deny: [
        "**/.docs/**",
        "**/.git/**",
        "**/Temp/references/**",
        "**/*.sqlite3*",
      ],
    },
    watch: {
      ignored: ["**/Temp/**", "**/.docs/**", "**/src-tauri/**", "**/crates/**"],
    },
  },
  build: {
    outDir: "Temp/build/frontend",
    emptyOutDir: false,
    rolldownOptions: {
      input: { main: resolve("index.html"), floating: resolve("float.html") },
    },
  },
});
