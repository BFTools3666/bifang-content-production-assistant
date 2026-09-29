import { createStore } from "jotai/vanilla";
import { describe, expect, it } from "vitest";

import {
  WORKBENCH_PAGE_SIZE_OPTIONS,
  workbenchPageSession,
  workbenchSearchAtom,
  workbenchSortAtom,
  workbenchStatusFilterAtom,
} from "./workbenchState";

describe("workbench page session", () => {
  /** 同一应用根 store 在页面卸载期间继续持有完整工作台会话。 */
  it("keeps the selection and pagination in the same application store", () => {
    const applicationStore = createStore();

    applicationStore.set(workbenchSearchAtom, "needle");
    applicationStore.set(workbenchStatusFilterAtom, "approved");
    applicationStore.set(workbenchSortAtom, "updated");
    applicationStore.set(workbenchPageSession.setPaginationAtom, {
      page: 3,
      pageSize: 20,
    });

    expect(applicationStore.get(workbenchPageSession.stateAtom)).toEqual({
      activeTab: "approved",
      page: 3,
      pageSize: 20,
      query: {
        search: "needle",
        sort: "updated",
      },
    });
    expect(applicationStore.get(workbenchSearchAtom)).toBe("needle");
    expect(applicationStore.get(workbenchStatusFilterAtom)).toBe("approved");
    expect(applicationStore.get(workbenchSortAtom)).toBe("updated");
  });

  /** 搜索、状态范围和排序任一变化都从第一页重新派生结果。 */
  it("resets the page after each selection change", () => {
    const applicationStore = createStore();
    const moveToPageFour = (): void => {
      applicationStore.set(workbenchPageSession.setPaginationAtom, {
        page: 4,
        pageSize: 20,
      });
    };

    moveToPageFour();
    applicationStore.set(workbenchSearchAtom, "new query");
    expect(applicationStore.get(workbenchPageSession.stateAtom).page).toBe(1);

    moveToPageFour();
    applicationStore.set(workbenchStatusFilterAtom, "partialFailure");
    expect(applicationStore.get(workbenchPageSession.stateAtom).page).toBe(1);

    moveToPageFour();
    applicationStore.set(workbenchSortAtom, "title");
    expect(applicationStore.get(workbenchPageSession.stateAtom).page).toBe(1);
  });

  /** 每页数量改变时使用模板契约回到第一页。 */
  it("resets the page when the page size changes", () => {
    const applicationStore = createStore();
    applicationStore.set(workbenchPageSession.setPaginationAtom, {
      page: 4,
      pageSize: 20,
    });
    applicationStore.set(workbenchPageSession.setPaginationAtom, {
      page: 4,
      pageSize: 50,
    });

    expect(applicationStore.get(workbenchPageSession.stateAtom)).toMatchObject({
      page: 1,
      pageSize: 50,
    });
    expect(WORKBENCH_PAGE_SIZE_OPTIONS).toEqual([20, 50, 100]);
  });

  /** 新应用 store 不继承上个进程的查询、范围、排序或分页。 */
  it("starts from workbench defaults in a new application store", () => {
    const firstProgramStore = createStore();
    firstProgramStore.set(workbenchSearchAtom, "persist only in memory");
    firstProgramStore.set(workbenchStatusFilterAtom, "approved");
    firstProgramStore.set(workbenchSortAtom, "status");
    firstProgramStore.set(workbenchPageSession.setPaginationAtom, {
      page: 5,
      pageSize: 100,
    });

    const nextProgramStore = createStore();
    expect(nextProgramStore.get(workbenchPageSession.stateAtom)).toEqual({
      activeTab: "all",
      page: 1,
      pageSize: 20,
      query: {
        search: "",
        sort: "created",
      },
    });
  });
});
