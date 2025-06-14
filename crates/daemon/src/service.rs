//! DaemonService implementation for AI inference

use crate::upstream::UpstreamClient;
use hellas_gate_proto::pb::gate::{
    common::v1::{self as common, error::ErrorCode},
    inference::v1::{inference_service_server::InferenceService, *},
};
use std::sync::Arc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info};

/// Implementation of InferenceService for handling AI inference requests
#[derive(Clone, Debug)]
pub struct DaemonServiceImpl {
    upstream_client: Arc<UpstreamClient>,
}

impl DaemonServiceImpl {
    /// Create a new DaemonService implementation
    pub fn new(upstream_client: Arc<UpstreamClient>) -> Self {
        info!("Creating DaemonService instance");
        Self { upstream_client }
    }

    /// Build error response with consistent format
    fn build_error_response(
        request_id: String,
        code: ErrorCode,
        message: String,
    ) -> InferenceResponse {
        InferenceResponse {
            request_id,
            response: Some(inference_response::Response::Error(common::Error {
                code: code as i32,
                message,
                details: std::collections::HashMap::new(),
            })),
        }
    }

    /// Build error response for invalid arguments
    fn build_invalid_argument_error(request_id: String, message: String) -> InferenceResponse {
        Self::build_error_response(request_id, ErrorCode::InvalidArgument, message)
    }

    /// Build error response for internal errors
    fn build_internal_error(request_id: String, message: String) -> InferenceResponse {
        Self::build_error_response(request_id, ErrorCode::InternalError, message)
    }
}

#[tonic::async_trait]
impl InferenceService for DaemonServiceImpl {
    type StreamingInferenceStream = ReceiverStream<Result<InferenceResponse, Status>>;

    async fn streaming_inference(
        &self,
        request: Request<Streaming<InferenceRequest>>,
    ) -> Result<Response<Self::StreamingInferenceStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let upstream_client = self.upstream_client.clone();

