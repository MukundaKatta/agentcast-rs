//! Structured-output enforcer for LLM responses.
//!
//! Three-pass pipeline: **repair** → **validate** → optional **retry with
//! LLM**. Repair handles the common cases (markdown fences, trailing
//! commas, surrounding prose) locally; validation uses a JSON Schema you
//! supply. If validation fails *and* you provide a retry function, the
//! pipeline rerolls with an LLM-friendly hint until success or
//! `max_retries` is exhausted.
//!
//! # Quick start
//!
//! ```
//! use agentcast::Caster;
//! use serde_json::json;
//!
//! let schema = json!({
//!     "type": "object",
//!     "properties": {
//!         "label": {"type": "string"},
//!         "confidence": {"type": "number"}
//!     },
//!     "required": ["label", "confidence"]
//! });
//! let caster = Caster::new(&schema).unwrap();
//!
//! // The LLM wrapped its JSON in markdown — repair handles that:
//! let raw = "```json\n{\"label\": \"positive\", \"confidence\": 0.92,}\n```";
//! let value = caster.parse(raw).unwrap();
//! assert_eq!(value["label"], "positive");
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

mod caster;
mod error;
mod repair;

pub use crate::caster::{Caster, RetryFn};
pub use crate::error::{CastError, CastIssue};
pub use crate::repair::repair;
