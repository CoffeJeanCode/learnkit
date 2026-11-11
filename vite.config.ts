import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // three.js is only reached through a dynamic import (the 3D visuals), so the
  // dev server would discover it late and force a full page reload the first
  // time a 3D visual appears. Pre-bundling it up front avoids that.
  optimizeDeps: {
    include: ["three", "three/addons/controls/OrbitControls.js", "three/addons/renderers/CSS2DRenderer.js"],
  },
  server: {
    port: 1420,
    strictPort: true,
    // Cargo writes (and locks) build artifacts under src-tauri/target while
    // `tauri dev` runs; watching them crashes Vite with EBUSY on Windows.
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: process.env.TAURI_ENV_PLATFORM ? "chrome105" : "esnext",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
