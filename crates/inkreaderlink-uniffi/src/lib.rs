//! `UniFFI` boundary: model conversion, runtime dispatch, and platform-neutral client exports.
mod client;
mod connection;
mod error;
mod models;
mod progress;
mod runtime;
mod sdk;

pub use client::SdkDeviceClient;
pub use error::SdkOperationError;
pub use models::*;
pub use progress::SdkUploadProgressObserver;
pub use sdk::BooksendSdk;

uniffi::setup_scaffolding!();
