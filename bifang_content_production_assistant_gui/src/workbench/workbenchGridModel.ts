import type { ArticleView, OverallStatus } from "./workbenchTypes";
import type { WorkbenchSort } from "./workbenchState";

/** 在不修改 Query 权威快照的前提下，先筛选再排序完整结果集。 */
export function deriveVisibleArticles(
  articles: ArticleView[],
  search: string,
  status: OverallStatus | "all",
  sort: WorkbenchSort,
  locale: string,
): ArticleView[] {
  const normalizedSearch = search.trim().toLocaleLowerCase(locale);
  return articles
    .filter((article) => status === "all" || article.overallStatus === status)
    .filter(
      (article) =>
        normalizedSearch.length === 0 ||
        article.article.title
          .toLocaleLowerCase(locale)
          .includes(normalizedSearch),
    )
    .slice()
    .sort((left, right) => {
      let compared: number;
      if (sort === "title") {
        compared = left.article.title.localeCompare(
          right.article.title,
          locale,
        );
      } else if (sort === "status") {
        compared = left.overallStatus.localeCompare(
          right.overallStatus,
          locale,
        );
      } else if (sort === "updated") {
        compared = right.article.updatedAtMs - left.article.updatedAtMs;
      } else {
        compared = right.article.createdAtMs - left.article.createdAtMs;
      }
      return (
        compared || right.article.id.localeCompare(left.article.id, "en-US")
      );
    });
}

/** 从已筛选且已排序的完整结果集中提取当前客户端页。 */
export function paginateArticles(
  articles: ArticleView[],
  page: number,
  pageSize: number,
): ArticleView[] {
  const start = (page - 1) * pageSize;
  return articles.slice(start, start + pageSize);
}
