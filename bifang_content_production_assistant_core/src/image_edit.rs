//! 配图人工编辑的规范化参数、乐观锁和领域状态转换。

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{ArticleTask, ReviewStatus, StageStatus, WorkflowStage};

/// 相对裁剪坐标使用的固定整数精度，避免跨语言浮点漂移。
pub const IMAGE_EDIT_COORDINATE_SCALE: u32 = 1_000_000;

/// 相对于 AI 原图的裁剪矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ImageCrop {
    /// 左边界，范围为 `0..IMAGE_EDIT_COORDINATE_SCALE`。
    pub x: u32,
    /// 上边界，范围为 `0..IMAGE_EDIT_COORDINATE_SCALE`。
    pub y: u32,
    /// 裁剪宽度，必须大于零且不能越过右边界。
    pub width: u32,
    /// 裁剪高度，必须大于零且不能越过下边界。
    pub height: u32,
}

impl Default for ImageCrop {
    /// 返回覆盖整张 AI 原图的默认裁剪矩形。
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: IMAGE_EDIT_COORDINATE_SCALE,
            height: IMAGE_EDIT_COORDINATE_SCALE,
        }
    }
}

/// 一次保存相对于 AI 原图执行的完整编辑规范。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ImageEditSpec {
    /// 首先执行的相对裁剪。
    pub crop: ImageCrop,
    /// 顺时针旋转的 90 度次数，只允许 0、1、2、3。
    pub rotation_quarter_turns: u8,
    /// 旋转后是否水平翻转。
    pub flip_horizontal: bool,
    /// 旋转后是否垂直翻转。
    pub flip_vertical: bool,
}

impl ImageEditSpec {
    /// 校验固定白名单与裁剪边界。
    pub fn validate(&self) -> Result<(), ImageEditError> {
        if self.rotation_quarter_turns > 3 {
            return Err(ImageEditError::InvalidRotation);
        }
        let crop = self.crop;
        if crop.width == 0
            || crop.height == 0
            || crop.x >= IMAGE_EDIT_COORDINATE_SCALE
            || crop.y >= IMAGE_EDIT_COORDINATE_SCALE
            || crop
                .x
                .checked_add(crop.width)
                .is_none_or(|right| right > IMAGE_EDIT_COORDINATE_SCALE)
            || crop
                .y
                .checked_add(crop.height)
                .is_none_or(|bottom| bottom > IMAGE_EDIT_COORDINATE_SCALE)
        {
            return Err(ImageEditError::InvalidCrop);
        }
        Ok(())
    }
}

/// 一次人工配图保存的独立血缘；AI 生成血缘继续保留在 `image_lineage`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ImageEditLineage {
    /// 被编辑 AI 原图所属的真实图片尝试 ID。
    pub source_attempt_id: String,
    /// 本次人工编辑的 UUID，不伪造阶段尝试。
    pub edit_id: String,
    /// 保存完成时间，使用 Unix 毫秒。
    pub edited_at_ms: i64,
    /// 已执行的规范化编辑参数。
    pub spec: ImageEditSpec,
}

/// 写入前由只读接口返回、保存时必须原样带回的乐观锁。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ImageEditLock {
    /// 读取编辑器时的行版本。
    pub row_version: u64,
    /// 读取编辑器时的文章包版本。
    pub package_version: u32,
    /// 读取编辑器时的 AI 图片尝试 ID。
    pub image_attempt_id: String,
    /// 读取编辑器时的人工编辑修订号。
    pub edit_revision: u64,
}

/// 只读编辑器取得的领域上下文；本地引用只允许由 Rust 适配器继续消费。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageEditContext {
    /// 始终指向 AI 原图的本地引用。
    pub original_image_ref: String,
    /// 当前文章包对应的编辑规范。
    pub spec: ImageEditSpec,
    /// 保存时必须匹配的完整乐观锁。
    pub lock: Option<ImageEditLock>,
    /// 保存被禁止时的稳定原因；查看原图仍然允许。
    pub blocked_reason: Option<ImageEditBlockedReason>,
}

/// 当前文章状态阻止配图保存的稳定原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageEditBlockedReason {
    /// 图片阶段已因上游变化而过期。
    ImageStale,
    /// 当前文章存在排队或运行中的阶段。
    ArticleBusy,
    /// 当前图片阶段并非完整成功状态。
    ImageNotReady,
    /// 当前图片缺少可验证的真实 AI 尝试血缘。
    MissingImageLineage,
}

