import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "dist",
    rollupOptions: {
      input: {
        index: "index.html",
        phone: "phone.html",
      },
    },
  },
  test: { passWithNoTests: true },
});
