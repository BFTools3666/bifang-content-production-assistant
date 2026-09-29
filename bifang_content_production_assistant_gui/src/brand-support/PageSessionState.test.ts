import { createStore } from "jotai/vanilla";
import { describe, expect, it } from "vitest";

import {
  createPageSessionState,
  type PageSessionState,
} from "./pageSessionState";

/** 测试页签使用的最小内容范围。 */
type TestScope = "all" | "failed";

const DEFAULT_STATE: PageSessionState<TestScope, { keyword: string }> = {
  activeTab: "all",
  query: { keyword: "" },
  page: 1,
  pageSize: 20,
};

const pageSession = createPageSessionState(DEFAULT_STATE);

describe("page session state", () => {
  /** 同一个应用根 store 在路由卸载和重挂期间保留查询与分页。 */
  it("keeps_tabs_queries_and_pagination_when_a_route_unmounts_and_remounts", () => {
    const applicationStore = createStore();

    applicationStore.set(pageSession.setSelectionAtom, {
      activeTab: "failed",
      query: { keyword: "needle" },
    });
    applicationStore.set(pageSession.setPaginationAtom, {
      page: 1,
      pageSize: 50,
    });
    applicationStore.set(pageSession.setPaginationAtom, {
      page: 4,
      pageSize: 50,
    });

    expect(applicationStore.get(pageSession.stateAtom)).toEqual({
      activeTab: "failed",
      query: { keyword: "needle" },
      page: 4,
      pageSize: 50,
    });
  });

  /** 每页数量变化必须原子回到第一页。 */
  it("resets_to_page_one_when_the_page_size_changes", () => {
    const applicationStore = createStore();
    applicationStore.set(pageSession.setPaginationAtom, {
      page: 4,
      pageSize: 20,
    });
    applicationStore.set(pageSession.setPaginationAtom, {
      page: 4,
      pageSize: 50,
    });

    expect(applicationStore.get(pageSession.stateAtom)).toMatchObject({
      page: 1,
      pageSize: 50,
    });
  });

  /** 查询或活动范围变化必须原子回到第一页。 */
  it("resets_to_page_one_when_the_tab_or_query_scope_changes", () => {
    const applicationStore = createStore();
    applicationStore.set(pageSession.setPaginationAtom, {
      page: 4,
      pageSize: 20,
    });
    applicationStore.set(pageSession.setSelectionAtom, {
      activeTab: "failed",
      query: { keyword: "new scope" },
    });

    expect(applicationStore.get(pageSession.stateAtom)).toMatchObject({
      activeTab: "failed",
      query: { keyword: "new scope" },
      page: 1,
    });
  });

  /** 只有成功返回的大于第一页空结果可以触发一次回退。 */
  it("falls_back_to_page_one_only_after_a_successful_empty_page_result", () => {
    const applicationStore = createStore();
    applicationStore.set(pageSession.setPaginationAtom, {
      page: 3,
      pageSize: 20,
    });

    expect(
      applicationStore.set(pageSession.reconcilePageAfterResultAtom, {
        status: "success",
        itemCount: 0,
      }),
    ).toBe(true);
    expect(applicationStore.get(pageSession.stateAtom).page).toBe(1);
    expect(
      applicationStore.set(pageSession.reconcilePageAfterResultAtom, {
        status: "success",
        itemCount: 0,
      }),
    ).toBe(false);
  });

  /** 加载、错误和第一页空结果都不能被误判为陈旧页码。 */
  it("does_not_treat_loading_errors_or_an_empty_first_page_as_a_stale_page", () => {
    const applicationStore = createStore();

    for (const status of ["loading", "error"] as const) {
      applicationStore.set(pageSession.setPaginationAtom, {
        page: 3,
        pageSize: 20,
      });
      expect(
        applicationStore.set(pageSession.reconcilePageAfterResultAtom, {
          status,
          itemCount: 0,
        }),
      ).toBe(false);
      expect(applicationStore.get(pageSession.stateAtom).page).toBe(3);
    }

    applicationStore.set(pageSession.setPaginationAtom, {
      page: 1,
      pageSize: 20,
    });
    expect(
      applicationStore.set(pageSession.reconcilePageAfterResultAtom, {
        status: "success",
        itemCount: 0,
      }),
    ).toBe(false);
  });

  /** 新应用 store 不继承上一进程的页面工作状态。 */
  it("starts_from_defaults_in_a_new_application_store", () => {
    const firstProgramStore = createStore();
    firstProgramStore.set(pageSession.setPaginationAtom, {
      page: 5,
      pageSize: 100,
    });

    const nextProgramStore = createStore();
    expect(nextProgramStore.get(pageSession.stateAtom)).toEqual(DEFAULT_STATE);
  });
});
