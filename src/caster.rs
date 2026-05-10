use crate::error::{CastError, CastIssue};
use crate::repair::repair;
use async_trait::async_trait;
use serde_json::Value;
use std::pin::Pin;

/// User-supplied retry function: takes the LLM-friendly hint, returns the
/// model's next attempt as raw text.
#[async_trait]
pub trait RetryFn: Send + Sync {
    /// Reroll the model with the given hint.
    async fn reroll(&self, hint: String) -> Result<String, String>;
}

#[async_trait]
impl<F, Fut> RetryFn for F
where
    F: Fn(String) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<String, String>> + Send,
{
    async fn reroll(&self, hint: String) -> Result<String, String> {
        (self)(hint).await
    }
}

/// Three-pass caster: repair → validate → optional LLM retry.
pub struct Caster {
    schema: jsonschema::Validator,
}

impl Caster {
    /// Compile a caster from a JSON Schema document.
    pub fn new(schema: &Value) -> Result<Self, CastError> {
        let v = jsonschema::validator_for(schema)
            .map_err(|e| CastError::InvalidSchema(e.to_string()))?;
        Ok(Self { schema: v })
    }

    /// Parse `raw` once: repair → JSON parse → schema validate.
    /// Does **not** retry — for that, use [`parse_with_retry`](Self::parse_with_retry).
    pub fn parse(&self, raw: &str) -> Result<Value, CastError> {
        let cleaned = repair(raw);
        let value: Value = serde_json::from_str(&cleaned)
            .map_err(|e| CastError::InvalidJson(e.to_string()))?;
        self.validate(&value).map(|_| value)
    }

    /// Parse with up to `max_retries` rerolls via the user-supplied function.
    ///
    /// On each failure, hands the model a short LLM-friendly hint (see
    /// [`CastError::for_llm`]) and uses the next response.
    pub async fn parse_with_retry(
        &self,
        raw: &str,
        max_retries: usize,
        retry_fn: &dyn RetryFn,
    ) -> Result<Value, CastError> {
        let mut current = raw.to_string();
        let mut last_err: Option<CastError> = None;
        for attempt in 0..=max_retries {
            match self.parse(&current) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if attempt == max_retries {
                        last_err = Some(e);
                        break;
                    }
                    let hint = e
                        .for_llm()
                        .unwrap_or_else(|| "Output rejected. Try again.".to_string());
                    let next = retry_fn
                        .reroll(hint)
                        .await
                        .map_err(CastError::RetryFailed)?;
                    current = next;
                    last_err = Some(e);
                }
            }
        }
        Err(CastError::RetryExhausted {
            attempts: max_retries + 1,
            last: last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "unknown".into()),
        })
    }

    fn validate(&self, value: &Value) -> Result<(), CastError> {
        let issues: Vec<CastIssue> = self
            .schema
            .iter_errors(value)
            .map(|err| CastIssue {
                pointer: err.instance_path.to_string(),
                message: err.to_string(),
            })
            .collect();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CastError::Invalid { issues })
        }
    }
}

// `Pin` is unused publicly but mentioned here so the doc-tests trivially compile.
#[allow(dead_code)]
fn _doc_pin_marker() -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async {})
}
