use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not logged in")]
    NotLoggedIn,
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    #[error("Matrix error: {0}")]
    Matrix(String),
    #[error("{0}")]
    Other(String),
}

impl From<matrix_sdk::Error> for AppError {
    fn from(e: matrix_sdk::Error) -> Self {
        AppError::Matrix(e.to_string())
    }
}

impl From<matrix_sdk::ClientBuildError> for AppError {
    fn from(e: matrix_sdk::ClientBuildError) -> Self {
        AppError::Matrix(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
