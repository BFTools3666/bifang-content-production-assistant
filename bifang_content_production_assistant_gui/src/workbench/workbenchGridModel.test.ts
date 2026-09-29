import { describe, expect, it } from "vitest";

import { deriveVisibleArticles, paginateArticles } from "./workbenchGridModel";
import type { ArticleView, OverallStatus } from "./workbenchTypes";

/** 构造只包含 Grid 派生所需字段的权威快照行。 */
function article(
  id: string,
  title: string,
  status: OverallStatus,
  createdAtMs: number,
): ArticleView {
  return {
    article: {
      createdAtMs,
      id,
      title,
      updatedAtMs: createdAtMs,
    },
    overallStatus: status,
  } as ArticleView;
}

describe("workbench Grid model", () => {
  /** 筛选与排序必须作用于完整快照，最后一步才提取当前页。 */
  it("filters and sorts the full snapshot before pagination", () => {
    const snapshot = Array.from({ length: 30 }, (_, index) =>
      article(
        `article-${index}`,
        `${index % 3 === 0 ? "目标" : "其他"}-${String(index).padStart(2, "0")}`,
        index % 2 === 0 ? "draft" : "approved",
        index,
      ),
    );

    const matching = deriveVisibleArticles(
      snapshot,
      "目标",
      "draft",
      "title",
      "zh-CN",
    );
    const secondPage = paginateArticles(matching, 2, 2);

    expect(matching.map((view) => view.article.id)).toEqual([
      "article-0",
      "article-6",
      "article-12",
      "article-18",
      "article-24",
    ]);
    expect(secondPage.map((view) => view.article.id)).toEqual([
      "article-12",
      "article-18",
    ]);
  });

  /** 创建时间排序保留稳定 ID 兜底，并且分页不改变完整结果。 */
  it("keeps created-time ordering stable while slicing a page", () => {
    const matching = deriveVisibleArticles(
      [
        article("article-a", "A", "draft", 2),
        article("article-z", "Z", "draft", 2),
        article("article-old", "Old", "draft", 1),
      ],
      "",
      "all",
      "created",
      "zh-CN",
    );

    expect(
      paginateArticles(matching, 1, 2).map((view) => view.article.id),
    ).toEqual(["article-z", "article-a"]);
    expect(matching).toHaveLength(3);
  });
});
