use inkreaderlink_core::SdkError;
#[derive(Clone, Debug, uniffi::Error, thiserror::Error)]
pub enum SdkOperationError {
    #[error("{detail}")]
    InvalidArgument { detail: String },
    #[error("{detail}")]
    Unsupported { detail: String },
    #[error("credential required: {key}")]
    CredentialRequired { key: String },
    #[error("credential rejected: {key}")]
    CredentialRejected { key: String },
    #[error("device unreachable")]
    Unreachable,
    #[error("operation timed out")]
    Timeout,
    #[error("{detail}")]
    Conflict { detail: String },
    #[error("insufficient storage")]
    InsufficientStorage,
    #[error("{detail}")]
    RemoteFailure { detail: String },
    #[error("{detail}")]
    CommittedWithWarning { detail: String },
    #[error("{detail}")]
    CommittedButCleanupFailed { detail: String },
    #[error("{detail}")]
    RecoveryFailed { detail: String },
}

impl From<SdkError> for SdkOperationError {
    fn from(error: SdkError) -> Self {
        match error {
            SdkError::InvalidArgument(detail) => Self::InvalidArgument { detail },
            SdkError::Unsupported(detail) => Self::Unsupported { detail },
            SdkError::CredentialRequired { key } => Self::CredentialRequired { key },
            SdkError::CredentialRejected { key } => Self::CredentialRejected { key },
            SdkError::Unreachable => Self::Unreachable,
            SdkError::Timeout => Self::Timeout,
            SdkError::Conflict(detail) => Self::Conflict { detail },
            SdkError::InsufficientStorage => Self::InsufficientStorage,
            SdkError::RemoteFailure(detail) => Self::RemoteFailure { detail },
            SdkError::CommittedWithWarning(detail) => Self::CommittedWithWarning { detail },
            SdkError::CommittedButCleanupFailed(detail) => {
                Self::CommittedButCleanupFailed { detail }
            }
            SdkError::RecoveryFailed(detail) => Self::RecoveryFailed { detail },
        }
    }
}
