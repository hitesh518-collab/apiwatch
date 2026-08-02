use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Null,
    Boolean,
    Number,
    String,
    Object {
        observations: u64,
        properties: BTreeMap<String, ObservedProperty>,
    },
    Map {
        values: Box<Shape>,
    },
    Array {
        items: Box<Shape>,
    },
    Union {
        variants: Vec<Shape>,
    },
    Unknown,
}

pub const DEFAULT_REQUIRED_THRESHOLD: f64 = 0.5;
pub const MINIMUM_OBSERVATION_FLOOR: u64 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedEntry {
    pub shape: Shape,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    #[serde(default)]
    pub first_seen: String,
    #[serde(default)]
    pub last_seen: String,
}

fn default_threshold() -> f64 {
    DEFAULT_REQUIRED_THRESHOLD
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedProperty {
    pub observations: u64,
    pub shape: Box<Shape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservedChangeKind {
    MissingRequiredField,
    IncompatibleShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedChange {
    pub kind: ObservedChangeKind,
    pub path: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TieredEntry {
    pub tier: TieredKind,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TieredKind {
    InsufficientlyObserved,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedVerifyReport {
    pub changes: Vec<ObservedChange>,
    pub tiered: Vec<TieredEntry>,
}

pub fn load_shape(path: &Path) -> Result<Shape> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read observed JSON {}", path.display()))?;
    let value: Value = serde_json::from_str(&input)
        .with_context(|| format!("failed to parse observed JSON {}", path.display()))?;

    Ok(infer(&value))
}

pub fn infer(value: &Value) -> Shape {
    match value {
        Value::Null => Shape::Null,
        Value::Bool(_) => Shape::Boolean,
        Value::Number(_) => Shape::Number,
        Value::String(_) => Shape::String,
        Value::Array(values) => {
            let mut values = values.iter();
            let Some(first) = values.next() else {
                return Shape::Array {
                    items: Box::new(Shape::Unknown),
                };
            };

            let mut items = infer(first);
            for value in values {
                merge(&mut items, &infer(value));
            }

            Shape::Array {
                items: Box::new(items),
            }
        }
        Value::Object(values) => Shape::Object {
            observations: 1,
            properties: values
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        ObservedProperty {
                            observations: 1,
                            shape: Box::new(infer(value)),
                        },
                    )
                })
                .collect(),
        },
    }
}

pub fn apply_map_annotations(shape: &mut Shape, paths: &[String]) -> Result<()> {
    let parsed = paths
        .iter()
        .map(|path| parse_map_path(path).map(|segments| (path, segments)))
        .collect::<Result<Vec<_>>>()?;
    let mut seen = BTreeSet::new();
    for (raw, segments) in &parsed {
        if !seen.insert(segments.clone()) {
            bail!("duplicate map annotation path {raw}");
        }
    }
    for (index, (raw, segments)) in parsed.iter().enumerate() {
        if parsed[..index]
            .iter()
            .any(|(_, previous)| paths_overlap(previous, segments))
        {
            bail!("overlapping map annotation path {raw}");
        }
    }

    let mut annotated = shape.clone();
    for (raw, segments) in parsed {
        annotate_map_at(&mut annotated, raw, &segments)?;
    }
    *shape = annotated;
    Ok(())
}

pub fn merge(existing: &mut Shape, incoming: &Shape) {
    if matches!(incoming, Shape::Unknown) {
        return;
    }
    if matches!(existing, Shape::Unknown) {
        *existing = incoming.clone();
        return;
    }

    match existing {
        Shape::Map { values } if matches!(incoming, Shape::Map { .. }) => {
            let Shape::Map {
                values: incoming_values,
            } = incoming
            else {
                unreachable!("guarded map match must remain a map");
            };
            merge(values, incoming_values);
            return;
        }
        Shape::Map { values } if matches!(incoming, Shape::Object { .. }) => {
            let Shape::Object {
                properties: incoming_properties,
                ..
            } = incoming
            else {
                unreachable!("guarded object match must remain an object");
            };
            let incoming_values = object_value_shape(incoming_properties);
            merge(values, &incoming_values);
            return;
        }
        Shape::Object {
            observations,
            properties,
        } if matches!(incoming, Shape::Object { .. }) => {
            let Shape::Object {
                observations: incoming_observations,
                properties: incoming_properties,
            } = incoming
            else {
                unreachable!("guarded object match must remain an object");
            };

            *observations += incoming_observations;
            for (name, incoming_property) in incoming_properties {
                match properties.get_mut(name) {
                    Some(existing_property) => {
                        existing_property.observations += incoming_property.observations;
                        merge(&mut existing_property.shape, &incoming_property.shape);
                    }
                    None => {
                        properties.insert(name.clone(), incoming_property.clone());
                    }
                }
            }
            return;
        }
        Shape::Array { items } if matches!(incoming, Shape::Array { .. }) => {
            let Shape::Array {
                items: incoming_items,
            } = incoming
            else {
                unreachable!("guarded array match must remain an array");
            };
            merge(items, incoming_items);
            return;
        }
        Shape::Union { variants } => {
            merge_union_variant(variants, incoming);
            return;
        }
        _ if same_kind(existing, incoming) => return,
        _ => {}
    }

    if let Shape::Union { variants } = incoming {
        let mut variants = variants.clone();
        variants.push(existing.clone());
        *existing = canonical_union(variants);
        return;
    }

    *existing = canonical_union(vec![existing.clone(), incoming.clone()]);
}

pub fn verify_with_tiers(expected: &Shape, actual: &Shape, threshold: f64) -> ObservedVerifyReport {
    let changes = compare(expected, actual, threshold);
    let mut tiered = tiered_report(expected, "$", 0, threshold);
    collect_unverified(expected, actual, "$", &mut tiered);
    ObservedVerifyReport { changes, tiered }
}

fn collect_unverified(
    expected: &Shape,
    actual: &Shape,
    path: &str,
    entries: &mut Vec<TieredEntry>,
) {
    match (expected, actual) {
        (
            Shape::Object {
                properties: exp_props,
                ..
            },
            Shape::Object {
                properties: act_props,
                ..
            },
        ) => {
            for (name, _) in act_props
                .iter()
                .filter(|(n, _)| !exp_props.contains_key(*n))
            {
                entries.push(TieredEntry {
                    tier: TieredKind::Unverified,
                    path: format!("{path}.{name}"),
                    detail: "field not in lock".to_string(),
                });
            }
            for (name, exp_prop) in exp_props {
                if let Some(act_prop) = act_props.get(name) {
                    collect_unverified(
                        &exp_prop.shape,
                        &act_prop.shape,
                        &format!("{path}.{name}"),
                        entries,
                    );
                }
            }
        }
        (Shape::Array { items: exp_items }, Shape::Array { items: act_items }) => {
            collect_unverified(exp_items, act_items, &format!("{path}[]"), entries);
        }
        (Shape::Map { values: exp_vals }, Shape::Map { values: act_vals }) => {
            collect_unverified(exp_vals, act_vals, &format!("{path}.<map-value>"), entries);
        }
        (Shape::Union { variants: exp_vars }, _) => {
            for variant in exp_vars {
                collect_unverified(variant, actual, path, entries);
            }
        }
        (_, Shape::Union { variants: act_vars }) => {
            for variant in act_vars {
                collect_unverified(expected, variant, path, entries);
            }
        }
        _ => {}
    }
}

pub fn compare(expected: &Shape, actual: &Shape, threshold: f64) -> Vec<ObservedChange> {
    let mut changes = Vec::new();
    compare_at(expected, actual, "$", 0, 0, threshold, &mut changes);
    changes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    changes
}

pub fn shape_name(shape: &Shape) -> String {
    match shape {
        Shape::Null => "null".to_string(),
        Shape::Boolean => "boolean".to_string(),
        Shape::Number => "number".to_string(),
        Shape::String => "string".to_string(),
        Shape::Object { .. } => "object".to_string(),
        Shape::Map { .. } => "map".to_string(),
        Shape::Array { .. } => "array".to_string(),
        Shape::Unknown => "unknown".to_string(),
        Shape::Union { variants } => variants
            .iter()
            .map(shape_name)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

pub fn is_hardened(parent_observations: u64, property_observations: u64, threshold: f64) -> bool {
    if parent_observations < MINIMUM_OBSERVATION_FLOOR {
        return false;
    }
    if parent_observations == 0 {
        return false;
    }
    let ratio = property_observations as f64 / parent_observations as f64;
    ratio >= threshold
}

pub fn tiered_report(
    shape: &Shape,
    path: &str,
    parent_observations: u64,
    threshold: f64,
) -> Vec<TieredEntry> {
    let mut entries = Vec::new();
    collect_tiered(shape, path, parent_observations, threshold, &mut entries);
    entries
}

fn collect_tiered(
    shape: &Shape,
    path: &str,
    _parent_observations: u64,
    threshold: f64,
    entries: &mut Vec<TieredEntry>,
) {
    match shape {
        Shape::Object {
            observations,
            properties,
        } => {
            if properties.is_empty() {
                entries.push(TieredEntry {
                    tier: TieredKind::InsufficientlyObserved,
                    path: path.to_string(),
                    detail: format!("empty object, seen {observations} time(s)"),
                });
                return;
            }
            for (name, property) in properties {
                let property_path = format!("{path}.{name}");
                if !is_hardened(*observations, property.observations, threshold) {
                    entries.push(TieredEntry {
                        tier: TieredKind::InsufficientlyObserved,
                        path: property_path.clone(),
                        detail: format!(
                            "seen {}/{} time(s), threshold {:.2}",
                            property.observations, observations, threshold
                        ),
                    });
                }
                collect_tiered(
                    &property.shape,
                    &property_path,
                    *observations,
                    threshold,
                    entries,
                );
            }
        }
        Shape::Array { items } if matches!(items.as_ref(), Shape::Unknown) => {
            entries.push(TieredEntry {
                tier: TieredKind::InsufficientlyObserved,
                path: format!("{path}[]"),
                detail: "empty array, no item evidence".to_string(),
            });
        }
        Shape::Array { items } => {
            collect_tiered(
                items,
                &format!("{path}[]"),
                _parent_observations,
                threshold,
                entries,
            );
        }
        Shape::Map { values } => {
            collect_tiered(
                values,
                &format!("{path}.<map-value>"),
                _parent_observations,
                threshold,
                entries,
            );
        }
        Shape::Union { variants } => {
            for variant in variants {
                collect_tiered(variant, path, _parent_observations, threshold, entries);
            }
        }
        _ => {}
    }
}

fn merge_union_variant(variants: &mut Vec<Shape>, incoming: &Shape) {
    match incoming {
        Shape::Union { variants: incoming } => {
            for incoming in incoming {
                merge_union_variant(variants, incoming);
            }
        }
        _ => match variants
            .iter_mut()
            .find(|existing| same_kind(existing, incoming))
        {
            Some(existing) => merge(existing, incoming),
            None => {
                variants.push(incoming.clone());
                variants.sort_by_key(shape_sort_key);
            }
        },
    }
}

fn canonical_union(variants: Vec<Shape>) -> Shape {
    let mut flattened = Vec::new();
    for variant in variants {
        match variant {
            Shape::Union { variants } => flattened.extend(variants),
            variant => flattened.push(variant),
        }
    }

    let mut canonical = Vec::new();
    for variant in flattened {
        merge_union_variant(&mut canonical, &variant);
    }
    canonical.sort_by_key(shape_sort_key);

    if canonical.len() == 1 {
        canonical.pop().expect("single union variant should exist")
    } else {
        Shape::Union {
            variants: canonical,
        }
    }
}

fn compare_at(
    expected: &Shape,
    actual: &Shape,
    path: &str,
    property_obs: u64,
    parent_obs: u64,
    threshold: f64,
    changes: &mut Vec<ObservedChange>,
) {
    if matches!(expected, Shape::Null) && !is_hardened(parent_obs, property_obs, threshold) {
        return;
    }

    if matches!(expected, Shape::Unknown) {
        return;
    }

    if let Shape::Union { variants } = expected {
        if variants.iter().any(|variant| {
            let mut branch_changes = Vec::new();
            compare_at(
                variant,
                actual,
                path,
                property_obs,
                parent_obs,
                threshold,
                &mut branch_changes,
            );
            branch_changes.is_empty()
        }) {
            return;
        }
        incompatible(path, expected, actual, changes);
        return;
    }

    if let Shape::Union { variants } = actual {
        for variant in variants {
            compare_at(
                expected,
                variant,
                path,
                property_obs,
                parent_obs,
                threshold,
                changes,
            );
        }
        return;
    }

    match (expected, actual) {
        (
            Shape::Map {
                values: expected_values,
            },
            Shape::Map {
                values: actual_values,
            },
        ) => compare_at(
            expected_values,
            actual_values,
            path,
            property_obs,
            parent_obs,
            threshold,
            changes,
        ),
        (
            Shape::Map {
                values: expected_values,
            },
            Shape::Object {
                properties: actual_properties,
                ..
            },
        ) => {
            for actual_property in actual_properties.values() {
                compare_at(
                    expected_values,
                    &actual_property.shape,
                    &format!("{path}.<map-value>"),
                    0,
                    0,
                    threshold,
                    changes,
                );
            }
        }
        (
            Shape::Object {
                observations,
                properties,
            },
            Shape::Object {
                properties: actual_properties,
                ..
            },
        ) => {
            for (name, expected_property) in properties {
                let property_path = format!("{path}.{name}");
                match actual_properties.get(name) {
                    Some(actual_property) => compare_at(
                        &expected_property.shape,
                        &actual_property.shape,
                        &property_path,
                        expected_property.observations,
                        *observations,
                        threshold,
                        changes,
                    ),
                    None if is_hardened(
                        *observations,
                        expected_property.observations,
                        threshold,
                    ) =>
                    {
                        changes.push(ObservedChange {
                            kind: ObservedChangeKind::MissingRequiredField,
                            path: property_path,
                            expected: None,
                            actual: None,
                        });
                    }
                    None => {}
                }
            }
        }
        (
            Shape::Array {
                items: expected_items,
            },
            Shape::Array {
                items: actual_items,
            },
        ) => {
            if !matches!(actual_items.as_ref(), Shape::Unknown) {
                compare_at(
                    expected_items,
                    actual_items,
                    &format!("{path}[]"),
                    0,
                    0,
                    threshold,
                    changes,
                );
            }
        }
        _ if same_kind(expected, actual) => {}
        _ => incompatible(path, expected, actual, changes),
    }
}

fn incompatible(path: &str, expected: &Shape, actual: &Shape, changes: &mut Vec<ObservedChange>) {
    changes.push(ObservedChange {
        kind: ObservedChangeKind::IncompatibleShape,
        path: path.to_string(),
        expected: Some(shape_name(expected)),
        actual: Some(shape_name(actual)),
    });
}

fn same_kind(left: &Shape, right: &Shape) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn parse_map_path(raw: &str) -> Result<Vec<String>> {
    if raw == "$" {
        return Ok(Vec::new());
    }

    let Some(remainder) = raw.strip_prefix("$.") else {
        bail!("invalid map annotation path {raw}: expected $ followed by named property segments");
    };
    let mut segments = Vec::new();
    for segment in remainder.split('.') {
        let mut characters = segment.bytes();
        let Some(first) = characters.next() else {
            bail!(
                "invalid map annotation path {raw}: expected $ followed by named property segments"
            );
        };
        if !(first.is_ascii_alphabetic() || first == b'_')
            || !characters.all(|character| character.is_ascii_alphanumeric() || character == b'_')
        {
            bail!(
                "invalid map annotation path {raw}: expected $ followed by named property segments"
            );
        }
        segments.push(segment.to_owned());
    }
    Ok(segments)
}

fn paths_overlap(left: &[String], right: &[String]) -> bool {
    let shortest = left.len().min(right.len());
    left[..shortest] == right[..shortest]
}

fn object_value_shape(properties: &BTreeMap<String, ObservedProperty>) -> Shape {
    let mut values = Shape::Unknown;
    for property in properties.values() {
        merge(&mut values, &property.shape);
    }
    values
}

fn annotate_map_at(shape: &mut Shape, raw: &str, segments: &[String]) -> Result<()> {
    if segments.is_empty() {
        return match shape {
            Shape::Object { properties, .. } => {
                let values = object_value_shape(properties);
                *shape = Shape::Map {
                    values: Box::new(values),
                };
                Ok(())
            }
            Shape::Map { .. } => Ok(()),
            _ => bail!("map annotation path {raw} must target an object"),
        };
    }

    let Shape::Object { properties, .. } = shape else {
        bail!("map annotation path {raw} must target an object");
    };
    let Some(property) = properties.get_mut(&segments[0]) else {
        bail!("map annotation path {raw} does not exist");
    };
    annotate_map_at(&mut property.shape, raw, &segments[1..])
}

fn shape_sort_key(shape: &Shape) -> u8 {
    match shape {
        Shape::Null => 0,
        Shape::Boolean => 1,
        Shape::Number => 2,
        Shape::String => 3,
        Shape::Object { .. } => 4,
        Shape::Map { .. } => 5,
        Shape::Array { .. } => 6,
        Shape::Union { .. } => 7,
        Shape::Unknown => 8,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        apply_map_annotations, compare, infer, merge, shape_name, ObservedChangeKind, Shape,
    };

    #[test]
    fn annotation_converts_an_object_to_a_value_free_map() {
        let mut shape = infer(&json!({
            "by_broker": {
                "acme": {"pnl_pct": 1.2, "session_token": "secret-one"},
                "globex": {"pnl_pct": 3.4, "session_token": "secret-two"}
            }
        }));

        apply_map_annotations(&mut shape, &["$.by_broker".to_owned()])
            .expect("annotation should succeed");
        let rendered = serde_yml::to_string(&shape).expect("shape should serialize");

        assert!(rendered.contains("kind: map"));
        assert!(rendered.contains("pnl_pct"));
        assert!(!rendered.contains("acme"));
        assert!(!rendered.contains("globex"));
        assert!(!rendered.contains("secret-one"));
        assert!(!rendered.contains("secret-two"));
    }

    #[test]
    fn annotation_accepts_root_and_nested_named_property_paths() {
        let mut root = infer(&json!({"acme": 1, "globex": 2}));
        apply_map_annotations(&mut root, &["$".to_owned()]).expect("root map should work");
        assert!(matches!(root, Shape::Map { .. }));

        let mut nested = infer(&json!({"state": {"by_region": {"in": true}}}));
        apply_map_annotations(&mut nested, &["$.state.by_region".to_owned()])
            .expect("nested map should work");
        let Shape::Object { properties, .. } = nested else {
            panic!("root should remain object");
        };
        let state = &properties["state"].shape;
        let Shape::Object { properties, .. } = state.as_ref() else {
            panic!("state should be object");
        };
        assert!(matches!(
            properties["by_region"].shape.as_ref(),
            Shape::Map { .. }
        ));
    }

    #[test]
    fn annotation_rejects_invalid_duplicate_missing_and_non_object_targets() {
        let base = infer(&json!({"by_broker": {"acme": 1}, "scalar": 1}));
        for paths in [
            vec!["$.by_broker".to_owned(), "$.by_broker".to_owned()],
            vec!["$".to_owned(), "$.by_broker".to_owned()],
            vec!["$.missing".to_owned()],
            vec!["$.scalar".to_owned()],
            vec!["$.by-broker".to_owned()],
            vec!["$.by_broker[0]".to_owned()],
            vec!["$.by_broker.*".to_owned()],
            vec!["$..by_broker".to_owned()],
        ] {
            let mut shape = base.clone();
            assert!(
                apply_map_annotations(&mut shape, &paths).is_err(),
                "{paths:?}"
            );
            assert_eq!(shape, base, "invalid paths must leave the shape unchanged");
        }
    }

    #[test]
    fn map_merges_later_plain_objects_and_verify_ignores_key_churn() {
        let mut expected = infer(&json!({"by_broker": {"acme": {"pnl_pct": 1}}}));
        apply_map_annotations(&mut expected, &["$.by_broker".to_owned()])
            .expect("annotation should succeed");
        merge(
            &mut expected,
            &infer(&json!({
                "by_broker": {"globex": {"pnl_pct": 2}}
            })),
        );

        assert!(compare(
            &expected,
            &infer(&json!({
                "by_broker": {"other": {"pnl_pct": 3}}
            })),
            1.0,
        )
        .is_empty());
        assert!(compare(&expected, &infer(&json!({"by_broker": {}})), 1.0).is_empty());

        let changes = compare(
            &expected,
            &infer(&json!({
                "by_broker": {"acme": {"pnl_pct": "wrong"}}
            })),
            1.0,
        );
        assert!(changes.iter().any(|change| {
            change.path == "$.by_broker.<map-value>.pnl_pct"
                && change.expected.as_deref() == Some("number")
                && change.actual.as_deref() == Some("string")
        }));
    }

    #[test]
    fn merge_marks_late_fields_optional_and_sorts_a_scalar_union() {
        let mut shape = infer(&json!({"live_price": 12, "holdings": []}));
        merge(
            &mut shape,
            &infer(&json!({
                "live_price": null,
                "holdings": [{"ticker": "APW"}],
                "error": "temporary"
            })),
        );
        merge(
            &mut shape,
            &infer(&json!({"live_price": 12, "holdings": []})),
        );

        assert!(compare(
            &shape,
            &infer(&json!({
                "live_price": 3,
                "holdings": [{"ticker": "DIFFERENT"}]
            })),
            1.0,
        )
        .is_empty());
        assert!(compare(&shape, &infer(&json!({"holdings": []})), 1.0)
            .iter()
            .any(|change| change.path == "$.live_price"));
        assert_eq!(shape_name(&shape), "object");
    }

    #[test]
    fn inferred_shapes_never_serialize_source_values() {
        let shape = infer(&json!({"token": "super-secret-token", "amount": 42}));
        let rendered = serde_yml::to_string(&shape).expect("shape should serialize");

        assert!(!rendered.contains("super-secret-token"));
        assert!(!rendered.contains("42"));
        assert!(rendered.contains("token"));
        assert!(rendered.contains("string"));
    }

    #[test]
    fn empty_array_accepts_a_populated_array() {
        let expected = infer(&json!({"holdings": []}));
        let actual = infer(&json!({"holdings": [{"ticker": "APW"}]}));

        assert!(compare(&expected, &actual, 1.0).is_empty());
    }

    #[test]
    fn reports_a_string_instead_of_a_locked_number() {
        let expected = infer(&json!({"live_price": 12}));
        let actual = infer(&json!({"live_price": "unavailable"}));
        let changes = compare(&expected, &actual, 1.0);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "$.live_price");
        assert_eq!(changes[0].expected.as_deref(), Some("number"));
        assert_eq!(changes[0].actual.as_deref(), Some("string"));
    }

    #[test]
    fn union_variants_are_sorted_deterministically() {
        let mut shape = infer(&json!(12));
        merge(&mut shape, &infer(&json!(null)));

        let Shape::Union { variants } = shape else {
            panic!("different scalar observations should create a union");
        };
        assert!(matches!(variants.as_slice(), [Shape::Null, Shape::Number]));
    }

    #[test]
    fn null_only_field_with_one_sample_is_lenient() {
        let expected = infer(&json!({"x": null}));
        let changes = compare(&expected, &infer(&json!({"x": "hello"})), 0.5);
        assert!(changes.is_empty(), "single-sample null must be lenient");
    }

    #[test]
    fn null_only_field_with_three_samples_is_hardened() {
        let mut expected = infer(&json!({"x": null}));
        merge(&mut expected, &infer(&json!({"x": null})));
        merge(&mut expected, &infer(&json!({"x": null})));
        let changes = compare(&expected, &infer(&json!({"x": "hello"})), 0.5);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ObservedChangeKind::IncompatibleShape);
    }

    #[test]
    fn low_observation_field_is_optional_with_threshold() {
        let mut expected = infer(&json!({"a": 1, "b": 2}));
        for _ in 0..7 {
            merge(&mut expected, &infer(&json!({"a": 1})));
        }
        let changes = compare(&expected, &infer(&json!({"a": 3})), 0.5);
        assert!(changes.is_empty(), "b below threshold must be optional");
    }

    #[test]
    fn threshold_one_zero_is_binary_required() {
        let mut expected = infer(&json!({"a": 1, "b": 2}));
        merge(&mut expected, &infer(&json!({"a": 1})));
        let changes = compare(&expected, &infer(&json!({"a": 3})), 1.0);
        assert!(changes.is_empty());
        let changes = compare(&expected, &infer(&json!({"a": 3})), 0.0);
        assert!(changes.is_empty(), "all optional at 0.0");
    }
}
