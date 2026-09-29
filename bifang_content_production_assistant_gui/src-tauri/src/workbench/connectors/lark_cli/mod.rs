//! 官方 `lark-cli` 的无 shell、有界子进程适配器。

mod process;
mod protocol;
mod publish;
mod setup;

pub(super) use process::LarkCliClient;
pub(super) use protocol::LarkCliError;

#[cfg(test)]
use process::{OperationRisk, resolve_lark_cli_candidates};
#[cfg(test)]
use protocol::{LarkCliUploadResult, extract_file_token, map_cli_error, parse_cli_version};
#[cfg(test)]
use setup::{LarkCliProfileSummary, validate_profile_name};

#[cfg(test)]
mod tests;