impl ImageEditBlockedReason {
    /// 返回供 GUI 分支处理的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::ImageStale => "IMAGE_EDIT_STALE",
            Self::ArticleBusy => "IMAGE_EDIT_ARTICLE_BUSY",
            Self::ImageNotReady => "IMAGE_EDIT_NOT_READY",
            Self::MissingImageLineage => "IMAGE_EDIT_LINEAGE_MISSING",
        }
    }

    /// 返回不含路径和内部状态的用户可见说明。
    pub const fn message(self) -> &'static str {
        match self {
            Self::ImageStale => "配图已因上游内容变化而过期，请先重新生成配图",
            Self::ArticleBusy => "当前文章正在排队或运行，暂时不能保存配图编辑",
            Self::ImageNotReady => "当前配图尚未完整生成，暂时不能保存编辑",
            Self::MissingImageLineage => "当前配图缺少可验证的 AI 生成血缘，不能保存编辑",
        }
    }
}

/// 配图编辑领域拒绝原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageEditError {
    /// 裁剪矩形为零、溢出或越界。
    InvalidCrop,
    /// 旋转值不在 0、1、2、3 白名单内。
    InvalidRotation,
    /// 当前文章没有可查看的原图引用。
    MissingOriginal,
    /// 当前状态不允许保存。
    Blocked(ImageEditBlockedReason),
    /// 行版本已变化。
    StaleRowVersion,
    /// 包版本已变化。
    StalePackageVersion,
    /// AI 图片尝试已变化。
    StaleImageAttempt,
    /// 人工编辑修订已变化。
    StaleEditRevision,
    /// 新受管引用或编辑 ID 为空。
    InvalidDerivedArtifact,
}

impl ImageEditError {
    /// 返回供适配器映射的稳定错误码。
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidCrop => "IMAGE_EDIT_INVALID_CROP",
            Self::InvalidRotation => "IMAGE_EDIT_INVALID_ROTATION",
            Self::MissingOriginal => "IMAGE_EDIT_ORIGINAL_MISSING",
            Self::Blocked(reason) => reason.code(),
            Self::StaleRowVersion
            | Self::StalePackageVersion
            | Self::StaleImageAttempt
            | Self::StaleEditRevision => "IMAGE_EDIT_CONFLICT",
            Self::InvalidDerivedArtifact => "IMAGE_EDIT_INVALID_ARTIFACT",
        }
    }
}

impl fmt::Display for ImageEditError {
    /// 输出不泄露内部路径的稳定中文错误说明。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCrop => formatter.write_str("裁剪范围必须非零且位于 AI 原图内"),
            Self::InvalidRotation => formatter.write_str("旋转只能使用 90 度的整数倍"),
            Self::MissingOriginal => formatter.write_str("找不到可恢复的 AI 原图"),
            Self::Blocked(reason) => formatter.write_str(reason.message()),
            Self::StaleRowVersion
            | Self::StalePackageVersion
            | Self::StaleImageAttempt
            | Self::StaleEditRevision => {
                formatter.write_str("文章或配图已变化，请重新载入编辑器后再保存")
            }
            Self::InvalidDerivedArtifact => formatter.write_str("新的配图产物信息无效"),
        }
    }
}

impl Error for ImageEditError {}

impl ArticleTask {
    /// 返回可查看的 AI 原图和当前编辑规范；状态不允许保存时仍保留只读结果。
    pub fn image_edit_context(&self) -> Result<ImageEditContext, ImageEditError> {
        let original_image_ref = self
            .artifacts
            .original_image_ref
            .as_ref()
            .filter(|reference| !reference.trim().is_empty())
            .cloned()
            .ok_or(ImageEditError::MissingOriginal)?;
        let lineage = self.artifacts.image_lineage.as_ref();
        let spec = self
            .artifacts
            .image_edit_lineage
            .as_ref()
            .map_or_else(ImageEditSpec::default, |lineage| lineage.spec.clone());
        spec.validate()?;
        let blocked_reason = if self.stages.image.status == StageStatus::Stale {
            Some(ImageEditBlockedReason::ImageStale)
        } else if WorkflowStage::ordered().into_iter().any(|stage| {
            matches!(
                self.stages.get(stage).status,
                StageStatus::Queued | StageStatus::Running
            )
        }) {
            Some(ImageEditBlockedReason::ArticleBusy)
        } else if self.stages.image.status != StageStatus::Succeeded
            || self.artifacts.image_ref.is_none()
        {
            Some(ImageEditBlockedReason::ImageNotReady)
        } else if lineage.is_none()
            || lineage.is_some_and(|lineage| lineage.attempt_id.trim().is_empty())
            || self
                .artifacts
                .image_edit_lineage
                .as_ref()
                .is_some_and(|manual| {
                    lineage.is_none_or(|ai| manual.source_attempt_id != ai.attempt_id)
                })
        {
            Some(ImageEditBlockedReason::MissingImageLineage)
        } else {
            None
        };
        let lock = lineage.map(|lineage| ImageEditLock {
            row_version: self.row_version,
            package_version: self.package_version,
            image_attempt_id: lineage.attempt_id.clone(),
            edit_revision: self.artifacts.image_edit_revision,
        });
        Ok(ImageEditContext {
            original_image_ref,
            spec,
            lock,
            blocked_reason,
        })
    }

