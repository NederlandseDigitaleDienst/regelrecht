//! HTTP client wrapper for downloading from BWB repository.

use std::time::Duration;

use reqwest::Client;

use crate::config::{DEFAULT_MAX_RESPONSE_SIZE, HTTP_TIMEOUT_SECS};
use crate::error::{HarvesterError, Result};

/// User agent string identifying this harvester.
const USER_AGENT: &str = concat!("regelrecht-harvester/", env!("CARGO_PKG_VERSION"));

/// Maximum number of retry attempts for transient failures.
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (milliseconds).
const RETRY_BASE_DELAY_MS: u64 = 500;

/// Create a configured HTTP client.
///
/// # Returns
/// A `reqwest::Client` configured with appropriate timeout and user agent.
pub fn create_client() -> Result<Client> {
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()?;
    Ok(client)
}

/// Download content from a URL with retry logic.
///
/// Uses exponential backoff for transient failures (network errors, 5xx responses).
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `url` - URL to download from
/// * `max_size` - Maximum response size in bytes
///
/// # Returns
/// Raw bytes of the response body
pub async fn download_bytes(client: &Client, url: &str, max_size: u64) -> Result<Vec<u8>> {
    let mut last_error: Option<String> = None;

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            // Exponential backoff: 500ms, 1000ms, 2000ms
            let delay = RETRY_BASE_DELAY_MS * (1 << (attempt - 1));
            tracing::debug!(attempt, delay_ms = delay, "Retrying after delay");
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status();

                // Retry on server errors (5xx)
                if status.is_server_error() {
                    tracing::warn!(
                        status = %status,
                        attempt = attempt + 1,
                        max_retries = MAX_RETRIES,
                        "Server error, will retry"
                    );
                    last_error = Some(format!("Server error: {status}"));
                    continue;
                }

                // Don't retry client errors (4xx) - they won't succeed
                let response = response.error_for_status()?;

                // Check Content-Length header before downloading
                if let Some(content_length) = response.content_length()
                    && content_length > max_size
                {
                    return Err(HarvesterError::ResponseTooLarge {
                        max_bytes: max_size,
                        actual_bytes: content_length,
                    });
                }

                let bytes = response.bytes().await?;

                // Also check actual size (Content-Length may be missing or wrong)
                if bytes.len() as u64 > max_size {
                    return Err(HarvesterError::ResponseTooLarge {
                        max_bytes: max_size,
                        actual_bytes: bytes.len() as u64,
                    });
                }

                return Ok(bytes.to_vec());
            }
            Err(e) => {
                // Retry on connection/timeout errors
                if e.is_connect() || e.is_timeout() {
                    tracing::warn!(
                        error = %e,
                        attempt = attempt + 1,
                        max_retries = MAX_RETRIES,
                        "Connection error, will retry"
                    );
                    last_error = Some(e.to_string());
                    continue;
                }
                // Other errors (like invalid URL) - don't retry
                return Err(HarvesterError::Http(e));
            }
        }
    }

    // All retries exhausted
    Err(HarvesterError::RetriesExhausted {
        attempts: MAX_RETRIES,
        message: last_error.unwrap_or_else(|| "Unknown error".to_string()),
    })
}

/// Download content from a URL with retry logic using default max size.
///
/// Convenience wrapper around [`download_bytes`] that uses [`DEFAULT_MAX_RESPONSE_SIZE`].
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `url` - URL to download from
///
/// # Returns
/// Raw bytes of the response body
pub async fn download_bytes_default(client: &Client, url: &str) -> Result<Vec<u8>> {
    download_bytes(client, url, DEFAULT_MAX_RESPONSE_SIZE).await
}

/// Convert bytes to a string, preferring strict UTF-8 but falling back to lossy conversion.
///
/// Takes ownership of the `Vec<u8>` to avoid cloning when the input is valid UTF-8.
///
/// # Arguments
/// * `bytes` - The bytes to convert (takes ownership)
/// * `source` - Description of the source for logging purposes
///
/// # Returns
/// A valid UTF-8 string
pub fn bytes_to_string(bytes: Vec<u8>, source: &str) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(source = %source, "Invalid UTF-8, using lossy conversion");
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_client() {
        let client = create_client();
        assert!(client.is_ok());
    }

    #[test]
    fn test_bytes_to_string_valid_utf8() {
        let bytes = b"Hello, world!".to_vec();
        let result = bytes_to_string(bytes, "test");
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_bytes_to_string_invalid_utf8() {
        let bytes = vec![0xff, 0xfe, 0x48, 0x65, 0x6c, 0x6c, 0x6f]; // Invalid UTF-8 prefix + "Hello"
        let result = bytes_to_string(bytes, "test");
        // Should not panic, should return something
        assert!(!result.is_empty());
    }
}