        // Process streaming requests
        tokio::spawn(async move {
            while let Some(request_result) = stream.next().await {
                match request_result {
                    Ok(req) => {
                        info!(
                            "Received streaming inference request for model: {}",
                            req.model_id
                        );

                        // Convert protobuf request to upstream format
                        let upstream_request = match convert_inference_request(&req) {
                            Ok(request) => request,
                            Err(e) => {
                                let error_response = Self::build_invalid_argument_error(
                                    req.request_id.clone(),
                                    format!("Invalid request: {}", e),
                                );
                                let _ = tx.send(Ok(error_response)).await;
                                continue;
                            }
                        };

                        // Call upstream inference
                        match upstream_client.chat_completion(upstream_request).await {
                            Ok(upstream_response) => {
                                let response = InferenceResponse {
                                    request_id: req.request_id.clone(),
                                    response: Some(inference_response::Response::Complete(
                                        inference_response::InferenceComplete {
                                            result: Some(upstream_response.response.into()),
                                            metrics: None, // TODO: Extract metrics from upstream response
                                        },
                                    )),
                                };

                                if tx.send(Ok(response)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Upstream inference failed: {}", e);
                                let error_response = Self::build_internal_error(
                                    req.request_id.clone(),
                                    format!("Inference failed: {}", e),
                                );
                                let _ = tx.send(Ok(error_response)).await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error in streaming inference: {}", e);
                        let error_response = Self::build_internal_error(
                            "unknown".to_string(),
                            format!("Streaming error: {}", e),
                        );
                        let _ = tx.send(Ok(error_response)).await;
                        break;
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn inference(
        &self,
        request: Request<InferenceRequest>,
    ) -> Result<Response<InferenceResponse>, Status> {
        let req = request.into_inner();
        info!(
            "Received unary inference request for model: {}",
            req.model_id
        );

        // Convert protobuf request to upstream format
        let upstream_request = convert_inference_request(&req)
            .map_err(|e| Status::invalid_argument(format!("Invalid request: {}", e)))?;

        // Call upstream inference
        match self.upstream_client.chat_completion(upstream_request).await {
            Ok(upstream_response) => {
                let response = InferenceResponse {
                    request_id: req.request_id.clone(),
                    response: Some(inference_response::Response::Complete(
                        inference_response::InferenceComplete {
                            result: Some(upstream_response.response.into()),
                            metrics: None, // TODO: Extract metrics from upstream response
                        },
                    )),
                };

                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Upstream inference failed: {}", e);
                let response = Self::build_internal_error(
                    req.request_id.clone(),
                    format!("Inference failed: {}", e),
                );
                Ok(Response::new(response))
            }
        }
    }

    async fn list_models(
        &self,
        _request: Request<ListModelsRequest>,
    ) -> Result<Response<ListModelsResponse>, Status> {
        info!("Received list models request");

        match self.upstream_client.list_models().await {
            Ok(_models_response) => {
                // TODO: Parse upstream response properly and convert to protobuf format
                let response = ListModelsResponse { models: vec![] };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get models from upstream: {}", e);
                Err(Status::internal(format!(
                    "Failed to retrieve models: {}",
                    e
                )))
            }
        }
    }

    type LoadModelStream = ReceiverStream<Result<LoadModelResponse, Status>>;

    async fn load_model(
        &self,
        request: Request<LoadModelRequest>,
    ) -> Result<Response<Self::LoadModelStream>, Status> {
        let req = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        info!("Loading model: {}", req.model_id);

        // TODO: Implement actual model loading
        // For now, simulate loading with progress updates
        tokio::spawn(async move {
            // Send progress update
            let progress_response = LoadModelResponse {
                model_id: req.model_id.clone(),
                response: Some(load_model_response::Response::Progress(
                    load_model_response::LoadProgress {
                        stage: "downloading".to_string(),
                        progress_percent: 50.0,
                        message: format!("Downloading model {}", req.model_id),
                        bytes_loaded: 1024000,
                        total_bytes: 2048000,
                    },
                )),
            };

            if tx.send(Ok(progress_response)).await.is_err() {
                return;
            }

            // Send completion
            let complete_response = LoadModelResponse {
                model_id: req.model_id.clone(),
                response: Some(load_model_response::Response::Complete(
                    load_model_response::LoadComplete {
                        success: true,
                        model_info: Some(list_models_response::ModelInfo {
                            id: req.model_id.clone(),
                            name: req.model_id.clone(),
                            description: "Loaded model".to_string(),
                            capabilities: vec!["text-generation".to_string()],
                            status: 1, // Loaded
                            specs: None,
                        }),
                    },
                )),
            };

            let _ = tx.send(Ok(complete_response)).await;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn unload_model(
        &self,
        request: Request<UnloadModelRequest>,
    ) -> Result<Response<UnloadModelResponse>, Status> {
        let req = request.into_inner();
        info!("Unloading model: {}", req.model_id);

        // TODO: Implement actual model unloading
        let response = UnloadModelResponse {
            success: true,
            message: format!("Model {} unloaded successfully", req.model_id),
        };

        Ok(Response::new(response))
    }

    async fn get_model_status(
        &self,
        request: Request<ModelStatusRequest>,
    ) -> Result<Response<ModelStatusResponse>, Status> {
        let req = request.into_inner();
        info!("Getting status for model: {}", req.model_id);

        // TODO: Implement actual model status checking
        let response = ModelStatusResponse {
            result: Some(model_status_response::Result::Status(
                model_status_response::ModelStatus {
                    model_id: req.model_id.clone(),
                    status: 2, // Loaded (from ModelStatus enum)
                    active_requests: 0,
                    memory_usage: 1000000000, // 1GB
                    last_activity: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                },
            )),
        };

        Ok(Response::new(response))
    }
}

/// Convert protobuf InferenceRequest to upstream format
fn convert_inference_request(
    req: &InferenceRequest,
) -> Result<crate::upstream::InferenceRequest, String> {
    // Extract and convert input data using proto converter
    let input_json = req.input_data.as_ref().ok_or("Missing input data")?;

    // Use existing converter instead of manual parsing
    let input_value: serde_json::Value = input_json
        .clone()
        .try_into()
        .map_err(|e| format!("Invalid JSON in input_data: {}", e))?;

    crate::upstream::InferenceRequest::new(input_value)
        .map_err(|e| format!("Failed to create upstream request: {}", e))
}
