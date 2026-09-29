use super::*;

/// 将 core 冻结绑定授权拒绝映射为稳定 IPC 错误。
pub(super) fn binding_provider_error(error: BindingProviderError) -> CommandError {
    CommandError::new(error.code(), error.message())
}

const COMMAND_ERROR_SEPARATOR: char = '\u{1f}';

pub(super) fn encode_command_error(error: CommandError) -> String {
    format!("{}{COMMAND_ERROR_SEPARATOR}{}", error.code, error.message)
}

pub(super) fn decode_stage_generate_store_error(message: String) -> CommandError {
    if let Some((code, safe_message)) = message.split_once(COMMAND_ERROR_SEPARATOR) {
        CommandError::new(code, safe_message)
    } else {
        CommandError::new("STAGE_GENERATE_FAILED", message)
    }
}

/// 把仓储错误映射为稳定且不含路径的 GUI 错误。
pub(super) fn store_error(message: String) -> CommandError {
    CommandError::new("WORKSPACE_SAVE_FAILED", message)
}
