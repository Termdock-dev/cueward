use super::*;

#[test]
fn batch_rename_request_schema_roundtrips_exact_entries_and_rejects_unknown_fields() {
    let value = serde_json::json!({"root":"/owned","entries":[{
        "path":"nested/來源\n<external>", "name":"副本\n<external>",
        "expected_version":"source", "expected_parent_version":"parent"
    }]});
    let request: BatchRenameRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), value);
    let mut invalid = value.clone();
    invalid["execute"] = true.into();
    assert!(serde_json::from_value::<BatchRenameRequest>(invalid).is_err());
    let mut invalid = value;
    invalid["entries"][0]["overwrite"] = true.into();
    assert!(serde_json::from_value::<BatchRenameRequest>(invalid).is_err());
}
