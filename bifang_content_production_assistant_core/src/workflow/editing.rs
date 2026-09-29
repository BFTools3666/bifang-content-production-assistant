use super::*;

impl ArticleTask {
    /// 编辑当前正文并让实际依赖它的简介、配图和格式产物过期。
    pub fn edit_article(
        &mut self,
        markdown: impl Into<String>,
        lineage: ArtifactLineage,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        let markdown = markdown.into();
        if markdown.trim().is_empty() {
            return Err(WorkflowError::EmptyArtifact);
        }
        if let Some(stage) = self.active_or_queued_stage() {
            return Err(WorkflowError::InvalidTransition {
                stage,
                status: self.stages.get(stage).status,
            });
        }
        let target_version = if self.review_status == ReviewStatus::Approved {
            self.package_version.saturating_add(1)
        } else {
            self.package_version
        };
        if lineage.package_version != target_version
            || !self.lineage_sources_match(WorkflowStage::Article, &lineage)
        {
            return Err(WorkflowError::ArtifactStageMismatch);
        }
        if self.review_status == ReviewStatus::Approved {
            self.begin_derived_version(now_ms);
        }
        self.artifacts.markdown = Some(markdown);
        self.artifacts.article_lineage = Some(lineage);
        let article = self.stages.get_mut(WorkflowStage::Article);
        article.status = StageStatus::Succeeded;
        article.artifact_version = Some(self.package_version);
        article.failure = None;
        article.active_attempt_id = None;
        for stage in [
            WorkflowStage::Summary,
            WorkflowStage::Image,
            WorkflowStage::Format,
        ] {
            self.mark_executed_stage_stale(stage);
        }
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 编辑当前简介并让依赖该简介的配图产物过期。
    pub fn edit_summary(
        &mut self,
        summary: impl Into<String>,
        lineage: ArtifactLineage,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        let summary = summary.into();
        if summary.trim().is_empty() {
            return Err(WorkflowError::EmptyArtifact);
        }
        if let Some(stage) = self.active_or_queued_stage() {
            return Err(WorkflowError::InvalidTransition {
                stage,
                status: self.stages.get(stage).status,
            });
        }
        if self.stages.article.status != StageStatus::Succeeded || self.artifacts.markdown.is_none()
        {
            return Err(WorkflowError::DependencyNotReady {
                stage: WorkflowStage::Summary,
            });
        }
        let target_version = if self.review_status == ReviewStatus::Approved {
            self.package_version.saturating_add(1)
        } else {
            self.package_version
        };
        if lineage.package_version != target_version
            || !self.lineage_sources_match(WorkflowStage::Summary, &lineage)
        {
            return Err(WorkflowError::ArtifactStageMismatch);
        }
        if self.review_status == ReviewStatus::Approved {
            self.begin_derived_version(now_ms);
        }
        self.artifacts.summary = Some(summary);
        self.artifacts.summary_lineage = Some(lineage);
        let summary_stage = self.stages.get_mut(WorkflowStage::Summary);
        summary_stage.status = StageStatus::Succeeded;
        summary_stage.artifact_version = Some(self.package_version);
        summary_stage.failure = None;
        summary_stage.active_attempt_id = None;
        self.mark_executed_stage_stale(WorkflowStage::Image);
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 更新关键词和内容要求；已有产物时创建新版本，避免旧输入与新产物混用。
    pub fn update_inputs(
        &mut self,
        keywords: Vec<String>,
        requirements: impl Into<String>,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        let keywords = keywords
            .into_iter()
            .map(|keyword| keyword.trim().to_string())
            .filter(|keyword| !keyword.is_empty())
            .collect::<Vec<_>>();
        let requirements = requirements.into().trim().to_string();
        if keywords.len() > 20 || keywords.iter().any(|value| value.chars().count() > 100) {
            return Err(WorkflowError::InvalidInput(
                "关键词最多 20 个，每个最多 100 个字符".into(),
            ));
        }
        if requirements.chars().count() > 2_000 {
            return Err(WorkflowError::InvalidInput(
                "内容要求最多 2000 个字符".into(),
            ));
        }
        if self.keywords == keywords && self.requirements == requirements {
            return Ok(());
        }
        if WorkflowStage::ordered().into_iter().any(|stage| {
            matches!(
                self.stages.get(stage).status,
                StageStatus::Queued | StageStatus::Running
            )
        }) {
            return Err(WorkflowError::InvalidTransition {
                stage: WorkflowStage::Article,
                status: self.stages.article.status,
            });
        }
        if self.stages != StageStates::default() || self.review_status != ReviewStatus::Draft {
            self.begin_new_version(now_ms);
        }
        self.keywords = keywords;
        self.requirements = requirements;
        self.touch(now_ms);
        Ok(())
    }

    /// 在保留旧包快照后创建新版本，并重新排队指定阶段。
    pub fn regenerate_stage(
        &mut self,
        stage: WorkflowStage,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        if let Some(active_stage) = self.active_or_queued_stage() {
            return Err(WorkflowError::InvalidTransition {
                stage: active_stage,
                status: self.stages.get(active_stage).status,
            });
        }
        if stage == WorkflowStage::Article {
            if self.title.trim().is_empty() {
                return Err(WorkflowError::EmptyTitle);
            }
            self.begin_new_version(now_ms);
            return self.queue_initial(now_ms);
        }
        if !self.dependencies_ready(stage) {
            return Err(WorkflowError::DependencyNotReady { stage });
        }
        self.begin_derived_version(now_ms);
        let progress = self.stages.get_mut(stage);
        progress.status = StageStatus::Queued;
        progress.failure = None;
        progress.active_attempt_id = None;
        if stage == WorkflowStage::Summary {
            self.mark_executed_stage_stale(WorkflowStage::Image);
        }
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 为 Grid 单步生成排队目标阶段：首次执行复用当前包，其余状态显式创建新包。
    pub fn queue_single_stage(
        &mut self,
        stage: WorkflowStage,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        if let Some(active_stage) = self.active_or_queued_stage() {
            return Err(WorkflowError::InvalidTransition {
                stage: active_stage,
                status: self.stages.get(active_stage).status,
            });
        }
        if self.stages.get(stage).status != StageStatus::NotStarted {
            return self.regenerate_stage(stage, now_ms);
        }
        if self.title.trim().is_empty() {
            return Err(WorkflowError::EmptyTitle);
        }
        if !self.dependencies_ready(stage) {
            return Err(WorkflowError::DependencyNotReady { stage });
        }
        let progress = self.stages.get_mut(stage);
        progress.status = StageStatus::Queued;
        progress.failure = None;
        progress.active_attempt_id = None;
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 把完整但未批准的文章包退回草稿，等待人工编辑和依赖重生成。
    pub fn return_to_draft(&mut self, now_ms: i64) -> Result<(), WorkflowError> {
        if self.review_status != ReviewStatus::Pending {
            return Err(WorkflowError::NotPendingReview);
        }
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 显式开始一次完整重新生成，创建新版本且保留当前包快照。
    fn begin_new_version(&mut self, now_ms: i64) {
        self.snapshot_current_package(now_ms);
        self.package_version = self.package_version.saturating_add(1);
        self.stages = Default::default();
        self.artifacts = Default::default();
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
    }

    /// 创建复用未受影响产物的新包版本，并让审核回到草稿。
    pub(crate) fn begin_derived_version(&mut self, now_ms: i64) {
        self.snapshot_current_package(now_ms);
        self.package_version = self.package_version.saturating_add(1);
        for stage in WorkflowStage::ordered() {
            let progress = self.stages.get_mut(stage);
            progress.attempts = 0;
            progress.active_attempt_id = None;
        }
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
    }

    /// 在被替换前保存当前完整输入与已有产物，避免同版本静默覆盖。
    fn snapshot_current_package(&mut self, now_ms: i64) {
        let has_work = self.artifacts != Default::default()
            || self.stages != StageStates::default()
            || self.review_status != ReviewStatus::Draft;
        let already_saved = self
            .package_history
            .last()
            .is_some_and(|snapshot| snapshot.version == self.package_version);
        if has_work && !already_saved {
            self.package_history.push(ArticlePackageSnapshot {
                version: self.package_version,
                title: self.title.clone(),
                keywords: self.keywords.clone(),
                requirements: self.requirements.clone(),
                review_status: self.review_status,
                artifacts: self.artifacts.clone(),
                captured_at_ms: now_ms,
            });
        }
    }

    /// 批准当前完整文章包并保存不可变快照。
    pub fn approve(&mut self, now_ms: i64) -> Result<(), WorkflowError> {
        if !self.all_stages_succeeded()
            || !self.artifacts.is_complete()
            || self.review_status != ReviewStatus::Pending
        {
            return Err(WorkflowError::NotReadyForApproval);
        }
        self.approved_history.push(ApprovedPackage {
            version: self.package_version,
            title: self.title.clone(),
            keywords: self.keywords.clone(),
            requirements: self.requirements.clone(),
            artifacts: self.artifacts.clone(),
            approved_at_ms: now_ms,
        });
        self.review_status = ReviewStatus::Approved;
        self.touch(now_ms);
        Ok(())
    }

    /// 返回当前依赖已经满足的排队阶段。
    pub fn next_runnable_stage(&self) -> Option<WorkflowStage> {
        WorkflowStage::ordered().into_iter().find(|stage| {
            self.stages.get(*stage).status == StageStatus::Queued && self.dependencies_ready(*stage)
        })
    }

    /// 为同一文章包、阶段和冻结执行输入生成跨适配器稳定的幂等键。
    #[allow(clippy::too_many_arguments)]
    pub fn stage_idempotency_key(
        &self,
        stage: WorkflowStage,
        provider_id: &str,
        provider_revision: u32,
        provider_endpoint: &str,
        template_id: &str,
        template_version: u32,
        template_content: &str,
        model: &str,
        rendered_prompt: &str,
        generation_parameters: &str,
    ) -> String {
        const FNV_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
        const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
        let stage = stage.stable_id();
        let package_version = self.package_version.to_string();
        let provider_revision = provider_revision.to_string();
        let template_version = template_version.to_string();
        let mut hash = FNV_OFFSET;
        for part in [
            self.id.as_str(),
            package_version.as_str(),
            stage,
            provider_id,
            provider_revision.as_str(),
            provider_endpoint,
            template_id,
            template_version.as_str(),
            template_content,
            model,
            rendered_prompt,
        ] {
            for byte in part.as_bytes() {
                hash ^= u128::from(*byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
            hash ^= u128::from(0xff_u8);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        if !generation_parameters.is_empty() {
            for byte in generation_parameters.as_bytes() {
                hash ^= u128::from(*byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
            hash ^= u128::from(0xff_u8);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        format!("gfs-{hash:032x}")
    }

    /// 判断目标阶段的执行前置阶段与所需来源产物是否完整成功。
    pub(super) fn dependencies_ready(&self, stage: WorkflowStage) -> bool {
        if !stage
            .execution_dependencies()
            .iter()
            .all(|dependency| self.stages.get(*dependency).status == StageStatus::Succeeded)
        {
            return false;
        }

        match stage {
            WorkflowStage::Article => true,
            WorkflowStage::Summary => {
                self.artifacts.markdown.is_some() && self.stages.article.artifact_version.is_some()
            }
            WorkflowStage::Image => {
                self.artifacts.markdown.is_some()
                    && self.artifacts.summary.is_some()
                    && self.stages.article.artifact_version.is_some()
                    && self.stages.summary.artifact_version.is_some()
            }
            WorkflowStage::Format => {
                self.artifacts.markdown.is_some() && self.stages.article.artifact_version.is_some()
            }
        }
    }

    /// 校验派生产物声明的直接来源版本与当前实际读取产物完全一致。
    pub(super) fn lineage_sources_match(
        &self,
        stage: WorkflowStage,
        lineage: &ArtifactLineage,
    ) -> bool {
        match stage {
            WorkflowStage::Article => {
                lineage.source_article_version.is_none() && lineage.source_summary_version.is_none()
            }
            WorkflowStage::Summary | WorkflowStage::Format => {
                self.stages.article.artifact_version.is_some()
                    && lineage.source_article_version == self.stages.article.artifact_version
                    && lineage.source_summary_version.is_none()
            }
            WorkflowStage::Image => {
                self.stages.article.artifact_version.is_some()
                    && self.stages.summary.artifact_version.is_some()
                    && lineage.source_article_version == self.stages.article.artifact_version
                    && lineage.source_summary_version == self.stages.summary.artifact_version
            }
        }
    }

    /// 返回固定顺序中的下一个未开始或已过期阶段。
    pub(super) fn next_pending_stage(&self) -> Option<WorkflowStage> {
        WorkflowStage::ordered().into_iter().find(|stage| {
            matches!(
                self.stages.get(*stage).status,
                StageStatus::NotStarted | StageStatus::Stale
            )
        })
    }

    /// 判断所有必需阶段是否成功。
    pub(crate) fn all_stages_succeeded(&self) -> bool {
        WorkflowStage::ordered()
            .into_iter()
            .all(|stage| self.stages.get(stage).status == StageStatus::Succeeded)
    }

    /// 返回当前由 worker 拥有的排队或运行阶段。
    fn active_or_queued_stage(&self) -> Option<WorkflowStage> {
        WorkflowStage::ordered().into_iter().find(|stage| {
            matches!(
                self.stages.get(*stage).status,
                StageStatus::Queued | StageStatus::Running
            )
        })
    }

    /// 上游人工变化后，任何已经执行过的下游状态都必须失效，不能再走普通失败重试。
    fn mark_executed_stage_stale(&mut self, stage: WorkflowStage) {
        let progress = self.stages.get_mut(stage);
        if progress.status != StageStatus::NotStarted {
            progress.status = StageStatus::Stale;
            progress.failure = None;
            progress.active_attempt_id = None;
        }
    }

    /// 更新行版本和最后修改时间。
    pub(crate) fn touch(&mut self, now_ms: i64) {
        self.row_version = self.row_version.saturating_add(1);
        self.updated_at_ms = now_ms;
    }
}
