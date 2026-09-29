import {
  ActionIcon,
  Alert,
  Button,
  Group,
  Loader,
  Modal,
  NumberInput,
  Paper,
  Slider,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconArrowBackUp,
  IconArrowForwardUp,
  IconArrowsMaximize,
  IconDeviceFloppy,
  IconFlipHorizontal,
  IconFlipVertical,
  IconRefresh,
  IconRestore,
  IconRotate,
  IconRotateClockwise,
  IconZoomIn,
  IconZoomOut,
} from "@tabler/icons-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactElement,
} from "react";
import { useTranslation } from "react-i18next";

import { loadArticleImageEditor } from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import type {
  ArticleImageEditorView,
  ImageCrop,
  ImageEditLineage,
  ImageEditSpec,
} from "./workbenchTypes";
import classes from "./ArticleImageEditor.module.css";

const COORDINATE_SCALE = 1_000_000;
const MIN_ZOOM = 25;
const MAX_ZOOM = 400;
const DEFAULT_SPEC: ImageEditSpec = {
  crop: { height: COORDINATE_SCALE, width: COORDINATE_SCALE, x: 0, y: 0 },
  flipHorizontal: false,
  flipVertical: false,
  rotationQuarterTurns: 0,
};

/** 保存图片编辑时必须原样带回的只读锁和规范。 */
export interface ImageEditorSaveRequest {
  taskId: string;
  expectedRowVersion: number;
  expectedPackageVersion: number;
  expectedImageAttemptId: string;
  expectedEditRevision: number;
  spec: ImageEditSpec;
}

/** 比较规范字段，避免对象引用变化误报未保存修改。 */
function specsEqual(left: ImageEditSpec, right: ImageEditSpec): boolean {
  return (
    left.crop.x === right.crop.x &&
    left.crop.y === right.crop.y &&
    left.crop.width === right.crop.width &&
    left.crop.height === right.crop.height &&
    left.rotationQuarterTurns === right.rotationQuarterTurns &&
    left.flipHorizontal === right.flipHorizontal &&
    left.flipVertical === right.flipVertical
  );
}

/** 判断后台刷新是否已经改变保存必须匹配的任一锁字段。 */
function editorLocksEqual(
  left: ArticleImageEditorView,
  right: ArticleImageEditorView,
): boolean {
  return (
    left.rowVersion === right.rowVersion &&
    left.packageVersion === right.packageVersion &&
    left.imageAttemptId === right.imageAttemptId &&
    left.editRevision === right.editRevision
  );
}

/** 计算居中的固定宽高比裁剪，并保留整数相对坐标。 */
function centeredCrop(
  sourceWidth: number,
  sourceHeight: number,
  ratio: number,
): ImageCrop {
  const sourceRatio = sourceWidth / sourceHeight;
  if (sourceRatio > ratio) {
    const width = Math.max(
      1,
      Math.min(
        COORDINATE_SCALE,
        Math.round((ratio / sourceRatio) * COORDINATE_SCALE),
      ),
    );
    return {
      height: COORDINATE_SCALE,
      width,
      x: Math.floor((COORDINATE_SCALE - width) / 2),
      y: 0,
    };
  }
  const height = Math.max(
    1,
    Math.min(
      COORDINATE_SCALE,
      Math.round((sourceRatio / ratio) * COORDINATE_SCALE),
    ),
  );
  return {
    height,
    width: COORDINATE_SCALE,
    x: 0,
    y: Math.floor((COORDINATE_SCALE - height) / 2),
  };
}

/** 把百万分之一单位转换为编辑面板展示的百分比。 */
function toPercent(value: number): number {
  return Math.round((value / 10_000) * 100) / 100;
}

/** 把用户百分比收敛为确定性整数坐标。 */
function fromPercent(value: string | number): number {
  const numeric = typeof value === "number" ? value : Number(value);
  return Math.round(Math.max(0, Math.min(100, numeric || 0)) * 10_000);
}

