#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let value: serde_json::Value = match serde_json::from_slice(data) {
        Ok(v) => v,
        Err(_) => return,
    };

    let shape = apiwatch::observed::infer(&value);

    let max_depth = measure_depth(&shape);
    assert!(max_depth <= 128, "shape depth {} exceeds limit", max_depth);

    let serialized = serde_json::to_value(&shape).ok();
    if let Some(ref v) = serialized {
        let _: Result<apiwatch::observed::Shape, _> = serde_json::from_value(v.clone());
    }
});

fn measure_depth(shape: &apiwatch::observed::Shape) -> usize {
    match shape {
        apiwatch::observed::Shape::Object { properties, .. } => {
            1 + properties.values().map(|p| measure_depth(&p.shape)).max().unwrap_or(0)
        }
        apiwatch::observed::Shape::Array { items } => 1 + measure_depth(items),
        apiwatch::observed::Shape::Map { values } => 1 + measure_depth(values),
        apiwatch::observed::Shape::Union { variants } => {
            1 + variants.iter().map(|v| measure_depth(v)).max().unwrap_or(0)
        }
        _ => 1,
    }
}
