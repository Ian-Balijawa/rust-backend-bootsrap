use crate::http::response::ApiError;
use axum::{
    Json,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;
use validator::Validate;

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state)
            .await
            .map_err(|error| ApiError::bad_request(error.body_text()))?;
        value
            .validate()
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        Ok(Self(value))
    }
}
