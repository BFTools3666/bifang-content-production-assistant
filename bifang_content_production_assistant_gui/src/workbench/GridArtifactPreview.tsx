import { Box, Button, Image, Stack, Text, UnstyledButton } from "@mantine/core";
import { IconRefresh } from "@tabler/icons-react";
import { useEffect, useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { loadArticleThumbnail } from "./workbenchCommands";
import type { StageStatus } from "./workbenchTypes";
import classes from "./workbench.module.css";

/** Grid 中可直接打开已有详情页签的产物类型。 */
export type GridInspectableArtifact = "article" | "summary" | "image";

const pendingThumbnailLoads = new Map<string, Promise<string | null>>();

/** 合并同一图片尝试的并发读取，避免开发期 StrictMode 或重复观察造成重复解码。 */
function loadGridThumbnail(
  taskId: string,
  attemptId: string | null,
  editRevision: number,
): Promise<string | null> {
  const key = `${taskId}:${attemptId ?? "legacy"}:edit-${editRevision}`;
  const pending = pendingThumbnailLoads.get(key);
  if (pending) return pending;
  const request = loadArticleThumbnail(taskId);
  pendingThumbnailLoads.set(key, request);
  const clear = () => {
    if (pendingThumbnailLoads.get(key) === request) {
      pendingThumbnailLoads.delete(key);
    }
  };
  void request.then(clear, clear);
  return request;
}

/** 在单元格内显示有界文本，点击只打开对应的手工编辑页签。 */
export function GridTextPreview({
  onInspect,
  stage,
  status,
  taskId,
  title,
  value,
}: {
  onInspect: (taskId: string, stage: GridInspectableArtifact) => void;
  stage: "article" | "summary";
  status: StageStatus;
  taskId: string;
  title: string;
  value: string | null;
}): ReactElement | null {
  const { t } = useTranslation("app");
  const preview = value?.trim();
  if (!preview) return null;

  return (
    <UnstyledButton
      aria-label={t("workbench.grid.open_text_preview", {
        stage: t(`workbench.grid.${stage}`),
        status: t(`status.stage.${status}`),
        title,
      })}
      className={classes.gridTextPreviewButton}
      onClick={() => onInspect(taskId, stage)}
      type="button"
    >
      <Text className={classes.gridTextPreview} lineClamp={3} size="xs">
        {preview}
      </Text>
    </UnstyledButton>
  );
}

/** 只在配图格进入视口附近时读取原生层缩略图。 */
export function GridImagePreview({
  attemptId,
  editRevision,
  hasImage,
  onInspect,
  status,
  taskId,
  title,
}: {
  attemptId: string | null;
  editRevision: number;
  hasImage: boolean;
  onInspect: (taskId: string, stage: GridInspectableArtifact) => void;
  status: StageStatus;
  taskId: string;
  title: string;
}): ReactElement | null {
  const { t } = useTranslation("app");
  const hostRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(
    () => typeof globalThis.IntersectionObserver === "undefined",
  );
  const [source, setSource] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [reloadVersion, setReloadVersion] = useState(0);

  useEffect(() => {
    if (!hasImage) {
      setVisible(false);
      return;
    }
    const host = hostRef.current;
    if (!host || typeof globalThis.IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        setVisible(entries.some((entry) => entry.isIntersecting));
      },
      { rootMargin: "160px" },
    );
    observer.observe(host);
    return () => observer.disconnect();
  }, [attemptId, editRevision, hasImage, taskId]);

  useEffect(() => {
    let active = true;
    setSource(null);
    setFailed(false);
    if (!hasImage || !visible) {
      setLoading(false);
      return () => {
        active = false;
      };
    }
    setLoading(true);
    void loadGridThumbnail(taskId, attemptId, editRevision)
      .then((thumbnail) => {
        if (!active) return;
        setSource(thumbnail);
        setFailed(thumbnail === null);
      })
      .catch(() => {
        if (!active) return;
        setSource(null);
        setFailed(true);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [attemptId, editRevision, hasImage, reloadVersion, taskId, visible]);

  if (!hasImage) return null;

  return (
    <Box className={classes.gridImagePreviewHost} ref={hostRef}>
      {loading ? (
        <Box className={classes.gridImagePreviewState} role="status">
          <Text c="dimmed" size="xs" ta="center">
            {t("workbench.grid.image_preview_loading", { title })}
          </Text>
        </Box>
      ) : failed ? (
        <Stack align="flex-start" gap={2}>
          <Text c="red" size="xs">
            {t("workbench.grid.image_preview_failed")}
          </Text>
          <Button
            aria-label={t("workbench.grid.retry_image_preview_title", {
              title,
            })}
            leftSection={<IconRefresh aria-hidden="true" size={13} />}
            onClick={() => setReloadVersion((value) => value + 1)}
            size="compact-sm"
            variant="subtle"
          >
            {t("workbench.grid.retry_image_preview")}
          </Button>
        </Stack>
      ) : source ? (
        <UnstyledButton
          aria-label={t("workbench.grid.open_image_preview", {
            status: t(`status.stage.${status}`),
            title,
          })}
          className={classes.gridImagePreviewButton}
          onClick={() => onInspect(taskId, "image")}
          type="button"
        >
          <Image
            alt=""
            aria-hidden="true"
            className={classes.gridImageThumbnail}
            fit="cover"
            src={source}
          />
        </UnstyledButton>
      ) : null}
    </Box>
  );
}
