//! RelayService implementation for handling DNS challenges and domain registration

use crate::cloudflare::CloudflareDnsManager;
use hellas_gate_proto::pb::gate::{
    common::v1::{self as common, error::ErrorCode},
    relay::v1::{check_dns_propagation_response, relay_service_server::RelayService, *},
};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

/// Validates that a domain follows the pattern [id].private.hellas.ai
/// where [id] is a valid identifier (alphanumeric, hyphens, underscores)
fn is_valid_challenge_domain(domain: &str) -> bool {
    // Check if domain ends with .private.hellas.ai
    if !domain.ends_with(".private.hellas.ai") {
        return false;
    }

    // Extract the ID part (everything before .private.hellas.ai)
    let id_part = domain.strip_suffix(".private.hellas.ai").unwrap();

    // Validate the ID part:
    // - Must not be empty
    // - Must not contain dots (only one level of subdomain allowed)
    // - Must contain only alphanumeric characters, hyphens, and underscores
    if id_part.is_empty() || id_part.contains('.') {
        return false;
    }

    // Check that all characters are valid
    id_part
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Implementation of RelayService for handling DNS challenges
#[derive(Clone, Debug)]
pub struct RelayServiceImpl {
    dns_manager: Arc<CloudflareDnsManager>,
}

impl RelayServiceImpl {
    /// Create a new RelayService implementation
    pub fn new(
        dns_manager: Arc<CloudflareDnsManager>,
    ) -> Self {
        info!("Creating RelayService instance");
        Self {
            dns_manager,
        }
    }
}

#[tonic::async_trait]
impl RelayService for RelayServiceImpl {
    type CreateDnsChallengeStream = ReceiverStream<Result<CreateDnsChallengeResponse, Status>>;

    async fn create_dns_challenge(
        &self,
        request: Request<CreateDnsChallengeRequest>,
    ) -> Result<Response<Self::CreateDnsChallengeStream>, Status> {
        let req = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        // Security: Only allow DNS challenges for domains matching [id].private.hellas.ai
        if !is_valid_challenge_domain(&req.domain) {
            error!("DNS challenge rejected for invalid domain: {}", req.domain);
            return Err(Status::permission_denied(format!(
                "DNS challenges only allowed for domains matching [id].private.hellas.ai, got: {}",
                req.domain
            )));
        }

        info!(
            "Creating DNS challenge for domain: {} with value: {}",
            req.domain, req.txt_value
        );

        // Send progress update
        let progress_response = CreateDnsChallengeResponse {
            response: Some(create_dns_challenge_response::Response::Progress(
                create_dns_challenge_response::ChallengeProgress {
                    stage: "creating".to_string(),
                    message: "Creating DNS TXT record".to_string(),
                    estimated_seconds_remaining: 30,
                },
            )),
        };

        if tx.send(Ok(progress_response)).await.is_err() {
            return Err(Status::internal("Stream closed"));
        }

        let final_response = match self
            .dns_manager
            .create_dns_challenge(&req.domain, &req.txt_value)
            .await
        {
            Ok(record_id) => {
                info!(
                    "Successfully created DNS TXT record for {}: {}",
                    req.domain, record_id
                );
                CreateDnsChallengeResponse {
                    response: Some(create_dns_challenge_response::Response::Complete(
                        create_dns_challenge_response::ChallengeComplete {
                            record_id: record_id.to_string(),
                            propagation_estimate_seconds: 60, // Standard DNS propagation time
                            verified: true,
                        },
                    )),
                }
            }
            Err(e) => {
                error!("Failed to create DNS TXT record for {}: {}", req.domain, e);
                CreateDnsChallengeResponse {
                    response: Some(create_dns_challenge_response::Response::Error(
                        common::Error {
                            code: ErrorCode::DnsChallengeFailed as i32,
                            message: format!("DNS challenge failed: {}", e),
                            details: std::collections::HashMap::new(),
                        },
                    )),
                }
            }
        };

        if tx.send(Ok(final_response)).await.is_err() {
            return Err(Status::internal("Stream closed"));
        }

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn cleanup_dns_challenge(
        &self,
        request: Request<CleanupDnsChallengeRequest>,
    ) -> Result<Response<CleanupDnsChallengeResponse>, Status> {
        let req = request.into_inner();

        // Security: Only allow DNS cleanup for domains matching [id].private.hellas.ai
        if !is_valid_challenge_domain(&req.domain) {
            error!("DNS cleanup rejected for invalid domain: {}", req.domain);
            return Err(Status::permission_denied(format!(
                "DNS cleanup only allowed for domains matching [id].private.hellas.ai, got: {}",
                req.domain
            )));
        }

        info!("Cleaning up DNS challenge for domain: {}", req.domain);

        let result = if !req.record_id.is_empty() {
            // Cleanup by specific record ID if provided
            self.dns_manager.cleanup_dns_challenge(&req.record_id).await
        } else {
            // Cleanup by domain
            self.dns_manager
                .cleanup_dns_challenge_by_domain(&req.domain)
                .await
        };

        let response = match result {
            Ok(()) => {
                info!("Successfully cleaned up DNS TXT record for {}", req.domain);
                CleanupDnsChallengeResponse {
                    result: Some(cleanup_dns_challenge_response::Result::Success(
                        cleanup_dns_challenge_response::CleanupSuccess { records_removed: 1 },
                    )),
                }
            }
            Err(e) => {
                warn!("Failed to cleanup DNS TXT record for {}: {}", req.domain, e);
                // Don't fail cleanup operations - return success anyway
                CleanupDnsChallengeResponse {
                    result: Some(cleanup_dns_challenge_response::Result::Success(
                        cleanup_dns_challenge_response::CleanupSuccess { records_removed: 0 },
                    )),
                }
            }
        };

        Ok(Response::new(response))
    }

    type CheckDnsPropagationStream = ReceiverStream<Result<CheckDnsPropagationResponse, Status>>;

    async fn check_dns_propagation(
        &self,
        request: Request<CheckDnsPropagationRequest>,
    ) -> Result<Response<Self::CheckDnsPropagationStream>, Status> {
        let req = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        // Security: Only allow DNS propagation checks for domains matching [id].private.hellas.ai
        if !is_valid_challenge_domain(&req.domain) {
            error!(
                "DNS propagation check rejected for invalid domain: {}",
                req.domain
            );
            return Err(Status::permission_denied(
                format!("DNS propagation checks only allowed for domains matching [id].private.hellas.ai, got: {}", req.domain)
            ));
        }

        info!(
            "Starting DNS propagation check for domain: {} with value: {}",
            req.domain, req.expected_value
        );

        let dns_manager = self.dns_manager.clone();
        let domain = req.domain;
        let expected_value = req.expected_value;
        let timeout_seconds = if req.timeout_seconds > 0 {
            req.timeout_seconds
        } else {
            300
        }; // Default 5 minutes

        tokio::spawn(async move {
            let start_time = std::time::Instant::now();
            let max_attempts = (timeout_seconds / 10).max(1) as u32; // Check every 10 seconds
            let mut attempt = 1;

            loop {
                // Send progress update
                let progress_response = CheckDnsPropagationResponse {
                    response: Some(check_dns_propagation_response::Response::Progress(
                        check_dns_propagation_response::PropagationProgress {
                            stage: "checking".to_string(),
                            message: format!(
                                "Checking DNS propagation (attempt {}/{})",
                                attempt, max_attempts
                            ),
                            attempt: attempt as i32,
                            max_attempts: max_attempts as i32,
                            next_check_seconds: if attempt < max_attempts { 10 } else { 0 },
                        },
                    )),
                };

                if tx.send(Ok(progress_response)).await.is_err() {
                    return; // Stream closed
                }

                // Check DNS propagation
                match dns_manager
                    .check_dns_propagation(&domain, &expected_value)
                    .await
                {
                    Ok(true) => {
                        // Success! DNS has propagated
                        let elapsed = start_time.elapsed().as_secs() as i32;
                        let complete_response = CheckDnsPropagationResponse {
                            response: Some(check_dns_propagation_response::Response::Complete(
                                check_dns_propagation_response::PropagationComplete {
                                    propagated: true,
                                    total_attempts: attempt as i32,
                                    elapsed_seconds: elapsed,
                                },
                            )),
                        };

                        let _ = tx.send(Ok(complete_response)).await;
                        info!(
                            "DNS propagation confirmed for {} after {} attempts ({} seconds)",
                            domain, attempt, elapsed
                        );
                        return;
                    }
                    Ok(false) => {
                        // Not yet propagated
                        if attempt >= max_attempts {
                            // Timeout
                            let elapsed = start_time.elapsed().as_secs() as i32;
                            let complete_response = CheckDnsPropagationResponse {
                                response: Some(check_dns_propagation_response::Response::Complete(
                                    check_dns_propagation_response::PropagationComplete {
                                        propagated: false,
                                        total_attempts: attempt as i32,
                                        elapsed_seconds: elapsed,
                                    },
                                )),
                            };

                            let _ = tx.send(Ok(complete_response)).await;
                            warn!(
                                "DNS propagation timed out for {} after {} attempts ({} seconds)",
                                domain, attempt, elapsed
                            );
                            return;
                        }
                    }
                    Err(e) => {
                        // Error during check
                        let error_response = CheckDnsPropagationResponse {
                            response: Some(check_dns_propagation_response::Response::Error(
                                common::Error {
                                    code: common::error::ErrorCode::DnsChallengeFailed as i32,
                                    message: format!("DNS propagation check failed: {}", e),
                                    details: std::collections::HashMap::new(),
                                },
                            )),
                        };

                        let _ = tx.send(Ok(error_response)).await;
                        error!("DNS propagation check error for {}: {}", domain, e);
                        return;
                    }
                }

                // Wait before next attempt
                attempt += 1;
                if attempt <= max_attempts {
                    // Send waiting progress
                    let wait_response = CheckDnsPropagationResponse {
                        response: Some(check_dns_propagation_response::Response::Progress(
                            check_dns_propagation_response::PropagationProgress {
                                stage: "waiting".to_string(),
                                message: "Waiting 10 seconds before next check".to_string(),
                                attempt: attempt as i32,
                                max_attempts: max_attempts as i32,
                                next_check_seconds: 10,
                            },
                        )),
                    };

                    if tx.send(Ok(wait_response)).await.is_err() {
                        return; // Stream closed
                    }

                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_rate_limit(
        &self,
        _request: Request<RateLimitRequest>,
    ) -> Result<Response<RateLimitResponse>, Status> {
        // Return default rate limiting info
        let response = RateLimitResponse {
            rate_limit: Some(rate_limit_response::RateLimit {
                max_concurrent: 5,
                current_count: 0,
                requests_per_hour: 100,
                requests_used: 0,
                reset_timestamp: chrono::Utc::now().timestamp() + 3600,
            }),
        };

        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_validation() {
        // Valid domains
        assert!(is_valid_challenge_domain("user1.private.hellas.ai"));
        assert!(is_valid_challenge_domain("test-node.private.hellas.ai"));
        assert!(is_valid_challenge_domain("node_123.private.hellas.ai"));
        assert!(is_valid_challenge_domain("a.private.hellas.ai"));

        // Invalid domains - wrong suffix
        assert!(!is_valid_challenge_domain("user1.private.hellas.com"));
        assert!(!is_valid_challenge_domain("user1.public.hellas.ai"));
        assert!(!is_valid_challenge_domain("user1.hellas.ai"));
        assert!(!is_valid_challenge_domain("example.com"));

        // Invalid domains - empty ID
        assert!(!is_valid_challenge_domain(".private.hellas.ai"));
        assert!(!is_valid_challenge_domain("private.hellas.ai"));

        // Invalid domains - multiple subdomains
        assert!(!is_valid_challenge_domain("sub.user1.private.hellas.ai"));
        assert!(!is_valid_challenge_domain("a.b.c.private.hellas.ai"));

        // Invalid domains - invalid characters
        assert!(!is_valid_challenge_domain("user@1.private.hellas.ai"));
        assert!(!is_valid_challenge_domain("user1$.private.hellas.ai"));
        assert!(!is_valid_challenge_domain("user 1.private.hellas.ai"));
    }
}
