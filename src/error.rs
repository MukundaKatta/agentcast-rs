use thiserror::Error;

/// One concrete validation issue.
#[derive(Debug, Clone)]
pub struct CastIssue {
    /// JSON pointer to the offending field, e.g. `/label`.
    pub pointer: String,
    /// Short human-readable description.
    pub message: String,
}

/// Error returned by [`Caster::parse`](crate::Caster::parse) and friends.
#[derive(Debug, Error)]
pub enum CastError {
    /// Could not parse as JSON even after repair.
    #[error("invalid JSON after repair: {0}")]
    InvalidJson(String),
    /// Parsed but failed schema validation.
    #[error("schema validation failed ({} issue(s))", issues.len())]
    Invalid {
        /// All discovered issues.
        issues: Vec<CastIssue>,
    },
    /// `max_retries` exhausted without producing a valid response.
    #[error("retry budget exhausted after {attempts} attempts; last error: {last}")]
    RetryExhausted {
        /// Number of attempts made.
        attempts: usize,
        /// Last error message.
        last: String,
    },
    /// User-supplied retry function returned an error.
    #[error("retry function failed: {0}")]
    RetryFailed(String),
    /// Compiling the schema failed.
    #[error("invalid schema: {0}")]
    InvalidSchema(String),
}

impl CastError {
    /// Render the issues (if any) as a short hint suitable for handing to
    /// the LLM on the next retry turn. Returns `None` for non-validation errors.
    pub fn for_llm(&self) -> Option<String> {
        match self {
            CastError::Invalid { issues } => {
                let mut s = String::from("Output rejected. Fix and try again:\n");
                for i in issues {
                    s.push_str("  - ");
                    if !i.pointer.is_empty() {
                        s.push_str(&i.pointer);
                        s.push_str(": ");
                    }
                    s.push_str(&i.message);
                    s.push('\n');
                }
                Some(s.trim_end().to_string())
            }
            CastError::InvalidJson(msg) => Some(format!(
                "Output was not valid JSON. Return only a JSON object matching the schema. Parser said: {msg}"
            )),
            _ => None,
        }
    }
}
