import { Text } from "@mantine/core";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { IS_XHS_EDITION } from "./researchState";
import styles from "./ResearchPage.module.css";

/** 呈现作品研究到原创成稿的工作路径，不用虚构业务数据装饰界面。 */
export function ResearchHeader({ title, description }: { title: string; description: string }): ReactElement {
  const { t } = useTranslation("research");
  return <header className={styles.masthead}>
    <div><div className={styles.eyebrow}>{IS_XHS_EDITION ? t("毕方 / 笔记创作") : t("毕方 / 文章创作")}</div><h1 className={styles.title}>{title}</h1><Text c="dimmed" size="sm">{description}</Text></div>

  </header>;
}
