use serde_yaml::Value;

/// Render guarded YAML while preserving its scalar values and native enum tags.
pub(crate) fn external_yaml_envelope(source: &str, value: &Value) -> Result<String, String> {
    Ok(super::external_frame(source, &flow_yaml(value)?))
}

fn flow_yaml(value: &Value) -> Result<String, String> {
    match value {
        // JSON double-quoted scalars are also valid YAML; angle escapes are decoded rather than literal.
        Value::String(text) => serde_json::to_string(text)
            .map(|json| super::escape_json_angles(&json))
            .map_err(|e| e.to_string()),
        Value::Sequence(values) => {
            let values = values
                .iter()
                .map(flow_yaml)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", values.join(", ")))
        }
        Value::Mapping(values) => {
            let entries = values
                .iter()
                .map(|(key, value)| Ok(format!("{}: {}", flow_yaml(key)?, flow_yaml(value)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(format!("{{{}}}", entries.join(", ")))
        }
        Value::Tagged(tagged) => {
            let tag = tagged.tag.to_string();
            if tag.contains(['<', '>']) {
                return Err("unsupported YAML tag in external output".into());
            }
            Ok(format!("{tag} {}", flow_yaml(&tagged.value)?))
        }
        scalar => serde_yaml::to_string(scalar)
            .map(|text| text.trim_end().to_owned())
            .map_err(|e| e.to_string()),
    }
}
