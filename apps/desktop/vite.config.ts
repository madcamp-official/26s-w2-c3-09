import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    strictPort: true,
    port: 1420,
    watch: {
      // Cargo writes and locks native object files here while Tauri is compiling.
      // Watching them on Windows can terminate Vite with EBUSY.
      ignored: ["**/src-tauri/target/**"]
    }
  },
  build: {
    outDir: "dist",
    emptyOutDir: true
  }
});
