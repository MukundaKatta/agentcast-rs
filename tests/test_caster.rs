use agentcast::{CastError, Caster};
use serde_json::json;

fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "label": {"type": "string"},
            "confidence": {"type": "number"}
        },
        "required": ["label", "confidence"]
    })
}

#[test]
fn happy_path() {
    let c = Caster::new(&schema()).unwrap();
    let v = c.parse(r#"{"label": "pos", "confidence": 0.9}"#).unwrap();
    assert_eq!(v["label"], "pos");
}

#[test]
fn repairs_markdown_fence() {
    let c = Caster::new(&schema()).unwrap();
    let raw = "```json\n{\"label\": \"pos\", \"confidence\": 0.9}\n```";
    let v = c.parse(raw).unwrap();
    assert_eq!(v["confidence"], 0.9);
}

#[test]
fn repairs_trailing_comma_and_prose() {
    let c = Caster::new(&schema()).unwrap();
    let raw = "Sure! Here's the result: {\"label\": \"pos\", \"confidence\": 0.92,} done.";
    let v = c.parse(raw).unwrap();
    assert_eq!(v["label"], "pos");
}

#[test]
fn invalid_json_returns_error() {
    let c = Caster::new(&schema()).unwrap();
    let err = c.parse("nope, not json").unwrap_err();
    assert!(matches!(err, CastError::InvalidJson(_)));
    let hint = err.for_llm().unwrap();
    assert!(hint.contains("not valid JSON"));
}

#[test]
fn schema_violation_returns_issues() {
    let c = Caster::new(&schema()).unwrap();
    let err = c.parse(r#"{"label": "x"}"#).unwrap_err();
    match &err {
        CastError::Invalid { issues } => assert!(!issues.is_empty()),
        _ => panic!("expected Invalid"),
    }
    let hint = err.for_llm().unwrap();
    assert!(hint.contains("confidence") || hint.contains("required"));
}

#[test]
fn malformed_schema_errors_at_compile() {
    let bad = json!({"type": 42});
    let res = Caster::new(&bad);
    assert!(res.is_err());
}

#[tokio::test]
async fn retry_eventually_succeeds() {
    let c = Caster::new(&schema()).unwrap();
    // First reroll: still missing 'confidence'. Second: valid.
    let attempts = std::sync::atomic::AtomicUsize::new(0);
    let attempts_ref = &attempts;
    let retry_fn = move |_hint: String| async move {
        let n = attempts_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(if n == 0 {
            r#"{"label": "x"}"#.to_string()
        } else {
            r#"{"label": "x", "confidence": 0.5}"#.to_string()
        })
    };

    let v = c
        .parse_with_retry(r#"{"label": "x"}"#, 2, &retry_fn)
        .await
        .unwrap();
    assert_eq!(v["confidence"], 0.5);
    // Two reroll calls were needed.
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn retry_exhausts() {
    let c = Caster::new(&schema()).unwrap();
    let retry_fn = |_: String| async { Ok(r#"{"label": "x"}"#.to_string()) };
    let err = c
        .parse_with_retry(r#"{"label": "x"}"#, 2, &retry_fn)
        .await
        .unwrap_err();
    assert!(matches!(err, CastError::RetryExhausted { .. }));
}

#[tokio::test]
async fn retry_propagates_function_error() {
    let c = Caster::new(&schema()).unwrap();
    let retry_fn = |_: String| async { Err::<String, _>("upstream LLM blew up".to_string()) };
    let err = c
        .parse_with_retry(r#"{"label": "x"}"#, 2, &retry_fn)
        .await
        .unwrap_err();
    assert!(matches!(err, CastError::RetryFailed(_)));
}