/** 用与 Rust 相同的向下/向上规则得到裁剪和旋转后的像素几何。 */
function previewGeometry(
  editor: ArticleImageEditorView,
  spec: ImageEditSpec,
): {
  cropHeight: number;
  cropWidth: number;
  left: number;
  outputHeight: number;
  outputWidth: number;
  rotationTransform: string;
  top: number;
} {
  const left = Math.floor(
    (spec.crop.x * editor.sourceWidth) / COORDINATE_SCALE,
  );
  const top = Math.floor(
    (spec.crop.y * editor.sourceHeight) / COORDINATE_SCALE,
  );
  const right = Math.ceil(
    ((spec.crop.x + spec.crop.width) * editor.sourceWidth) / COORDINATE_SCALE,
  );
  const bottom = Math.ceil(
    ((spec.crop.y + spec.crop.height) * editor.sourceHeight) / COORDINATE_SCALE,
  );
  const cropWidth = Math.max(1, right - left);
  const cropHeight = Math.max(1, bottom - top);
  if (spec.rotationQuarterTurns === 1) {
    return {
      cropHeight,
      cropWidth,
      left,
      outputHeight: cropWidth,
      outputWidth: cropHeight,
      rotationTransform: `matrix(0 1 -1 0 ${cropHeight} 0)`,
      top,
    };
  }
  if (spec.rotationQuarterTurns === 2) {
    return {
      cropHeight,
      cropWidth,
      left,
      outputHeight: cropHeight,
      outputWidth: cropWidth,
      rotationTransform: `matrix(-1 0 0 -1 ${cropWidth} ${cropHeight})`,
      top,
    };
  }
  if (spec.rotationQuarterTurns === 3) {
    return {
      cropHeight,
      cropWidth,
      left,
      outputHeight: cropWidth,
      outputWidth: cropHeight,
      rotationTransform: `matrix(0 -1 1 0 0 ${cropWidth})`,
      top,
    };
  }
  return {
    cropHeight,
    cropWidth,
    left,
    outputHeight: cropHeight,
    outputWidth: cropWidth,
    rotationTransform: "matrix(1 0 0 1 0 0)",
    top,
  };
}