    /// 在任何像素写入前校验参数、当前状态和全部乐观锁。
    pub fn prepare_image_edit(
        &self,
        lock: &ImageEditLock,
        spec: &ImageEditSpec,
    ) -> Result<ImageEditContext, ImageEditError> {
        spec.validate()?;
        let context = self.image_edit_context()?;
        if let Some(reason) = context.blocked_reason {
            return Err(ImageEditError::Blocked(reason));
        }
        let current_lock = context.lock.as_ref().ok_or(ImageEditError::Blocked(
            ImageEditBlockedReason::MissingImageLineage,
        ))?;
        if lock.row_version != current_lock.row_version {
            return Err(ImageEditError::StaleRowVersion);
        }
        if lock.package_version != current_lock.package_version {
            return Err(ImageEditError::StalePackageVersion);
        }
        if lock.image_attempt_id != current_lock.image_attempt_id {
            return Err(ImageEditError::StaleImageAttempt);
        }
        if lock.edit_revision != current_lock.edit_revision {
            return Err(ImageEditError::StaleEditRevision);
        }
        Ok(context)
    }

    /// 原子提交一个从 AI 原图重新计算的人工编辑产物，并创建新的派生文章包。
    pub fn commit_image_edit(
        &mut self,
        lock: &ImageEditLock,
        spec: ImageEditSpec,
        derived_image_ref: String,
        edit_id: String,
        now_ms: i64,
    ) -> Result<(), ImageEditError> {
        let context = self.prepare_image_edit(lock, &spec)?;
        if derived_image_ref.trim().is_empty() || edit_id.trim().is_empty() {
            return Err(ImageEditError::InvalidDerivedArtifact);
        }
        self.begin_derived_version(now_ms);
        self.artifacts.original_image_ref = Some(context.original_image_ref);
        self.artifacts.image_ref = Some(derived_image_ref);
        self.artifacts.image_edit_revision = self.artifacts.image_edit_revision.saturating_add(1);
        self.artifacts.image_edit_lineage = Some(ImageEditLineage {
            source_attempt_id: lock.image_attempt_id.clone(),
            edit_id,
            edited_at_ms: now_ms,
            spec,
        });
        let image_stage = self.stages.get_mut(WorkflowStage::Image);
        image_stage.status = StageStatus::Succeeded;
        image_stage.artifact_version = Some(self.package_version);
        image_stage.failure = None;
        image_stage.active_attempt_id = None;
        self.review_status = if self.all_stages_succeeded() && self.artifacts.is_complete() {
            ReviewStatus::Pending
        } else {
            ReviewStatus::Draft
        };
        self.touch(now_ms);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ArticleTask, ArtifactLineage, ArtifactOutput, ImageCrop, ImageEditError, ImageEditSpec,
        ReviewStatus, StageStatus, WorkflowStage,
    };

    /// 为固定 SOP 构造与当前包一致的真实阶段血缘。
    fn lineage(task: &ArticleTask, stage: WorkflowStage, attempt_id: String) -> ArtifactLineage {
        ArtifactLineage {
            package_version: task.package_version,
            source_article_version: (stage != WorkflowStage::Article)
                .then_some(task.stages.article.artifact_version)
                .flatten(),
            source_summary_version: (stage == WorkflowStage::Image)
                .then_some(task.stages.summary.artifact_version)
                .flatten(),
            template_id: (stage != WorkflowStage::Format).then(|| "template".into()),
            template_version: (stage != WorkflowStage::Format).then_some(1),
            provider_id: (stage != WorkflowStage::Format).then(|| "provider".into()),
            provider_revision: (stage != WorkflowStage::Format).then_some(1),
            model: (stage != WorkflowStage::Format).then(|| "model".into()),
            attempt_id,
        }
    }

