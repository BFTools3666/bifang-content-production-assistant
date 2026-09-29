import {
  Alert,
  Button,
  Group,
  Loader,
  Modal,
  Paper,
  Stack,
  Text,
  UnstyledButton,
} from "@mantine/core";
import { IconEdit, IconRefresh } from "@tabler/icons-react";
import {
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
} from "react";
import { useTranslation } from "react-i18next";

import {
  ArticleImageEditor,
  type ImageEditorSaveRequest,
} from "./ArticleImageEditor";
import { loadArticleImage } from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import type { ImageEditLineage } from "./workbenchTypes";
import classes from "./ArticleImageEditorModal.module.css";

/** 抽屉预览读取状态；刷新时允许保留已有图片，避免界面闪空。 */
interface ImagePreviewState {
  dataUrl: string | null;
  error: string | null;
  loading: boolean;
}

/** 图片预览与独立高级编辑弹窗之间的展示层契约。 */
export interface ArticleImageEditorModalProps {
  actionLocked: boolean;
  active: boolean;
  hasImage: boolean;
  manualLineage: ImageEditLineage | null;
  onLayerChange?: (opened: boolean) => void;
  onSave: (request: ImageEditorSaveRequest) => Promise<void>;
  refreshKey: string;
  taskId: string;
  title: string;
}

