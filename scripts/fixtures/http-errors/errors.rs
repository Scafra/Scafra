use scafra::prelude::*;
use scafra::web::axum::response::{IntoResponse, Response};

pub struct DomainError;
impl IntoResponse for DomainError {
    fn into_response(self) -> Response {
        AppError::public_validation("quantity must be positive").into_response()
    }
}

#[derive(serde::Deserialize)]
pub struct Input {
    pub quantity: u32,
}

#[controller("/errors", public)]
pub struct Errors;
#[routes]
impl Errors {
    #[get("/{case}")]
    pub async fn failure(&self, case: Path<String>) -> scafra::Result<&'static str> {
        Err(match case.0.as_str() {
            "not-found" => AppError::NotFound,
            "validation" => AppError::Validation("issue12-private-database://password/path".into()),
            _ => AppError::internal(std::io::Error::other(
                "issue12-private-database://password/path",
            )),
        })
    }
    #[get("/typed")]
    pub async fn typed(&self) -> std::result::Result<&'static str, DomainError> {
        Err(DomainError)
    }
    #[get("/slow")]
    pub async fn slow(&self) -> &'static str {
        std::future::pending().await
    }
    #[post("/json")]
    pub async fn json(&self, input: JsonBody<Input>) -> String {
        input.0.quantity.to_string()
    }
    #[post("/native")]
    pub async fn native(&self, input: Json<Input>) -> String {
        input.0.quantity.to_string()
    }
}
