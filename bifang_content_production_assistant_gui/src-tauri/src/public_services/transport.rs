//! 公共服务 HTTP transport；不重试、不跟随重定向、不记录载荷或凭据。

use std::{future::Future, pin::Pin, time::Duration};

use bifang_content_production_assistant_core::public_services::DailyActivePayload;

use super::config::{TelemetryConfig, VersionApiConfig};
use super::{MAX_TELEMETRY_BYTES, MAX_UPDATE_BYTES, PublicServicesError};

pub(super) trait PublicServicesTransport: Send + Sync {
    fn send_telemetry(&self, payload: DailyActivePayload) -> TransportFuture<'_, bool>;
    fn fetch_update(&self) -> TransportFuture<'_, Result<Vec<u8>, PublicServicesError>>;
    fn telemetry_configured(&self) -> bool;
    fn updates_configured(&self) -> bool;
}

pub(super) type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(super) struct ReqwestPublicServicesTransport {
    client: reqwest::Client,
    telemetry: Option<TelemetryConfig>,
    version_api: Option<VersionApiConfig>,
}

impl ReqwestPublicServicesTransport {
    pub(super) fn create() -> std::sync::Arc<dyn PublicServicesTransport> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!(
                "bifang-content-production-assistant/",
                env!("CARGO_PKG_VERSION")
            ))
            .build();
        match client {
            Ok(client) => std::sync::Arc::new(Self {
                client,
                telemetry: TelemetryConfig::registered(),
                version_api: VersionApiConfig::registered(),
            }),
            Err(_) => std::sync::Arc::new(DisabledPublicServicesTransport),
        }
    }
}

struct DisabledPublicServicesTransport;

impl PublicServicesTransport for DisabledPublicServicesTransport {
    fn send_telemetry(&self, _payload: DailyActivePayload) -> TransportFuture<'_, bool> {
        Box::pin(async { false })
    }

    fn fetch_update(&self) -> TransportFuture<'_, Result<Vec<u8>, PublicServicesError>> {
        Box::pin(async { Err(PublicServicesError::UpdateNotConfigured) })
    }

    fn telemetry_configured(&self) -> bool {
        false
    }

    fn updates_configured(&self) -> bool {
        false
    }
}

impl PublicServicesTransport for ReqwestPublicServicesTransport {
    fn send_telemetry(&self, payload: DailyActivePayload) -> TransportFuture<'_, bool> {
        Box::pin(async move {
            let Some(config) = self.telemetry.as_ref() else {
                return false;
            };
            let Ok(response) = self
                .client
                .post(config.endpoint())
                .header(reqwest::header::ACCEPT, "application/json")
                .json(&payload)
                .send()
                .await
            else {
                return false;
            };
            if response.status() != reqwest::StatusCode::ACCEPTED
                || !has_json_content_type(&response)
                || response
                    .content_length()
                    .is_some_and(|length| length > MAX_TELEMETRY_BYTES as u64)
            {
                return false;
            }
            let Ok(bytes) = read_bounded(response, MAX_TELEMETRY_BYTES).await else {
                return false;
            };
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|value| value.as_object().cloned())
                .is_some_and(|object| {
                    object.len() == 1 && object.get("ok") == Some(&serde_json::Value::Bool(true))
                })
        })
    }

    fn fetch_update(&self) -> TransportFuture<'_, Result<Vec<u8>, PublicServicesError>> {
        Box::pin(async move {
            let config = self
                .version_api
                .as_ref()
                .ok_or(PublicServicesError::UpdateNotConfigured)?;
            let response = self
                .client
                .execute(config.build_request(&self.client)?)
                .await
                .map_err(|_| PublicServicesError::NetworkUnavailable)?;
            if response.status() != reqwest::StatusCode::OK
                || !has_json_content_type(&response)
                || response
                    .content_length()
                    .is_some_and(|length| length > MAX_UPDATE_BYTES as u64)
            {
                return Err(PublicServicesError::InvalidResponse);
            }
            read_bounded(response, MAX_UPDATE_BYTES).await
        })
    }

    fn telemetry_configured(&self) -> bool {
        self.telemetry.is_some()
    }

    fn updates_configured(&self) -> bool {
        self.version_api.is_some()
    }
}

fn has_json_content_type(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"))
}

async fn read_bounded(
    mut response: reqwest::Response,
    maximum: usize,
) -> Result<Vec<u8>, PublicServicesError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| PublicServicesError::NetworkUnavailable)?
    {
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err(PublicServicesError::InvalidResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