    /// 生成完整待审核文章，供编辑状态与历史断言复用。
    fn complete_article() -> ArticleTask {
        let mut task = ArticleTask::new("task", "标题", 0);
        task.queue_initial(1).unwrap();
        for output in [
            ArtifactOutput::Article("# 正文".into()),
            ArtifactOutput::Summary("简介".into()),
            ArtifactOutput::Image("/managed/ai.png".into()),
            ArtifactOutput::Format("<h1>正文</h1>".into()),
        ] {
            let stage = output.stage();
            let attempt_id = task.start_stage(stage, 2).unwrap();
            let lineage = lineage(&task, stage, attempt_id);
            task.complete_stage(stage, output, lineage, 3).unwrap();
        }
        task
    }

    /// 越界、零面积和非白名单旋转必须在写文件前失败关闭。
    #[test]
    fn normalized_spec_rejects_invalid_crop_and_rotation() {
        let mut zero = ImageEditSpec::default();
        zero.crop.width = 0;
        assert_eq!(zero.validate(), Err(ImageEditError::InvalidCrop));

        let outside = ImageEditSpec {
            crop: ImageCrop {
                x: 900_000,
                y: 0,
                width: 100_001,
                height: 1_000_000,
            },
            ..ImageEditSpec::default()
        };
        assert_eq!(outside.validate(), Err(ImageEditError::InvalidCrop));

        let rotation = ImageEditSpec {
            rotation_quarter_turns: 4,
            ..ImageEditSpec::default()
        };
        assert_eq!(rotation.validate(), Err(ImageEditError::InvalidRotation));
    }

    /// 每次保存都保留 AI 原图和 AI 血缘、冻结旧包且不伪造阶段尝试。
    #[test]
    fn save_creates_derived_package_and_manual_lineage() {
        let mut task = complete_article();
        let original_attempt = task
            .artifacts
            .image_lineage
            .as_ref()
            .unwrap()
            .attempt_id
            .clone();
        let attempts_before = task.attempt_history.clone();
        let lock = task.image_edit_context().unwrap().lock.unwrap();
        let spec = ImageEditSpec {
            rotation_quarter_turns: 1,
            flip_horizontal: true,
            ..ImageEditSpec::default()
        };

        task.commit_image_edit(
            &lock,
            spec.clone(),
            "/managed/edit.png".into(),
            "edit-uuid".into(),
            50,
        )
        .unwrap();

        assert_eq!(task.package_version, 2);
        assert_eq!(task.package_history.len(), 1);
        assert_eq!(
            task.package_history[0].artifacts.image_ref.as_deref(),
            Some("/managed/ai.png")
        );
        assert_eq!(
            task.artifacts.original_image_ref.as_deref(),
            Some("/managed/ai.png")
        );
        assert_eq!(
            task.artifacts.image_ref.as_deref(),
            Some("/managed/edit.png")
        );
        assert_eq!(
            task.artifacts.image_lineage.as_ref().unwrap().attempt_id,
            original_attempt
        );
        assert_eq!(task.artifacts.image_edit_revision, 1);
        assert_eq!(
            task.artifacts.image_edit_lineage.as_ref().unwrap().spec,
            spec
        );
        assert_eq!(task.attempt_history, attempts_before);
        assert_eq!(task.review_status, ReviewStatus::Pending);
    }

    /// 批准历史保持不可变，且任一乐观锁变化都会拒绝提交。
    #[test]
    fn approved_history_is_immutable_and_conflicts_fail_closed() {
        let mut task = complete_article();
        task.approve(10).unwrap();
        let approved = task.approved_history.clone();
        let mut stale_lock = task.image_edit_context().unwrap().lock.unwrap();
        stale_lock.row_version = stale_lock.row_version.saturating_sub(1);

        assert_eq!(
            task.commit_image_edit(
                &stale_lock,
                ImageEditSpec::default(),
                "/managed/edit.png".into(),
                "edit".into(),
                20,
            ),
            Err(ImageEditError::StaleRowVersion)
        );
        assert_eq!(task.approved_history, approved);
        assert_eq!(task.package_version, 1);

        let current_lock = task.image_edit_context().unwrap().lock.unwrap();
        task.commit_image_edit(
            &current_lock,
            ImageEditSpec {
                flip_horizontal: true,
                ..ImageEditSpec::default()
            },
            "/managed/approved-derived.png".into(),
            "approved-edit".into(),
            21,
        )
        .unwrap();
        assert_eq!(task.package_version, 2);
        assert_eq!(task.approved_history, approved);
        assert_eq!(task.approved_history[0].version, 1);
    }

