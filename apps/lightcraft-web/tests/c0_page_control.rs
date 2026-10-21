#![cfg(feature = "page-control")]
use lightcraft_web::page_control::*;
use serde_json::json;

#[test]
fn request_envelopes_and_null_params() {
    assert_eq!(parse(r#"{"method":"c0.inspect"}"#).unwrap(), ("c0.inspect".into(), json!({})));
    assert_eq!(parse(r#"{"method":"c0.inspect","params":null}"#).unwrap().1, json!({}));
    for input in ["no", "[]", "{}", r#"{"method":1}"#, r#"{"method":""}"#, r#"{"method":"c0.set","params":[]}"#] {
        assert!(parse(input).is_err(), "{input}");
    }
}

#[test]
fn ordinary_requests_have_one_mib_limit_host_bytes_get_the_larger_limit() {
    let p = json!({"method":"ui.text","params":{"text":"x".repeat(MAX_REQUEST_BYTES)}}).to_string();
    assert!(parse(&p).is_err());
    let file = json!({"method":"c0.openFile","params":{"name":"photo.png","bytes":"a".repeat(MAX_REQUEST_BYTES)}}).to_string();
    assert!(parse(&file).is_ok());
    assert!(parse(&p.replace("ui.text", "c0.inspect")).is_err());
    assert!(parse(&json!({"method":"c0.openFile","params":{"name":"a.png","bytes":"","tree":vec![0;MAX_REQUEST_BYTES]}}).to_string()).is_err());
}

#[test]
fn hostile_synthetic_input_counts_are_bounded() {
    for (method, key, limit) in [
        ("ui.click", "count", MAX_CLICK_COUNT),
        ("ui.clickWidget", "count", MAX_CLICK_COUNT),
        ("ui.drag", "steps", MAX_DRAG_STEPS),
        ("ui.dragWidget", "steps", MAX_DRAG_STEPS),
    ] {
        assert!(parse(&json!({"method":method,"params":{key:limit}}).to_string()).is_ok());
        for value in [json!(limit + 1), json!(u64::MAX), json!(-1), json!("lots")] {
            assert!(parse(&json!({"method":method,"params":{key:value}}).to_string()).is_err());
        }
    }
    assert!(parse(&json!({"method":"ui.pointer","params":{"events":vec![json!({});257]}}).to_string()).is_err());
}

#[test]
fn read_requests_do_not_schedule_extra_passes() {
    for method in ["c0.inspect", "c0.documentBytes", "ui.inspect"] {
        assert!(reads_only(method));
        assert_eq!(inline_passes(method), 0);
    }
    for method in ["ui.key", "ui.clickWidget", "ui.pointer"] {
        assert_eq!(inline_passes(method), MAX_INLINE_PASSES);
    }
    assert!(pump_due(true, 1016.0, 1000.0));
    assert!(!pump_due(true, 1015.0, 1000.0));
    assert!(!pump_due(false, 5000.0, 0.0));
}

#[test]
fn coordinates_cannot_overflow_eguis_float_width() {
    for p in [json!({"x":1e300,"y":0}), json!({"x":"no"}), json!({"events":[{"x":1e300,"y":0}]})] {
        assert!(parse(&json!({"method":"ui.pointer","params":p}).to_string()).is_err());
    }
}
