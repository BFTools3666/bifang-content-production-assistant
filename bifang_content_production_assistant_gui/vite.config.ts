import { readFileSync } from "node:fs";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

/** 本仓库是独立产品，环境变量不允许将前端切换成另一个工具。 */
const product = JSON.parse(readFileSync(new URL("../product-edition.json", import.meta.url), "utf8"));
if (product.schemaVersion !== 1 || !["seo", "xhs"].includes(product.edition)) {
  throw new Error("产品身份清单无效");
}

export default defineConfig({
  define: {
    "import.meta.env.VITE_BIFANG_EDITION": JSON.stringify(product.edition),
  },
  build: {
    sourcemap: false,
  },
  clearScreen: false,
  plugins: [
    tanstackRouter({
      autoCodeSplitting: true,
      target: "react",
    }),
    react(),
  ],
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
  },
});
