# agentcast

[![crates.io](https://img.shields.io/crates/v/agentcast.svg)](https://crates.io/crates/agentcast)
[![docs.rs](https://docs.rs/agentcast/badge.svg)](https://docs.rs/agentcast)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Structured-output enforcer for LLM responses. Three-pass: **repair** → **validate** → **retry-with-LLM** (optional). BYO-LLM, BYO-schema.

```toml
[dependencies]
agentcast = "0.1"
```

## Why

Models advertise JSON, then wrap it in markdown fences, leak prose around it, drop a trailing comma, or miss a required field. Without a buffer, you crash. `agentcast` gives you the buffer:

1. **Repair** — strip ```json fences, extract the largest balanced object from surrounding prose, remove trailing commas.
2. **Validate** — JSON Schema you supply (the same one you sent the model in your tool def).
3. **Retry** — if validation still fails, hand the model a short hint and reroll. Loop up to `max_retries`.

## Quick start

```rust
use agentcast::Caster;
use serde_json::json;

let schema = json!({
    "type": "object",
    "properties": {
        "label": {"type": "string"},
        "confidence": {"type": "number"}
    },
    "required": ["label", "confidence"]
});
let caster = Caster::new(&schema).unwrap();

let raw = "```json\n{\"label\": \"positive\", \"confidence\": 0.92,}\n```";
let value = caster.parse(raw).unwrap();   // repair handles the fence + trailing comma
assert_eq!(value["label"], "positive");
```

## With LLM retry

```rust,no_run
# use agentcast::Caster;
# use serde_json::json;
# tokio_test::block_on(async {
let caster = Caster::new(&json!({})).unwrap();

// You supply a closure that re-prompts the model with the hint:
let retry_fn = |hint: String| async move {
    // call your LLM, returning Result<String, String>
    let new_response = your_llm_call(hint).await?;
    Ok::<_, String>(new_response)
};

let value = caster
    .parse_with_retry(
        "model's first attempt that doesn't validate",
        /* max_retries */ 2,
        &retry_fn,
    )
    .await
    .unwrap();
# async fn your_llm_call(_h: String) -> Result<String, String> { Ok("{}".into()) }
# });
```

## What's in the hint sent back to the model

```text
Output rejected. Fix and try again:
  - /confidence: required property missing
  - /label: must be one of: ["positive", "negative"]
```

Short, action-oriented, and what you sent in the tool def is the spec — nothing fancy.

## What it doesn't do

- Doesn't call any LLM — you supply the retry function.
- Doesn't write the schema for you — bring the same one you used in your `tools` payload.
- Doesn't infer types into a `T: DeserializeOwned`; returns `serde_json::Value`. Add `serde_json::from_value(value)` when you have a typed shape.

## Sibling: JS `@mukundakatta/agentcast`

JS users: see [@mukundakatta/agentcast](https://www.npmjs.com/package/@mukundakatta/agentcast) on npm.

## License

MIT