/** 支持查看、无损草稿与显式保存的文章配图编辑器。 */
export function ArticleImageEditor({
  actionLocked,
  active,
  hasImage,
  manualLineage,
  onDirtyChange,
  onFullscreenChange,
  onSave,
  onSavingChange,
  refreshKey,
  taskId,
  title,
}: {
  actionLocked: boolean;
  active: boolean;
  hasImage: boolean;
  manualLineage: ImageEditLineage | null;
  onDirtyChange: (dirty: boolean) => void;
  onFullscreenChange?: (opened: boolean) => void;
  onSave: (request: ImageEditorSaveRequest) => Promise<void>;
  onSavingChange?: (saving: boolean) => void;
  refreshKey: string;
  taskId: string;
  title: string;
}): ReactElement {
  const { i18n, t } = useTranslation("app");
  const [editor, setEditor] = useState<ArticleImageEditorView | null>(null);
  const [spec, setSpec] = useState<ImageEditSpec>(DEFAULT_SPEC);
  const [undoStack, setUndoStack] = useState<ImageEditSpec[]>([]);
  const [redoStack, setRedoStack] = useState<ImageEditSpec[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [reloadRevision, setReloadRevision] = useState(0);
  const [zoom, setZoom] = useState(100);
  const [fitScale, setFitScale] = useState<number | null>(null);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [fullscreen, setFullscreen] = useState(false);
  const previewRef = useRef<HTMLDivElement>(null);
  const fullscreenPreviewRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{
    pointerId: number;
    x: number;
    y: number;
    panX: number;
    panY: number;
  } | null>(null);
  const dirty = editor !== null && !specsEqual(spec, editor.currentSpec);
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;

  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    if (!active || !hasImage) return;
    let current = true;
    setLoading(true);
    setError(null);
    void loadArticleImageEditor(taskId)
      .then((view) => {
        if (!current) return;
        const preserveDraft = dirtyRef.current;
        setEditor((loadedEditor) => {
          if (!preserveDraft || !loadedEditor) return view;
          const locksChanged = !editorLocksEqual(loadedEditor, view);
          return {
            ...loadedEditor,
            canSave: view.canSave && !locksChanged,
            saveBlockedReason:
              view.saveBlockedReason ??
              (locksChanged
                ? {
                    code: "IMAGE_EDIT_CONFLICT",
                    message: t("errors.IMAGE_EDIT_CONFLICT"),
                  }
                : null),
          };
        });
        if (!preserveDraft) {
          setSpec(view.currentSpec);
          setUndoStack([]);
          setRedoStack([]);
          setPan({ x: 0, y: 0 });
          setZoom(100);
          setFitScale(null);
        }
      })
      .catch((reason: unknown) => {
        if (current) {
          setEditor((loadedEditor) =>
            dirtyRef.current && loadedEditor ? loadedEditor : null,
          );
          setError(localizedCommandError(reason, t));
        }
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [active, hasImage, refreshKey, reloadRevision, taskId, t]);

  /** 将一次离散编辑加入撤销栈，并清空重做分支。 */
  const applySpec = (next: ImageEditSpec): void => {
    if (specsEqual(next, spec)) return;
    setUndoStack((current) => [...current, spec]);
    setRedoStack([]);
    setSpec(next);
    setFitScale(null);
    setError(null);
    setFeedback(null);
  };

  const undo = (): void => {
    const previous = undoStack.at(-1);
    if (!previous) return;
    setUndoStack((current) => current.slice(0, -1));
    setRedoStack((current) => [spec, ...current]);
    setSpec(previous);
    setFitScale(null);
  };

  const redo = (): void => {
    const next = redoStack[0];
    if (!next) return;
    setRedoStack((current) => current.slice(1));
    setUndoStack((current) => [...current, spec]);
    setSpec(next);
    setFitScale(null);
  };

  const updateCrop = (field: keyof ImageCrop, value: string | number): void => {
    const nextCrop = { ...spec.crop, [field]: fromPercent(value) };
    if (field === "x") {
      nextCrop.x = Math.min(nextCrop.x, COORDINATE_SCALE - 1);
      nextCrop.width = Math.min(nextCrop.width, COORDINATE_SCALE - nextCrop.x);
    } else if (field === "y") {
      nextCrop.y = Math.min(nextCrop.y, COORDINATE_SCALE - 1);
      nextCrop.height = Math.min(
        nextCrop.height,
        COORDINATE_SCALE - nextCrop.y,
      );
    } else if (field === "width") {
      nextCrop.width = Math.max(
        1,
        Math.min(nextCrop.width, COORDINATE_SCALE - nextCrop.x),
      );
    } else {
      nextCrop.height = Math.max(
        1,
        Math.min(nextCrop.height, COORDINATE_SCALE - nextCrop.y),
      );
    }
    applySpec({ ...spec, crop: nextCrop });
  };

  const save = async (): Promise<void> => {
    if (
      savingRef.current ||
      !dirty ||
      !editor?.canSave ||
      !editor.imageAttemptId
    ) {
      return;
    }
    savingRef.current = true;
    onSavingChange?.(true);
    setSaving(true);
    setError(null);
    setFeedback(null);
    try {
      await onSave({
        expectedEditRevision: editor.editRevision,
        expectedImageAttemptId: editor.imageAttemptId,
        expectedPackageVersion: editor.packageVersion,
        expectedRowVersion: editor.rowVersion,
        spec,
        taskId,
      });
      setEditor({ ...editor, currentSpec: spec });
      setUndoStack([]);
      setRedoStack([]);
      setFeedback(t("details.image_editor.saved"));
      setReloadRevision((current) => current + 1);
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      savingRef.current = false;
      onSavingChange?.(false);
      setSaving(false);
    }
  };

  const startPan = (event: ReactPointerEvent<HTMLDivElement>): void => {
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      panX: pan.x,
      panY: pan.y,
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
    };
  };

  const movePan = (event: ReactPointerEvent<HTMLDivElement>): void => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setPan({
      x: drag.panX + event.clientX - drag.x,
      y: drag.panY + event.clientY - drag.y,
    });
  };

  const stopPan = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null;
  };

  /** 明确放弃当前草稿并重新读取权威图片、规范和保存锁。 */
  const reloadFromAuthority = (): void => {
    setEditor(null);
    setUndoStack([]);
    setRedoStack([]);
    setError(null);
    setFeedback(null);
    setPan({ x: 0, y: 0 });
    setFitScale(null);
    setReloadRevision((current) => current + 1);
  };

  const geometry = useMemo(
    () => (editor ? previewGeometry(editor, spec) : null),
    [editor, spec],
  );
  const outputRatio = geometry
    ? geometry.outputWidth / geometry.outputHeight
    : 16 / 9;
  const renderedZoom = fitScale ?? zoom;

  /** 打开最高层查看窗口前同步锁住外层弹窗的关闭入口。 */
  const openFullscreen = (): void => {
    onFullscreenChange?.(true);
    setFullscreen(true);
  };

  /** 关闭最高层查看窗口并把外层编辑弹窗恢复为可关闭状态。 */
  const closeFullscreen = (): void => {
    onFullscreenChange?.(false);
    setFullscreen(false);
  };

  /** 以当前预览容器计算实际适应比例，同时保持公开缩放范围。 */
  const fitPreview = (isFullscreen: boolean): void => {
    if (!geometry) return;
    const bounds = (
      isFullscreen ? fullscreenPreviewRef : previewRef
    ).current?.getBoundingClientRect();
    if (!bounds || bounds.width <= 0 || bounds.height <= 0) return;
    const raw =
      Math.min(
        bounds.width / geometry.outputWidth,
        bounds.height / geometry.outputHeight,
      ) * 100;
    setFitScale(Math.max(1, Math.min(MAX_ZOOM, raw)));
    setPan({ x: 0, y: 0 });
  };

  const preview = (isFullscreen: boolean): ReactElement | null =>
    editor && geometry ? (
      <div
        aria-label={t("details.image_editor.pan_area")}
        className={isFullscreen ? classes.fullscreenPreview : classes.preview}
        data-testid={
          isFullscreen ? "image-editor-fullscreen" : "image-editor-preview"
        }
        onPointerCancel={stopPan}
        onPointerDown={startPan}
        onPointerMove={movePan}
        onPointerUp={stopPan}
        ref={isFullscreen ? fullscreenPreviewRef : previewRef}
        role="application"
        style={{ aspectRatio: String(outputRatio) }}
      >
        <svg
          aria-label={t("details.generated_image_alt", { title })}
          className={classes.previewImage}
          role="img"
          style={{
            height: `${geometry.outputHeight}px`,
            transform: `translate(calc(-50% + ${pan.x}px), calc(-50% + ${pan.y}px)) scale(${renderedZoom / 100})`,
            width: `${geometry.outputWidth}px`,
          }}
          viewBox={`0 0 ${geometry.outputWidth} ${geometry.outputHeight}`}
          xmlns="http://www.w3.org/2000/svg"
        >
          <g
            transform={
              spec.flipVertical
                ? `matrix(1 0 0 -1 0 ${geometry.outputHeight})`
                : undefined
            }
          >
            <g
              transform={
                spec.flipHorizontal
                  ? `matrix(-1 0 0 1 ${geometry.outputWidth} 0)`
                  : undefined
              }
            >
              <g transform={geometry.rotationTransform}>
                <image
                  height={editor.sourceHeight}
                  href={editor.imageDataUrl}
                  preserveAspectRatio="none"
                  width={editor.sourceWidth}
                  x={-geometry.left}
                  y={-geometry.top}
                />
              </g>
            </g>
          </g>
        </svg>
      </div>
    ) : null;

  if (!hasImage) {
    return <Alert color="gray">{t("details.image_empty")}</Alert>;
  }
  if (loading && !editor) {
    return (
      <Group gap="xs" role="status">
        <Loader size="sm" />
        <Text c="dimmed">{t("details.image_loading")}</Text>
      </Group>
    );
  }
  if (!editor) {
    return (
      <Stack gap="sm">
        {error ? <Alert color="red">{error}</Alert> : null}
        <Button
          leftSection={<IconRefresh aria-hidden="true" size={16} />}
          onClick={() => setReloadRevision((current) => current + 1)}
          variant="default"
        >
          {t("common.retry")}
        </Button>
      </Stack>
    );
  }

  return (
    <Stack data-testid="article-image-editor" gap="md">
      <Paper className={classes.viewer} p="sm" radius="md" withBorder>
        {preview(false)}
        <Group gap="xs" justify="space-between" mt="sm" wrap="wrap">
          <Group gap={4}>
            <Tooltip label={t("details.image_editor.zoom_out")}>
              <ActionIcon
                aria-label={t("details.image_editor.zoom_out")}
                disabled={fitScale === null && zoom <= MIN_ZOOM}
                onClick={() => {
                  setFitScale(null);
                  setZoom(Math.max(MIN_ZOOM, renderedZoom - 25));
                }}
                variant="default"
              >
                <IconZoomOut size={17} />
              </ActionIcon>
            </Tooltip>
            <Slider
              aria-label={t("details.image_editor.zoom")}
              className={classes.zoomSlider}
              label={(value) => `${value}%`}
              max={MAX_ZOOM}
              min={MIN_ZOOM}
              onChange={(value) => {
                setFitScale(null);
                setZoom(value);
              }}
              step={5}
              value={Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, renderedZoom))}
            />
            <Tooltip label={t("details.image_editor.zoom_in")}>
              <ActionIcon
                aria-label={t("details.image_editor.zoom_in")}
                disabled={renderedZoom >= MAX_ZOOM}
                onClick={() => {
                  setFitScale(null);
                  setZoom(
                    Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, renderedZoom) + 25),
                  );
                }}
                variant="default"
              >
                <IconZoomIn size={17} />
              </ActionIcon>
            </Tooltip>
            <Text c="dimmed" size="xs" w={44}>
              {Math.round(renderedZoom)}%
            </Text>
          </Group>
          <Group gap={4}>
            <Button
              onClick={() => {
                setFitScale(null);
                setZoom(100);
              }}
              size="compact-xs"
              variant="default"
            >
              {t("details.image_editor.actual_size")}
            </Button>
            <Button
              onClick={() => fitPreview(false)}
              size="compact-xs"
              variant="default"
            >
              {t("details.image_editor.fit")}
            </Button>
            <Button
              onClick={() => {
                setFitScale(null);
                setZoom(100);
                setPan({ x: 0, y: 0 });
              }}
              size="compact-xs"
              variant="default"
            >
              {t("details.image_editor.reset_view")}
            </Button>
            <Tooltip label={t("details.image_editor.fullscreen")}>
              <ActionIcon
                aria-label={t("details.image_editor.fullscreen")}
                onClick={openFullscreen}
                variant="default"
              >
                <IconArrowsMaximize size={17} />
              </ActionIcon>
            </Tooltip>
          </Group>
        </Group>
      </Paper>

      <Paper p="sm" radius="md" withBorder>
        <Stack gap="sm">
          <Text fw={600} size="sm">
            {t("details.image_editor.crop")}
          </Text>
          <Group gap="xs" wrap="wrap">
            {[
              ["1:1", 1],
              ["4:3", 4 / 3],
              ["16:9", 16 / 9],
            ].map(([label, ratio]) => (
              <Button
                key={String(label)}
                onClick={() =>
                  applySpec({
                    ...spec,
                    crop: centeredCrop(
                      editor.sourceWidth,
                      editor.sourceHeight,
                      Number(ratio),
                    ),
                  })
                }
                size="compact-sm"
                variant="default"
              >
                {label}
              </Button>
            ))}
            <Button
              onClick={() => applySpec({ ...spec, crop: DEFAULT_SPEC.crop })}
              size="compact-sm"
              variant="default"
            >
              {t("details.image_editor.free_crop_reset")}
            </Button>
          </Group>
          <div className={classes.cropGrid}>
            {(["x", "y", "width", "height"] as const).map((field) => (
              <NumberInput
                allowNegative={false}
                decimalScale={2}
                key={field}
                label={t(`details.image_editor.crop_${field}`)}
                max={
                  field === "x" || field === "y"
                    ? 99.99
                    : toPercent(
                        COORDINATE_SCALE -
                          (field === "width" ? spec.crop.x : spec.crop.y),
                      )
                }
                min={field === "width" || field === "height" ? 0.01 : 0}
                onChange={(value) => updateCrop(field, value)}
                suffix="%"
                value={toPercent(spec.crop[field])}
              />
            ))}
          </div>
          <Group gap="xs" wrap="wrap">
            <Button
              leftSection={<IconRotate aria-hidden="true" size={16} />}
              onClick={() =>
                applySpec({
                  ...spec,
                  rotationQuarterTurns: ((spec.rotationQuarterTurns + 3) %
                    4) as 0 | 1 | 2 | 3,
                })
              }
              size="compact-sm"
              variant="default"
            >
              {t("details.image_editor.rotate_left")}
            </Button>
            <Button
              leftSection={<IconRotateClockwise aria-hidden="true" size={16} />}
              onClick={() =>
                applySpec({
                  ...spec,
                  rotationQuarterTurns: ((spec.rotationQuarterTurns + 1) %
                    4) as 0 | 1 | 2 | 3,
                })
              }
              size="compact-sm"
              variant="default"
            >
              {t("details.image_editor.rotate_right")}
            </Button>
            <Button
              leftSection={<IconFlipHorizontal aria-hidden="true" size={16} />}
              onClick={() =>
                applySpec({ ...spec, flipHorizontal: !spec.flipHorizontal })
              }
              size="compact-sm"
              variant={spec.flipHorizontal ? "filled" : "default"}
            >
              {t("details.image_editor.flip_horizontal")}
            </Button>
            <Button
              leftSection={<IconFlipVertical aria-hidden="true" size={16} />}
              onClick={() =>
                applySpec({ ...spec, flipVertical: !spec.flipVertical })
              }
              size="compact-sm"
              variant={spec.flipVertical ? "filled" : "default"}
            >
              {t("details.image_editor.flip_vertical")}
            </Button>
          </Group>
        </Stack>
      </Paper>

      <Group justify="space-between" wrap="wrap">
        <Group gap="xs">
          <Button
            disabled={undoStack.length === 0}
            leftSection={<IconArrowBackUp aria-hidden="true" size={16} />}
            onClick={undo}
            variant="default"
          >
            {t("details.image_editor.undo")}
          </Button>
          <Button
            disabled={redoStack.length === 0}
            leftSection={<IconArrowForwardUp aria-hidden="true" size={16} />}
            onClick={redo}
            variant="default"
          >
            {t("details.image_editor.redo")}
          </Button>
          <Button
            leftSection={<IconRestore aria-hidden="true" size={16} />}
            onClick={() => applySpec(DEFAULT_SPEC)}
            variant="default"
          >
            {t("details.image_editor.restore_original")}
          </Button>
        </Group>
        <Button
          disabled={actionLocked || !editor.canSave || !dirty}
          leftSection={<IconDeviceFloppy aria-hidden="true" size={17} />}
          loading={saving}
          onClick={() => void save()}
        >
          {t("details.image_editor.save")}
        </Button>
      </Group>

      {editor.saveBlockedReason ? (
        <Alert color="yellow">
          {t(`errors.${editor.saveBlockedReason.code}`, {
            defaultValue: editor.saveBlockedReason.message,
          })}
        </Alert>
      ) : null}
      {error ? <Alert color="red">{error}</Alert> : null}
      {error || editor.saveBlockedReason?.code === "IMAGE_EDIT_CONFLICT" ? (
        <Button onClick={reloadFromAuthority} variant="default">
          {t("details.image_editor.reload_discard")}
        </Button>
      ) : null}
      {feedback ? (
        <Alert color="green" role="status">
          {feedback}
        </Alert>
      ) : null}
      {manualLineage ? (
        <Paper p="sm" radius="md" withBorder>
          <Text c="dimmed" size="xs">
            {t("details.image_editor.manual_lineage")}
          </Text>
          <Text size="xs">
            {t("details.image_editor.manual_lineage_summary", {
              editId: manualLineage.editId,
              sourceAttemptId: manualLineage.sourceAttemptId,
            })}
          </Text>
          <Text size="xs">
            {t("details.image_editor.manual_lineage_time", {
              time: new Date(manualLineage.editedAtMs).toLocaleString(
                i18n.resolvedLanguage,
              ),
            })}
          </Text>
          <Text size="xs">
            {t("details.image_editor.manual_lineage_spec", {
              height: toPercent(manualLineage.spec.crop.height),
              horizontal: manualLineage.spec.flipHorizontal
                ? t("details.image_editor.state_on")
                : t("details.image_editor.state_off"),
              rotation: manualLineage.spec.rotationQuarterTurns * 90,
              vertical: manualLineage.spec.flipVertical
                ? t("details.image_editor.state_on")
                : t("details.image_editor.state_off"),
              width: toPercent(manualLineage.spec.crop.width),
              x: toPercent(manualLineage.spec.crop.x),
              y: toPercent(manualLineage.spec.crop.y),
            })}
          </Text>
        </Paper>
      ) : null}
      <Text c="dimmed" size="xs">
        {t("details.image_editor.safety_note")}
      </Text>

      <Modal
        fullScreen
        onClose={closeFullscreen}
        opened={fullscreen}
        title={t("details.image_editor.fullscreen_title", { title })}
      >
        <Stack className={classes.fullscreenLayout} gap="md">
          {preview(true)}
          <Group justify="center">
            <Button
              onClick={() => {
                setFitScale(null);
                setZoom(100);
              }}
              variant="default"
            >
              {t("details.image_editor.actual_size")}
            </Button>
            <Button onClick={() => fitPreview(true)} variant="default">
              {t("details.image_editor.fit")}
            </Button>
            <Button
              onClick={() => {
                setFitScale(null);
                setZoom(100);
                setPan({ x: 0, y: 0 });
              }}
              variant="default"
            >
              {t("details.image_editor.reset_view")}
            </Button>
            <Button onClick={closeFullscreen}>{t("common.close")}</Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
