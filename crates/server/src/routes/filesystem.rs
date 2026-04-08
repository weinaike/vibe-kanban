use axum::{
    Router,
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{Json as ResponseJson, Response},
    routing::get,
};
use deployment::Deployment;
use serde::Deserialize;
use services::services::filesystem::{DirectoryEntry, DirectoryListResponse, FileReadResponse, FilesystemError};
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct ListDirectoryQuery {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadFileQuery {
    path: String,
}

pub async fn list_directory(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListDirectoryQuery>,
) -> Result<ResponseJson<ApiResponse<DirectoryListResponse>>, ApiError> {
    match deployment.filesystem().list_directory(query.path).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(FilesystemError::DirectoryDoesNotExist) => {
            Ok(ResponseJson(ApiResponse::error("Directory does not exist")))
        }
        Err(FilesystemError::PathIsNotDirectory) => {
            Ok(ResponseJson(ApiResponse::error("Path is not a directory")))
        }
        Err(FilesystemError::Io(e)) => {
            tracing::error!("Failed to read directory: {}", e);
            Ok(ResponseJson(ApiResponse::error(&format!(
                "Failed to read directory: {}",
                e
            ))))
        }
    }
}

pub async fn list_git_repos(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListDirectoryQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<DirectoryEntry>>>, ApiError> {
    let res = if let Some(ref path) = query.path {
        deployment
            .filesystem()
            .list_git_repos(Some(path.clone()), 800, 1200, Some(3))
            .await
    } else {
        deployment
            .filesystem()
            .list_common_git_repos(800, 1200, Some(4))
            .await
    };
    match res {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(FilesystemError::DirectoryDoesNotExist) => {
            Ok(ResponseJson(ApiResponse::error("Directory does not exist")))
        }
        Err(FilesystemError::PathIsNotDirectory) => {
            Ok(ResponseJson(ApiResponse::error("Path is not a directory")))
        }
        Err(FilesystemError::Io(e)) => {
            tracing::error!("Failed to read directory: {}", e);
            Ok(ResponseJson(ApiResponse::error(&format!(
                "Failed to read directory: {}",
                e
            ))))
        }
    }
}

pub async fn read_file(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ReadFileQuery>,
) -> Result<ResponseJson<ApiResponse<FileReadResponse>>, ApiError> {
    match deployment.filesystem().read_file(query.path).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(FilesystemError::Io(e)) => {
            tracing::error!("Failed to read file: {}", e);
            Ok(ResponseJson(ApiResponse::error(&format!(
                "Failed to read file: {}",
                e
            ))))
        }
        _ => Ok(ResponseJson(ApiResponse::error("Failed to read file"))),
    }
}

pub async fn read_file_binary(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ReadFileQuery>,
) -> Response {
    match deployment.filesystem().read_file_binary(query.path).await {
        Ok((bytes, content_type)) => {
            let body = Body::from(bytes);
            Response::builder()
                .header(header::CONTENT_TYPE, content_type)
                .body(body)
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Failed to build response"))
                        .unwrap()
                })
        }
        Err(FilesystemError::Io(e)) => {
            tracing::error!("Failed to read binary file: {}", e);
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!("Failed to read file: {}", e)))
                .unwrap()
        }
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Failed to read file"))
            .unwrap(),
    }
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/filesystem/directory", get(list_directory))
        .route("/filesystem/git-repos", get(list_git_repos))
        .route("/filesystem/read-file", get(read_file))
        .route("/filesystem/read-file-binary", get(read_file_binary))
}
