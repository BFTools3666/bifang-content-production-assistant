use super::*;

impl WorkspaceData {
    /// 把旧 Provider 配置与图片执行绑定迁移为当前工作区模式。
    pub fn migrate_to_current_schema(&mut self) -> Result<bool, String> {
        let source_schema_version = self.schema_version;
        let provider_schema_version = match source_schema_version {
            1 | 2 => source_schema_version,
            3 | 4 => 3,
            5 => 4,
            VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION
            | PRE_IMAGE_EDITOR_SCHEMA_VERSION
            | PRE_GENERATION_METRICS_SCHEMA_VERSION
            | PRE_CONNECTOR_SCHEMA_VERSION
            | PRE_LARK_CLI_CONNECTOR_SCHEMA_VERSION
            | PRE_RESEARCH_SCHEMA_VERSION
            | WORKSPACE_SCHEMA_VERSION => 5,
            _ => return Err("本地工作区版本不受支持".to_string()),
        };
        let inferred_execution_contracts = self
            .execution_bindings
            .iter()
            .map(|binding| {
                let prompt_renderer_version =
                    infer_missing_prompt_renderer_version(source_schema_version, binding)?;
                let effective_prompt_renderer_version =
                    prompt_renderer_version.unwrap_or(binding.prompt_renderer_version);
                let image_request_contract = infer_missing_image_request_contract(
                    source_schema_version,
                    effective_prompt_renderer_version,
                    binding,
                )?;
                Ok::<_, String>((prompt_renderer_version, image_request_contract))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let migration = ProviderConfigMigration::for_schema_version(provider_schema_version)
            .map_err(|error| error.to_string())?;
        let mut changed = false;
        if source_schema_version < VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION {
            for provider in self
                .provider_configs
                .iter_mut()
                .chain(self.provider_config_history.iter_mut())
            {
                changed |= migration.migrate_config(provider);
            }
        } else if source_schema_version == VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION {
            for provider in &mut self.provider_configs {
                changed |= migrate_legacy_volcengine_ark_image_provider(provider);
            }
        }
        for (binding, (inferred_renderer, inferred_contract)) in self
            .execution_bindings
            .iter_mut()
            .zip(inferred_execution_contracts)
        {
            if let Some(inferred_renderer) = inferred_renderer {
                binding.prompt_renderer_version = inferred_renderer;
                changed = true;
            }
            if let Some(inferred_contract) = inferred_contract {
                binding.image_request_contract = Some(inferred_contract);
                changed = true;
            }
            if source_schema_version < VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION {
                for provider in [
                    binding.article_provider.as_mut(),
                    binding.summary_provider.as_mut(),
                    binding.image_provider.as_mut(),
                ]
                .into_iter()
                .flatten()
                {
                    changed |= migration.migrate_config(&mut provider.config);
                }
            }
        }
        if source_schema_version < PRE_GENERATION_METRICS_SCHEMA_VERSION {
            for article in &mut self.articles {
                changed |= initialize_original_image_reference(&mut article.artifacts);
                for snapshot in &mut article.package_history {
                    changed |= initialize_original_image_reference(&mut snapshot.artifacts);
                }
                for approved in &mut article.approved_history {
                    changed |= initialize_original_image_reference(&mut approved.artifacts);
                }
            }
        }
        for article in &mut self.articles {
            changed |= article.backfill_created_at_ms();
        }
        if source_schema_version < PRE_RESEARCH_SCHEMA_VERSION {
            changed |= migrate_connector_configs_to_cli(
                &mut self.connector_configs,
                &mut self.connector_config_history,
            )
            .map_err(|error| error.to_string())?;
            let legacy_references = std::mem::take(&mut self.connector_credential_refs);
            if !legacy_references.is_empty() {
                self.pending_connector_credential_deletions
                    .extend(legacy_references.into_values());
                changed = true;
            }
        }
        if self.schema_version != WORKSPACE_SCHEMA_VERSION {
            self.schema_version = WORKSPACE_SCHEMA_VERSION;
            changed = true;
        }
        Ok(changed)
    }
}

/// 旧快照中的每一份产物都以自己的当前图片作为不可变 AI 原图起点。
fn initialize_original_image_reference(artifacts: &mut ArticleArtifacts) -> bool {
    if artifacts.original_image_ref.is_some() {
        return false;
    }
    let Some(image_ref) = artifacts
        .image_ref
        .as_ref()
        .filter(|reference| !reference.trim().is_empty())
        .cloned()
    else {
        return false;
    };
    artifacts.original_image_ref = Some(image_ref);
    true
}

/// 仅把 v6 当前集合中语义明确的旧 Ark 图片配置切换到专属 Provider 协议。
fn migrate_legacy_volcengine_ark_image_provider(provider: &mut ProviderConfig) -> bool {
    let endpoint = provider
        .base_url
        .strip_suffix('/')
        .unwrap_or(&provider.base_url);
    let is_legacy_ark_image = provider.kind == ProviderKind::OpenAiCompatible
        && endpoint == LEGACY_VOLCENGINE_ARK_IMAGE_ENDPOINT
        && provider.text_model.trim().is_empty()
        && provider.article_model().trim().is_empty()
        && provider.summary_model().trim().is_empty()
        && !provider.default_for_article
        && !provider.default_for_summary
        && !provider.image_model.trim().is_empty();
    if !is_legacy_ark_image {
        return false;
    }

    let timeout_ms = provider.image_parameters().timeout_ms;
    provider.kind = ProviderKind::VolcengineArkImage;
    provider.base_url = VOLCENGINE_ARK_API_ROOT.into();
    provider.image_parameters = Some(ImageGenerationParameters {
        size: "2K".into(),
        watermark: true,
        timeout_ms,
    });
    true
}

/// 旧 schema 可补入当时唯一的 renderer；v3 及更新版本缺失则无法证明执行语义。
fn infer_missing_prompt_renderer_version(
    source_schema_version: u32,
    binding: &PackageExecutionBinding,
) -> Result<Option<u32>, String> {
    if binding.prompt_renderer_version != 0 {
        return Ok(None);
    }
    match source_schema_version {
        1 | 2 => Ok(Some(LEGACY_PROMPT_RENDERER_VERSION)),
        3 => Err("v3 工作区缺少 Prompt 渲染契约".to_string()),
        4
        | 5
        | VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION
        | PRE_IMAGE_EDITOR_SCHEMA_VERSION
        | PRE_GENERATION_METRICS_SCHEMA_VERSION
        | PRE_CONNECTOR_SCHEMA_VERSION
        | PRE_LARK_CLI_CONNECTOR_SCHEMA_VERSION
        | WORKSPACE_SCHEMA_VERSION => Err("当前工作区缺少 Prompt 渲染契约".to_string()),
        _ => Err("本地工作区版本不受支持".to_string()),
    }
}

/// 使用持久 schema、有效 renderer 与冻结图片 Provider 类型推断旧图片 wire 契约。
fn infer_missing_image_request_contract(
    source_schema_version: u32,
    prompt_renderer_version: u32,
    binding: &PackageExecutionBinding,
) -> Result<Option<ImageRequestContract>, String> {
    let provider_kind = binding
        .image_provider
        .as_ref()
        .map(|provider| provider.config.kind);
    if source_schema_version < VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION
        && (provider_kind == Some(ProviderKind::VolcengineArkImage)
            || binding.image_request_contract == Some(ImageRequestContract::VolcengineArkUrlV1))
    {
        return Err(format!(
            "v{source_schema_version} 工作区包含当时不受支持的火山方舟图片契约"
        ));
    }
    if binding.image_request_contract.is_some() {
        return Ok(None);
    }
    match source_schema_version {
        1 | 2 => Ok(Some(legacy_image_request_contract())),
        3 => {
            match prompt_renderer_version {
                LEGACY_PROMPT_RENDERER_VERSION => match provider_kind {
                    // 这类绑定创建于显式 b64_json 契约时期；没有真实图片 Provider 时，
                    // v3 程序不可能以省略字段的 body 发出图片请求，因此不存在重试歧义。
                    None | Some(ProviderKind::Fixture) => Ok(Some(legacy_image_request_contract())),
                    Some(ProviderKind::OpenAiCompatible) => {
                        Err("v3 工作区的旧图片请求契约存在歧义".to_string())
                    }
                    Some(ProviderKind::VolcengineArkImage) => unreachable!(),
                },
                V3_NEW_BINDING_PROMPT_RENDERER_VERSION => {
                    Ok(Some(ImageRequestContract::ProviderDefaultBase64Unversioned))
                }
                _ => Err("旧工作区的图片请求契约无法判定".to_string()),
            }
        }
        4
        | 5
        | VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION
        | PRE_IMAGE_EDITOR_SCHEMA_VERSION
        | PRE_GENERATION_METRICS_SCHEMA_VERSION
        | PRE_CONNECTOR_SCHEMA_VERSION
        | PRE_LARK_CLI_CONNECTOR_SCHEMA_VERSION
        | WORKSPACE_SCHEMA_VERSION => Err("当前工作区缺少图片请求契约".to_string()),
        _ => Err("本地工作区版本不受支持".to_string()),
    }
}
