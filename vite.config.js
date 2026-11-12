import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "0.0.0.0", // Permitir acceso desde dispositivos móviles en la misma red
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2022', 'chrome105', 'safari15'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
