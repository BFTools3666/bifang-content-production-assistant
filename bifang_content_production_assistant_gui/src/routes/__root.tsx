import { createRootRoute, Outlet } from "@tanstack/react-router";

import { AppShell } from "../AppShell";

/** 渲染不会随文件路由卸载的应用壳层。 */
function RootRouteComponent() {
  return (
    <AppShell>
      <Outlet />
    </AppShell>
  );
}

export const Route = createRootRoute({ component: RootRouteComponent });
