use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    Null,
    Boolean,
    Number,
    String,
    Object {
        observations: u64,
        properties: BTreeMap<String, ObservedProperty>,
    },
    Array {
        items: Box<Shape>,
    },
    Union {
        variants: Vec<Shape>,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

pub fn merge(existing: &mut Shape, incoming: &Shape) {
    if matches!(incoming, Shape::Unknown) {
        return;
    }
    if matches!(existing, Shape::Unknown) {
        *existing = incoming.clone();
        return;
    }

    match existing {
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

pub fn compare(expected: &Shape, actual: &Shape) -> Vec<ObservedChange> {
    let mut changes = Vec::new();
    compare_at(expected, actual, "$", &mut changes);
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
        Shape::Array { .. } => "array".to_string(),
        Shape::Unknown => "unknown".to_string(),
        Shape::Union { variants } => variants
            .iter()
            .map(shape_name)
            .collect::<Vec<_>>()
            .join(" | "),
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

fn compare_at(expected: &Shape, actual: &Shape, path: &str, changes: &mut Vec<ObservedChange>) {
    if matches!(expected, Shape::Unknown) {
        return;
    }

    if let Shape::Union { variants } = expected {
        if variants.iter().any(|variant| {
            let mut branch_changes = Vec::new();
            compare_at(variant, actual, path, &mut branch_changes);
            branch_changes.is_empty()
        }) {
            return;
        }
        incompatible(path, expected, actual, changes);
        return;
    }

    if let Shape::Union { variants } = actual {
        for variant in variants {
            compare_at(expected, variant, path, changes);
        }
        return;
    }

    match (expected, actual) {
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
                        changes,
                    ),
                    None if expected_property.observations == *observations => {
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
                compare_at(expected_items, actual_items, &format!("{path}[]"), changes);
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

fn shape_sort_key(shape: &Shape) -> u8 {
    match shape {
        Shape::Null => 0,
        Shape::Boolean => 1,
        Shape::Number => 2,
        Shape::String => 3,
        Shape::Object { .. } => 4,
        Shape::Array { .. } => 5,
        Shape::Union { .. } => 6,
        Shape::Unknown => 7,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{compare, infer, merge, shape_name, Shape};

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

        assert!(compare(
            &shape,
            &infer(&json!({
                "live_price": 3,
                "holdings": [{"ticker": "DIFFERENT"}]
            })),
        )
        .is_empty());
        assert!(compare(&shape, &infer(&json!({"holdings": []})))
            .iter()
            .any(|change| change.path == "$.live_price"));
        assert_eq!(shape_name(&shape), "object");
    }

    #[test]
    fn inferred_shapes_never_serialize_source_values() {
        let shape = infer(&json!({"token": "super-secret-token", "amount": 42}));
        let rendered = serde_yaml::to_string(&shape).expect("shape should serialize");

        assert!(!rendered.contains("super-secret-token"));
        assert!(!rendered.contains("42"));
        assert!(rendered.contains("token"));
        assert!(rendered.contains("string"));
    }

    #[test]
    fn empty_array_accepts_a_populated_array() {
        let expected = infer(&json!({"holdings": []}));
        let actual = infer(&json!({"holdings": [{"ticker": "APW"}]}));

        assert!(compare(&expected, &actual).is_empty());
    }

    #[test]
    fn reports_a_string_instead_of_a_locked_number() {
        let expected = infer(&json!({"live_price": 12}));
        let actual = infer(&json!({"live_price": "unavailable"}));
        let changes = compare(&expected, &actual);

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
}