/** 在抽屉内只展示配图预览，把所有高级操作隔离到独立大弹窗。 */
export function ArticleImageEditorModal({
  actionLocked,
  active,
  hasImage,
  manualLineage,
  onLayerChange,
  onSave,
  refreshKey,
  taskId,
  title,
}: ArticleImageEditorModalProps): ReactElement {
  const { t } = useTranslation("app");
  const [editorOpened, setEditorOpened] = useState(false);
  const [discardOpened, setDiscardOpened] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [fullscreenOpened, setFullscreenOpened] = useState(false);
  const [previewRevision, setPreviewRevision] = useState(0);
  const [preview, setPreview] = useState<ImagePreviewState>({
    dataUrl: null,
    error: null,
    loading: false,
  });
  const lastTriggerRef = useRef<HTMLButtonElement | null>(null);
  const lastEditorFocusRef = useRef<HTMLElement | null>(null);
  const layerOpened = editorOpened || discardOpened || fullscreenOpened;

  useEffect(() => {
    onLayerChange?.(layerOpened);
  }, [layerOpened, onLayerChange]);

  useEffect(
    () => () => {
      onLayerChange?.(false);
    },
    [onLayerChange],
  );

  useEffect(() => {
    if (!active || !hasImage) return;
    let current = true;
    setPreview((value) => ({ ...value, error: null, loading: true }));
    void loadArticleImage(taskId)
      .then((dataUrl) => {
        if (current) setPreview({ dataUrl, error: null, loading: false });
      })
      .catch((reason: unknown) => {
        if (!current) return;
        setPreview((value) => ({
          ...value,
          error: localizedCommandError(reason, t),
          loading: false,
        }));
      });
    return () => {
      current = false;
    };
  }, [active, hasImage, previewRevision, refreshKey, taskId, t]);

  useEffect(() => {
    setEditorOpened(false);
    setDiscardOpened(false);
    setDirty(false);
    setSaving(false);
    setFullscreenOpened(false);
  }, [taskId]);

  /** 从图片或显式按钮打开编辑器，并以干净草稿开始本次弹窗会话。 */
  const openEditor = (event: ReactMouseEvent<HTMLButtonElement>): void => {
    lastTriggerRef.current = event.currentTarget;
    onLayerChange?.(true);
    setDirty(false);
    setSaving(false);
    setFullscreenOpened(false);
    setEditorOpened(true);
  };

  /** 在编辑弹窗完全关闭后把键盘焦点还给本次实际使用的触发控件。 */
  const restoreTriggerFocus = (): void => {
    window.setTimeout(() => lastTriggerRef.current?.focus(), 0);
  };

  /** 只有干净且没有保存中的草稿可以直接关闭，否则转入放弃确认。 */
  const requestClose = (): void => {
    if (saving) return;
    if (dirty) {
      lastEditorFocusRef.current =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      setDiscardOpened(true);
      return;
    }
    setEditorOpened(false);
    setFullscreenOpened(false);
    restoreTriggerFocus();
  };

  /** 明确放弃草稿后卸载编辑器，下一次打开会重新读取权威图片。 */
  const discardAndClose = (): void => {
    setDiscardOpened(false);
    setDirty(false);
    setEditorOpened(false);
    setFullscreenOpened(false);
    restoreTriggerFocus();
  };

  /** 取消放弃时回到刚才的编辑控件，避免焦点落回底层 Drawer。 */
  const keepEditing = (): void => {
    setDiscardOpened(false);
    window.setTimeout(() => lastEditorFocusRef.current?.focus(), 0);
  };

  if (!hasImage) {
    return <Alert color="gray">{t("details.image_empty")}</Alert>;
  }

  return (
    <Stack data-testid="article-image-editor-launcher" gap="sm">
      <Paper className={classes.previewSurface} p="sm" radius="md" withBorder>
        {preview.dataUrl ? (
          <UnstyledButton
            aria-label={t("details.image_editor.open_editor_named", { title })}
            className={classes.previewTrigger}
            onClick={openEditor}
          >
            <img
              alt={t("details.generated_image_alt", { title })}
              className={classes.previewImage}
              src={preview.dataUrl}
            />
          </UnstyledButton>
        ) : preview.loading ? (
          <Group className={classes.previewPlaceholder} gap="xs" role="status">
            <Loader size="sm" />
            <Text c="dimmed">{t("details.image_loading")}</Text>
          </Group>
        ) : preview.error ? (
          <Stack className={classes.previewPlaceholder} gap="sm">
            <Alert color="red">{preview.error}</Alert>
            <Button
              leftSection={<IconRefresh aria-hidden="true" size={16} />}
              onClick={() => setPreviewRevision((value) => value + 1)}
              variant="default"
            >
              {t("common.retry")}
            </Button>
          </Stack>
        ) : (
          <Alert color="gray">{t("details.image_empty")}</Alert>
        )}

        {preview.dataUrl && preview.loading ? (
          <Group gap="xs" mt="sm" role="status">
            <Loader size="xs" />
            <Text c="dimmed" size="xs">
              {t("details.image_loading")}
            </Text>
          </Group>
        ) : null}
        {preview.dataUrl && preview.error ? (
          <Group align="flex-start" gap="xs" mt="sm" wrap="wrap">
            <Alert className={classes.refreshError} color="red">
              {preview.error}
            </Alert>
            <Button
              leftSection={<IconRefresh aria-hidden="true" size={16} />}
              onClick={() => setPreviewRevision((value) => value + 1)}
              variant="default"
            >
              {t("common.retry")}
            </Button>
          </Group>
        ) : null}

        <Group align="flex-end" justify="space-between" mt="sm" wrap="wrap">
          <Stack gap={2} maw={680}>
            <Text fw={600} size="sm">
              {t("details.image_editor.preview_title")}
            </Text>
            <Text c="dimmed" size="xs">
              {t("details.image_editor.preview_hint")}
            </Text>
          </Stack>
          <Button
            disabled={preview.dataUrl === null}
            leftSection={<IconEdit aria-hidden="true" size={17} />}
            onClick={openEditor}
          >
            {t("details.image_editor.open_editor")}
          </Button>
        </Group>
      </Paper>

      <Modal
        centered
        classNames={{
          body: classes.editorModalBody,
          content: classes.editorModalContent,
        }}
        closeButtonProps={{
          "aria-label": t("details.image_editor.close_editor"),
          disabled: saving,
        }}
        closeOnClickOutside={!saving && !discardOpened && !fullscreenOpened}
        closeOnEscape={!saving && !discardOpened && !fullscreenOpened}
        onClose={requestClose}
        opened={editorOpened}
        overlayProps={{ id: "image-editor-modal-overlay" }}
        returnFocus={false}
        size="calc(100vw - 32px)"
        title={t("details.image_editor.editor_title", { title })}
        transitionProps={{ duration: 0 }}
      >
        <Stack gap="md">
          <Text c="dimmed" size="sm">
            {t("details.image_editor.editor_hint")}
          </Text>
          <ArticleImageEditor
            actionLocked={actionLocked}
            active={active && editorOpened}
            hasImage={hasImage}
            manualLineage={manualLineage}
            onDirtyChange={setDirty}
            onFullscreenChange={setFullscreenOpened}
            onSave={onSave}
            onSavingChange={setSaving}
            refreshKey={refreshKey}
            taskId={taskId}
            title={title}
          />
        </Stack>
      </Modal>

      <Modal
        centered
        onClose={keepEditing}
        opened={discardOpened}
        returnFocus={false}
        title={t("details.image_editor.discard_title")}
        transitionProps={{ duration: 0 }}
      >
        <Stack gap="md">
          <Text>{t("details.image_editor.discard_message")}</Text>
          <Group justify="flex-end">
            <Button onClick={keepEditing} variant="default">
              {t("details.image_editor.keep_editing")}
            </Button>
            <Button color="red" onClick={discardAndClose}>
              {t("details.image_editor.discard")}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
