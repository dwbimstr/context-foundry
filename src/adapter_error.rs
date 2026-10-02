//! Named 003 adapter failures that are not core store/retrieval errors.
//!
//! The core's `FoundryError` owns the 001 contract codes. The adapter owns a
//! few additional named codes from 003 and its economics contract
//! (`budget_scope_unsupported`, `host_unsupported`, `receipt_conflict`,
//! `usage_log_full`, ...). They get their own type instead of being smuggled
//! through a core variant, and every consumer prints the same bounded
//! `{code,message,retryable}` JSON (<= 1024 bytes).

use crate::FoundryError;

#[derive(Debug)]
pub enum AdapterError {
    Core(FoundryError),
    Named {
        code: &'static str,
        message: String,
        exit: i32,
    },
}

impl<E> From<E> for AdapterError
where
    FoundryError: From<E>,
{
    fn from(error: E) -> Self {
        Self::Core(FoundryError::from(error))
    }
}

impl AdapterError {
    /// A named adapter failure; exit code 2 (invalid arguments/unsupported).
    pub fn named(code: &'static str, message: impl Into<String>) -> Self {
        Self::Named {
            code,
            message: message.into(),
            exit: 2,
        }
    }

    /// A named runtime failure; exit code 1.
    pub fn runtime(code: &'static str, message: impl Into<String>) -> Self {
        Self::Named {
            code,
            message: message.into(),
            exit: 1,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.code(),
            Self::Named { code, .. } => code,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Core(error) => error.exit_code(),
            Self::Named { exit, .. } => *exit,
        }
    }

    /// Bounded `{code,message,retryable}`; total serialized value <= 1024 bytes.
    pub fn bounded_json(&self) -> String {
        match self {
            Self::Core(error) => error.bounded_json(),
            Self::Named { code, message, .. } => {
                let mut text = message.clone();
                loop {
                    let rendered = serde_json::json!({
                        "code": code,
                        "message": text,
                        "retryable": false,
                    })
                    .to_string();
                    if rendered.len() <= 1024 || text.is_empty() {
                        return rendered;
                    }
                    text.pop();
                }
            }
        }
    }
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.bounded_json())
    }
}

impl std::error::Error for AdapterError {}

pub type AResult<T> = Result<T, AdapterError>;