    /// 包、AI 尝试和人工修订任一变化都必须在写文件前独立拒绝。
    #[test]
    fn every_image_edit_lock_field_is_enforced() {
        let task = complete_article();
        let current = task.image_edit_context().unwrap().lock.unwrap();

        let mut stale_package = current.clone();
        stale_package.package_version += 1;
        assert_eq!(
            task.prepare_image_edit(&stale_package, &ImageEditSpec::default()),
            Err(ImageEditError::StalePackageVersion)
        );

        let mut stale_attempt = current.clone();
        stale_attempt.image_attempt_id = "other-attempt".into();
        assert_eq!(
            task.prepare_image_edit(&stale_attempt, &ImageEditSpec::default()),
            Err(ImageEditError::StaleImageAttempt)
        );

        let mut stale_edit = current;
        stale_edit.edit_revision += 1;
        assert_eq!(
            task.prepare_image_edit(&stale_edit, &ImageEditSpec::default()),
            Err(ImageEditError::StaleEditRevision)
        );
        assert_eq!(task.package_version, 1);
        assert!(task.package_history.is_empty());
    }

    /// 过期配图可查看原图，但保存必须返回明确稳定原因。
    #[test]
    fn stale_image_remains_viewable_but_cannot_save() {
        let mut task = complete_article();
        task.stages.image.status = StageStatus::Stale;
        let context = task.image_edit_context().unwrap();

        assert_eq!(context.original_image_ref, "/managed/ai.png");
        assert_eq!(context.blocked_reason.unwrap().code(), "IMAGE_EDIT_STALE");
        assert!(matches!(
            task.prepare_image_edit(&context.lock.unwrap(), &ImageEditSpec::default()),
            Err(ImageEditError::Blocked(_))
        ));
    }

    /// 原图引用缺失时不得退回当前派生图继续编辑，以免产生有损级联。
    #[test]
    fn missing_original_never_falls_back_to_current_derivative() {
        let mut task = complete_article();
        task.artifacts.image_ref = Some("/managed/manual-edit.png".into());
        task.artifacts.original_image_ref = None;

        assert_eq!(
            task.image_edit_context(),
            Err(ImageEditError::MissingOriginal)
        );
    }

    /// 任一阶段排队或运行时仍可查看 AI 原图，但保存必须明确禁用。
    #[test]
    fn busy_article_remains_viewable_but_cannot_save() {
        let mut task = complete_article();
        task.stages.format.status = StageStatus::Queued;
        let context = task.image_edit_context().unwrap();

        assert_eq!(context.original_image_ref, "/managed/ai.png");
        assert_eq!(
            context.blocked_reason.unwrap().code(),
            "IMAGE_EDIT_ARTICLE_BUSY"
        );
    }

    /// 新 AI 图片成功后替换原图起点并清空旧人工编辑血缘和修订。
    #[test]
    fn new_ai_image_resets_manual_edit_origin() {
        let mut task = complete_article();
        let lock = task.image_edit_context().unwrap().lock.unwrap();
        task.commit_image_edit(
            &lock,
            ImageEditSpec {
                flip_vertical: true,
                ..ImageEditSpec::default()
            },
            "/managed/edit.png".into(),
            "edit".into(),
            40,
        )
        .unwrap();
        task.regenerate_stage(WorkflowStage::Image, 50).unwrap();
        let attempt_id = task.start_stage(WorkflowStage::Image, 51).unwrap();
        let image_lineage = lineage(&task, WorkflowStage::Image, attempt_id);
        task.complete_stage_only(
            WorkflowStage::Image,
            ArtifactOutput::Image("/managed/new-ai.png".into()),
            image_lineage,
            52,
        )
        .unwrap();

        assert_eq!(
            task.artifacts.original_image_ref.as_deref(),
            Some("/managed/new-ai.png")
        );
        assert_eq!(
            task.artifacts.image_ref.as_deref(),
            Some("/managed/new-ai.png")
        );
        assert!(task.artifacts.image_edit_lineage.is_none());
        assert_eq!(task.artifacts.image_edit_revision, 0);
    }
}
