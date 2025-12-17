use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct InitChunkedUploadRequest {
    pub content_type: String,
    pub metadata: Option<serde_json::Value>,
    pub total_parts_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitChunkedUploadResponse {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbortChunkedUploadRequest {
    pub session_id: String,
}
