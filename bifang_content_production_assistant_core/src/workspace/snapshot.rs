use super::*;

/// 持久化在本地 SQLite 快照中的工作区数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceData {
    /// 持久数据模式版本。
    pub schema_version: u32,
    /// 已处理的一次性内置初始内容修订；删除种子后仍保持不变。
    #[serde(default)]
    pub initial_data_revision: u32,
    /// Grid 文章任务。
    pub articles: Vec<ArticleTask>,
    /// Prompt 模板的全部当前版本。
    pub prompt_templates: Vec<PromptTemplate>,
    /// 不含秘密的 Provider 配置。
    pub provider_configs: Vec<ProviderConfig>,
    /// 被新修订替换过的 Provider 非秘密配置。
    #[serde(default)]
    pub provider_config_history: Vec<ProviderConfig>,
    /// 当前 Provider ID 到版本化系统凭据条目的映射。
    #[serde(default)]
    pub provider_credential_refs: BTreeMap<String, String>,
    /// 等待从系统凭据库幂等删除的不透明引用；不包含密钥值。
    #[serde(default)]
    pub pending_credential_deletions: BTreeSet<String>,
    /// 当前和历史文章包的不可变执行绑定，不包含密钥值。
    #[serde(default)]
    pub execution_bindings: Vec<PackageExecutionBinding>,
    /// 不含秘密的云文档连接器当前配置。
    #[serde(default)]
    pub connector_configs: Vec<ConnectorConfig>,
    /// 被新修订替换或删除的连接器非秘密配置。
    #[serde(default)]
    pub connector_config_history: Vec<ConnectorConfig>,
    /// 当前连接器 ID 到版本化系统凭据条目的映射。
    #[serde(default)]
    pub connector_credential_refs: BTreeMap<String, String>,
    /// 等待从连接器专用系统凭据库幂等删除的引用。
    #[serde(default)]
    pub pending_connector_credential_deletions: BTreeSet<String>,
    /// 已成功上传最终文件的幂等记录。
    #[serde(default)]
    pub document_sync_records: Vec<DocumentSyncRecord>,
    /// 最近批量运行记录，最新在前。
    pub run_history: Vec<RunRecord>,
}

impl Default for WorkspaceData {
    fn default() -> Self {
        Self {
            schema_version: WORKSPACE_SCHEMA_VERSION,
            initial_data_revision: 0,
            articles: Vec::new(),
            prompt_templates: initial_prompt_templates(),
            provider_configs: Vec::new(),
            provider_config_history: Vec::new(),
            provider_credential_refs: BTreeMap::new(),
            pending_credential_deletions: BTreeSet::new(),
            execution_bindings: Vec::new(),
            connector_configs: Vec::new(),
            connector_config_history: Vec::new(),
            connector_credential_refs: BTreeMap::new(),
            pending_connector_credential_deletions: BTreeSet::new(),
            document_sync_records: Vec::new(),
            run_history: Vec::new(),
        }
    }
}

impl WorkspaceData {
    /// 在显式调试 E2E 环境中增加可识别的确定性夹具配置。
    pub fn with_mode_defaults(fixture_enabled: bool) -> Self {
        let mut workspace = Self::default();
        workspace.normalize_environment_for_mode(fixture_enabled);
        workspace
    }

    /// 收集当前 Provider 与冻结执行绑定仍在使用的所有系统凭据引用。
    pub fn referenced_credential_refs(&self) -> BTreeSet<String> {
        self.provider_configs
            .iter()
            .filter(|provider| provider.kind != ProviderKind::Fixture)
            .map(|provider| {
                self.provider_credential_refs
                    .get(&provider.id)
                    .cloned()
                    .unwrap_or_else(|| provider.id.clone())
            })
            .chain(self.execution_bindings.iter().flat_map(|binding| {
                [
                    binding.article_provider.as_ref(),
                    binding.summary_provider.as_ref(),
                    binding.image_provider.as_ref(),
                ]
                .into_iter()
                .flatten()
                .filter_map(|provider| provider.credential_ref.clone())
            }))
            .collect()
    }

    /// 依据已解析的运行模式收窄 Provider；独立参数便于无全局环境竞态地测试。
    pub fn normalize_environment_for_mode(&mut self, fixture_enabled: bool) -> bool {
        let before = (
            self.provider_configs.clone(),
            self.provider_config_history.clone(),
            self.provider_credential_refs.clone(),
            self.pending_credential_deletions.clone(),
        );
        let mut credential_refs_before = self.referenced_credential_refs();
        credential_refs_before.extend(self.provider_credential_refs.values().cloned());
        if fixture_enabled {
            self.provider_configs = vec![ProviderConfig {
                id: "provider-e2e-fixture".into(),
                name: "E2E 测试夹具（非真实 AI）".into(),
                revision: 1,
                kind: ProviderKind::Fixture,
                enabled: true,
                base_url: String::new(),
                text_model: "fixture-text-v1".into(),
                article_model: Some("fixture-article-v1".into()),
                summary_model: Some("fixture-summary-v1".into()),
                image_model: "fixture-image-v1".into(),
                article_parameters: Some(TextGenerationParameters::default()),
                summary_parameters: Some(TextGenerationParameters::default()),
                image_parameters: Some(ImageGenerationParameters::default()),
                default_for_article: true,
                default_for_summary: true,
                default_for_image: true,
            }];
            self.provider_config_history.clear();
            self.provider_credential_refs.clear();
        } else {
            self.provider_configs
                .retain(|provider| provider.kind != ProviderKind::Fixture);
            self.provider_config_history
                .retain(|provider| provider.kind != ProviderKind::Fixture);
            let current_ids = self
                .provider_configs
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            self.provider_credential_refs
                .retain(|provider_id, _| current_ids.contains(provider_id.as_str()));
        }
        let credential_refs_after = self.referenced_credential_refs();
        self.pending_credential_deletions.extend(
            credential_refs_before
                .difference(&credential_refs_after)
                .cloned(),
        );
        before
            != (
                self.provider_configs.clone(),
                self.provider_config_history.clone(),
                self.provider_credential_refs.clone(),
                self.pending_credential_deletions.clone(),
            )
    }

    /// 在普通工作区一次性写入已确认的飞书初始内容，并安全升级未编辑旧模板。
    ///
    /// 测试夹具工作区只推进账本，不混入真实业务行。已有文章、运行历史或执行绑定
    /// 时也只推进账本，避免日后删空后突然出现种子。
    pub fn apply_initial_content(
        &mut self,
        fixture_enabled: bool,
        now_ms: i64,
    ) -> Result<bool, String> {
        let mut changed = upgrade_legacy_default_prompts(&mut self.prompt_templates)
            .map_err(|error| error.to_string())?;
        let has_business_content = !self.articles.is_empty()
            || !self.run_history.is_empty()
            || !self.execution_bindings.is_empty();
        let decision = decide_initial_content(self.initial_data_revision, has_business_content)
            .map_err(|error| error.to_string())?;
        match decision {
            InitialContentDecision::AlreadyProcessed => {}
            InitialContentDecision::SkipAndMarkProcessed => {
                self.initial_data_revision = CURRENT_INITIAL_CONTENT_REVISION;
                changed = true;
            }
            InitialContentDecision::Apply if fixture_enabled => {
                self.initial_data_revision = CURRENT_INITIAL_CONTENT_REVISION;
                changed = true;
            }
            InitialContentDecision::Apply => {
                self.articles.extend(initial_article_tasks(now_ms));
                self.initial_data_revision = CURRENT_INITIAL_CONTENT_REVISION;
                changed = true;
            }
        }
        Ok(changed)
    }
}
