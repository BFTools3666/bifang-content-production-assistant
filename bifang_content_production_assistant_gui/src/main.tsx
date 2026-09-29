import "@mantine/core/styles.css";
import "./styles.css";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { createStore, Provider as JotaiProvider } from "jotai";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { interfaceLanguageAtom } from "./appState";
import { AppThemeProviderTemplate } from "./brand-support/AppThemeProviderTemplate";
import { initializeI18n } from "./i18n";
import { routeTree } from "./routeTree.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
});
const appStore = createStore();
const router = createRouter({ routeTree });

/** 为 TanStack Router 注册生成路由树的静态类型。 */
declare module "@tanstack/react-router" {
  /** 让文件路由使用当前唯一 Router 实例的静态类型。 */
  interface Register {
    router: typeof router;
  }
}

/** 初始化语言后挂载唯一 React、Mantine、Query 与 Jotai 根。 */
async function bootstrap(): Promise<void> {
  const language = await initializeI18n();
  appStore.set(interfaceLanguageAtom, language);
  const root = document.getElementById("root");
  if (!root) throw new Error("missing application root");
  createRoot(root).render(
    <StrictMode>
      <JotaiProvider store={appStore}>
        <QueryClientProvider client={queryClient}>
          <AppThemeProviderTemplate>
            <RouterProvider router={router} />
          </AppThemeProviderTemplate>
        </QueryClientProvider>
      </JotaiProvider>
    </StrictMode>,
  );
}

void bootstrap();
