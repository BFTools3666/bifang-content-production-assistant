use super::*;

impl ArticleTask {
    /// 把草稿任务的首个阶段放入队列，不发出外部调用。
    pub fn queue_initial(&mut self, now_ms: i64) -> Result<(), WorkflowError> {
        if self.title.trim().is_empty() {
            return Err(WorkflowError::EmptyTitle);
        }
        let article = self.stages.get_mut(WorkflowStage::Article);
        if article.status != StageStatus::NotStarted {
            return Err(WorkflowError::InvalidTransition {
                stage: WorkflowStage::Article,
                status: article.status,
            });
        }
        article.status = StageStatus::Queued;
        article.failure = None;
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 开始一个已经排队且依赖就绪的阶段。
    pub fn start_stage(
        &mut self,
        stage: WorkflowStage,
        now_ms: i64,
    ) -> Result<String, WorkflowError> {
        let current = self.stages.get(stage).status;
        if current != StageStatus::Queued {
            return Err(WorkflowError::InvalidTransition {
                stage,
                status: current,
            });
        }
        if !self.dependencies_ready(stage) {
            return Err(WorkflowError::DependencyNotReady { stage });
        }
        let progress = self.stages.get_mut(stage);
        progress.status = StageStatus::Running;
        progress.attempts = progress.attempts.saturating_add(1);
        progress.failure = None;
        let attempt_id = format!(
            "{}:v{}:{stage:?}:attempt-{}",
            self.id, self.package_version, progress.attempts
        );
        progress.active_attempt_id = Some(attempt_id.clone());
        self.attempt_history.push(StageAttempt {
            id: attempt_id.clone(),
            package_version: self.package_version,
            stage,
            status: StageAttemptStatus::Running,
            started_at_ms: now_ms,
            finished_at_ms: None,
            failure: None,
            token_usage: None,
            lineage: None,
        });
        self.touch(now_ms);
        Ok(attempt_id)
    }

    /// 原子地把类型匹配的完整产物写入当前阶段并推进后续阶段。
    pub fn complete_stage(
        &mut self,
        stage: WorkflowStage,
        output: ArtifactOutput,
        lineage: ArtifactLineage,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        self.complete_stage_with_usage(stage, output, lineage, None, now_ms)
    }

    /// 原子保存产物及 Provider 实际返回的 token 用量，并推进固定 SOP。
    pub fn complete_stage_with_usage(
        &mut self,
        stage: WorkflowStage,
        output: ArtifactOutput,
        lineage: ArtifactLineage,
        token_usage: Option<TokenUsage>,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        self.complete_stage_with_policy(stage, output, lineage, token_usage, now_ms, true)
    }

    /// 原子保存一次显式单步生成结果，但不自动排队任何后续阶段。
    pub fn complete_stage_only(
        &mut self,
        stage: WorkflowStage,
        output: ArtifactOutput,
        lineage: ArtifactLineage,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        self.complete_stage_only_with_usage(stage, output, lineage, None, now_ms)
    }

    /// 原子保存单步产物及 Provider 实际返回的 token 用量，不排队后续阶段。
    pub fn complete_stage_only_with_usage(
        &mut self,
        stage: WorkflowStage,
        output: ArtifactOutput,
        lineage: ArtifactLineage,
        token_usage: Option<TokenUsage>,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        self.complete_stage_with_policy(stage, output, lineage, token_usage, now_ms, false)
    }

    fn complete_stage_with_policy(
        &mut self,
        stage: WorkflowStage,
        output: ArtifactOutput,
        lineage: ArtifactLineage,
        token_usage: Option<TokenUsage>,
        now_ms: i64,
        queue_downstream: bool,
    ) -> Result<(), WorkflowError> {
        let current = self.stages.get(stage).status;
        if current != StageStatus::Running {
            return Err(WorkflowError::InvalidTransition {
                stage,
                status: current,
            });
        }
        let active_attempt_id = self.stages.get(stage).active_attempt_id.clone();
        if stage != output.stage()
            || lineage.package_version != self.package_version
            || active_attempt_id.as_deref() != Some(lineage.attempt_id.as_str())
            || !self.lineage_sources_match(stage, &lineage)
        {
            return Err(WorkflowError::ArtifactStageMismatch);
        }
        if output.is_empty() {
            return Err(WorkflowError::EmptyArtifact);
        }

        let attempt_id = lineage.attempt_id.clone();
        let has_matching_attempt = self.attempt_history.iter().rev().any(|attempt| {
            attempt.id == attempt_id
                && attempt.package_version == self.package_version
                && attempt.stage == stage
                && attempt.status == StageAttemptStatus::Running
        });
        if !has_matching_attempt {
            return Err(WorkflowError::ArtifactStageMismatch);
        }

        let attempt_lineage = lineage.clone();
        match output {
            ArtifactOutput::Article(value) => {
                self.artifacts.markdown = Some(value);
                self.artifacts.article_lineage = Some(lineage);
            }
            ArtifactOutput::Summary(value) => {
                self.artifacts.summary = Some(value);
                self.artifacts.summary_lineage = Some(lineage);
            }
            ArtifactOutput::Image(value) => {
                self.artifacts.original_image_ref = Some(value.clone());
                self.artifacts.image_ref = Some(value);
                self.artifacts.image_lineage = Some(lineage);
                self.artifacts.image_edit_lineage = None;
                self.artifacts.image_edit_revision = 0;
            }
            ArtifactOutput::Format(value) => {
                self.artifacts.html = Some(value);
                self.artifacts.format_lineage = Some(lineage);
            }
        }

        let progress = self.stages.get_mut(stage);
        progress.status = StageStatus::Succeeded;
        progress.artifact_version = Some(self.package_version);
        progress.failure = None;
        progress.active_attempt_id = None;
        if let Some(attempt) = self
            .attempt_history
            .iter_mut()
            .rev()
            .find(|attempt| attempt.id == attempt_id)
        {
            attempt.status = StageAttemptStatus::Succeeded;
            attempt.finished_at_ms = Some(now_ms);
            attempt.token_usage = token_usage;
            attempt.lineage = Some(attempt_lineage);
        }

        if queue_downstream && let Some(next) = self.next_pending_stage() {
            self.stages.get_mut(next).status = StageStatus::Queued;
        }
        if self.all_stages_succeeded() && self.artifacts.is_complete() {
            self.review_status = ReviewStatus::Pending;
        }
        self.touch(now_ms);
        Ok(())
    }

    /// 记录阶段失败并保留此前已经成功的产物。
    pub fn fail_stage(
        &mut self,
        stage: WorkflowStage,
        failure: Failure,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        self.fail_stage_with_usage(stage, failure, None, now_ms)
    }

    /// 记录阶段失败，并保留在失败前已经由 Provider 返回的真实 token 用量。
    pub fn fail_stage_with_usage(
        &mut self,
        stage: WorkflowStage,
        failure: Failure,
        token_usage: Option<TokenUsage>,
        now_ms: i64,
    ) -> Result<(), WorkflowError> {
        let current = self.stages.get(stage).status;
        if !matches!(current, StageStatus::Running | StageStatus::Queued) {
            return Err(WorkflowError::InvalidTransition {
                stage,
                status: current,
            });
        }
        let progress = self.stages.get_mut(stage);
        progress.status = StageStatus::Failed;
        progress.failure = Some(failure.clone());
        if let Some(attempt_id) = progress.active_attempt_id.take()
            && let Some(attempt) = self
                .attempt_history
                .iter_mut()
                .rev()
                .find(|attempt| attempt.id == attempt_id)
        {
            attempt.status = StageAttemptStatus::Failed;
            attempt.finished_at_ms = Some(now_ms);
            attempt.failure = Some(failure);
            attempt.token_usage = token_usage;
        }
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(())
    }

    /// 只把最早的可重试失败或中断阶段重新排队；其余情况必须显式重新生成。
    pub fn retry_failed(&mut self, now_ms: i64) -> Result<WorkflowStage, WorkflowError> {
        let stage = WorkflowStage::ordered()
            .into_iter()
            .find(|stage| {
                let progress = self.stages.get(*stage);
                matches!(
                    progress.status,
                    StageStatus::Failed | StageStatus::Interrupted
                ) && progress
                    .failure
                    .as_ref()
                    .is_some_and(|failure| failure.retryable)
            })
            .ok_or(WorkflowError::NothingToRetry)?;
        if !self.dependencies_ready(stage) {
            return Err(WorkflowError::DependencyNotReady { stage });
        }
        let progress = self.stages.get_mut(stage);
        progress.status = StageStatus::Queued;
        progress.failure = None;
        progress.active_attempt_id = None;
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        Ok(stage)
    }

    /// 把失去 worker 所有权的排队或运行中阶段恢复为默认可重试中断状态。
    pub fn recover_interrupted(&mut self, now_ms: i64) -> bool {
        self.recover_interrupted_with_policy(now_ms, |_| true)
    }

    /// 恢复中断状态，并由宿主按冻结外部契约判定已运行阶段能否安全普通重试。
    pub fn recover_interrupted_with_policy(
        &mut self,
        now_ms: i64,
        mut running_stage_is_retryable: impl FnMut(WorkflowStage) -> bool,
    ) -> bool {
        let mut changed = false;
        for stage in WorkflowStage::ordered() {
            let progress = self.stages.get_mut(stage);
            if matches!(progress.status, StageStatus::Queued | StageStatus::Running) {
                let was_running = progress.status == StageStatus::Running;
                let retryable = !was_running || running_stage_is_retryable(stage);
                progress.status = StageStatus::Interrupted;
                let failure = if retryable {
                    Failure {
                        code: "RUN_INTERRUPTED".into(),
                        message: "应用退出或任务所有权丢失后已安全中断，可从该阶段重试".into(),
                        retryable: true,
                        retry_after_ms: None,
                    }
                } else {
                    Failure {
                        code: "RUN_OUTCOME_UNKNOWN".into(),
                        message:
                            "应用退出时外部生成请求可能已经提交，结果无法确认；为避免重复调用，不会自动重试，请手动重新生成"
                                .into(),
                        retryable: false,
                        retry_after_ms: None,
                    }
                };
                progress.failure = Some(failure.clone());
                if was_running
                    && let Some(attempt_id) = progress.active_attempt_id.take()
                    && let Some(attempt) = self
                        .attempt_history
                        .iter_mut()
                        .rev()
                        .find(|attempt| attempt.id == attempt_id)
                {
                    attempt.status = StageAttemptStatus::Interrupted;
                    attempt.finished_at_ms = Some(now_ms);
                    attempt.failure = Some(failure);
                }
                changed = true;
            }
        }
        if changed {
            self.review_status = ReviewStatus::Draft;
            self.touch(now_ms);
        }
        changed
    }

    /// 只恢复一次显式单步运行拥有的目标阶段，避免改写同包其他阶段。
    pub fn recover_interrupted_stage_with_policy(
        &mut self,
        stage: WorkflowStage,
        now_ms: i64,
        running_stage_is_retryable: bool,
    ) -> bool {
        let progress = self.stages.get_mut(stage);
        if !matches!(progress.status, StageStatus::Queued | StageStatus::Running) {
            return false;
        }
        let was_running = progress.status == StageStatus::Running;
        let retryable = !was_running || running_stage_is_retryable;
        progress.status = StageStatus::Interrupted;
        let failure = if retryable {
            Failure {
                code: "RUN_INTERRUPTED".into(),
                message: "应用退出或任务所有权丢失后已安全中断，可从该阶段重试".into(),
                retryable: true,
                retry_after_ms: None,
            }
        } else {
            Failure {
                code: "RUN_OUTCOME_UNKNOWN".into(),
                message:
                    "应用退出时外部生成请求可能已经提交，结果无法确认；为避免重复调用，不会自动重试，请手动重新生成"
                        .into(),
                retryable: false,
                retry_after_ms: None,
            }
        };
        progress.failure = Some(failure.clone());
        if was_running
            && let Some(attempt_id) = progress.active_attempt_id.take()
            && let Some(attempt) = self
                .attempt_history
                .iter_mut()
                .rev()
                .find(|attempt| attempt.id == attempt_id)
        {
            attempt.status = StageAttemptStatus::Interrupted;
            attempt.finished_at_ms = Some(now_ms);
            attempt.failure = Some(failure);
        }
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        true
    }

    /// 取消未完成阶段并保留全部成功产物。
    pub fn cancel_pending(&mut self, now_ms: i64) -> bool {
        let mut changed = false;
        for stage in WorkflowStage::ordered() {
            let progress = self.stages.get_mut(stage);
            if matches!(
                progress.status,
                StageStatus::NotStarted
                    | StageStatus::Queued
                    | StageStatus::Running
                    | StageStatus::Stale
            ) {
                if progress.status == StageStatus::Running
                    && let Some(attempt_id) = progress.active_attempt_id.take()
                    && let Some(attempt) = self
                        .attempt_history
                        .iter_mut()
                        .rev()
                        .find(|attempt| attempt.id == attempt_id)
                {
                    attempt.status = StageAttemptStatus::Cancelled;
                    attempt.finished_at_ms = Some(now_ms);
                }
                progress.status = StageStatus::Cancelled;
                progress.failure = None;
                progress.active_attempt_id = None;
                changed = true;
            }
        }
        if changed {
            self.review_status = ReviewStatus::Draft;
            self.touch(now_ms);
        }
        changed
    }

    /// 只取消一次显式单步运行拥有的目标阶段。
    pub fn cancel_pending_stage(&mut self, stage: WorkflowStage, now_ms: i64) -> bool {
        let progress = self.stages.get_mut(stage);
        if !matches!(
            progress.status,
            StageStatus::NotStarted
                | StageStatus::Queued
                | StageStatus::Running
                | StageStatus::Stale
        ) {
            return false;
        }
        if progress.status == StageStatus::Running
            && let Some(attempt_id) = progress.active_attempt_id.take()
            && let Some(attempt) = self
                .attempt_history
                .iter_mut()
                .rev()
                .find(|attempt| attempt.id == attempt_id)
        {
            attempt.status = StageAttemptStatus::Cancelled;
            attempt.finished_at_ms = Some(now_ms);
        }
        progress.status = StageStatus::Cancelled;
        progress.failure = None;
        progress.active_attempt_id = None;
        self.review_status = ReviewStatus::Draft;
        self.touch(now_ms);
        true
    }
}
