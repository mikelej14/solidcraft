use serde_json::{Value, json};

use super::AngleReadoutParams;
use crate::Session;
use crate::schema::tests::{assert_keys, assert_strict_keeps_todays_errors, conforms};
use crate::schema::{ParamCheck, input_schema_for, output_schema_for};

#[test]
fn schemas_and_the_key_check_agree() {
    assert_keys(input_schema_for::<AngleReadoutParams>(), &["enabled"]);
}

#[test]
fn strict_keeps_todays_errors() {
    assert_strict_keeps_todays_errors(&[
        ("view.angle_readout", json!({"enabled": 1})),
        ("view.angle_readout", json!({"enabled": "true"})),
        ("view.angle_readout", json!({"enabled": null})),
    ]);
}

#[test]
fn angle_readout_replies_match_the_output_schema() {
    let schema = output_schema_for::<super::AngleReadoutResult>();
    let schema = schema.as_value();
    let mut s = Session { param_check: ParamCheck::Warn, ..Session::default() };
    for enabled in [true, false] {
        let r = s.execute("view.angle_readout", &json!({"enabled": enabled})).unwrap_or_else(|e| panic!("{e}"));
        assert!(conforms(schema, schema, &r), "{r} doesn't match {schema}");
        assert_eq!(r.get("param_warnings"), None, "{r}");
        assert_eq!(r["enabled"], Value::Bool(enabled));
    }
    assert!(!conforms(schema, schema, &json!({"enabled": "yes"})));
    assert!(!conforms(schema, schema, &json!({})));
}
