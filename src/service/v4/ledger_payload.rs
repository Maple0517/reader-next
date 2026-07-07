use crate::storage::db::v4::claim_repo::ClaimRecord;

pub type LedgerPayloadObject = serde_json::Map<String, serde_json::Value>;

pub fn ensure_claim_type(claim: &ClaimRecord, expected: &str, domain: &str) -> anyhow::Result<()> {
    if claim.claim_type != expected {
        anyhow::bail!("claim {} is not a {}", claim.id, domain);
    }
    Ok(())
}

pub fn ensure_claim_type_in(
    claim: &ClaimRecord,
    expected: &[&str],
    domain: &str,
) -> anyhow::Result<()> {
    if !expected.contains(&claim.claim_type.as_str()) {
        anyhow::bail!("claim {} is not a {}", claim.id, domain);
    }
    Ok(())
}

pub fn claim_payload_object(
    claim: &ClaimRecord,
    domain: &str,
) -> anyhow::Result<LedgerPayloadObject> {
    let value_json = claim
        .value_json
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("{domain} claim {} missing value_json", claim.id))?;
    match serde_json::from_str::<serde_json::Value>(value_json)? {
        serde_json::Value::Object(object) => Ok(object),
        _ => anyhow::bail!("{domain} claim {} value_json must be an object", claim.id),
    }
}

pub fn reject_forbidden_fields(
    claim: &ClaimRecord,
    object: &LedgerPayloadObject,
    domain: &str,
    field_label: &str,
    fields: &[&str],
) -> anyhow::Result<()> {
    for key in fields {
        if object.contains_key(*key) {
            anyhow::bail!(
                "{} claim {} contains {} {}; ledger payload must preserve source observation only",
                domain,
                claim.id,
                field_label,
                key
            );
        }
    }
    Ok(())
}

pub fn required_raw_string_field(
    object: &LedgerPayloadObject,
    domain: &str,
    key: &str,
) -> anyhow::Result<String> {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("{domain} claim missing string field {key}"))
}

pub fn required_string(
    object: &LedgerPayloadObject,
    domain: &str,
    key: &str,
) -> anyhow::Result<String> {
    optional_string(object, key).ok_or_else(|| anyhow::anyhow!("{domain} claim missing {key}"))
}

pub fn optional_string(object: &LedgerPayloadObject, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn optional_string_array(object: &LedgerPayloadObject, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn optional_f64(object: &LedgerPayloadObject, key: &str) -> Option<f64> {
    object.get(key).and_then(serde_json::Value::as_f64)
}
