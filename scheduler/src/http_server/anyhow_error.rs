use axum::http::{Response, StatusCode};
use axum::Json;
use axum::response::IntoResponse;
use serde_json::json;

pub struct HttpError(anyhow::Error);

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": self.0.to_string()
            }))
        ).into_response()
    }
}

impl<E> From<E> for HttpError
    where
        E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}