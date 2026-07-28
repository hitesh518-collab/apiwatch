use anyhow::{Context, Result};

pub(crate) fn canonical_media_type(value: &str) -> Result<String> {
    let parsed: mime::Mime = value.parse().context("invalid media type")?;
    let mut parameters = parsed
        .params()
        .map(|(name, value)| {
            (
                name.as_str().to_ascii_lowercase(),
                value.as_str().to_string(),
            )
        })
        .collect::<Vec<_>>();
    parameters.sort();
    let mut subtype = parsed.subtype().as_str().to_ascii_lowercase();
    if let Some(suffix) = parsed.suffix() {
        subtype.push('+');
        subtype.push_str(&suffix.as_str().to_ascii_lowercase());
    }
    let mut canonical = format!(
        "{}/{}",
        parsed.type_().as_str().to_ascii_lowercase(),
        subtype
    );
    for (name, value) in parameters {
        canonical.push_str(&format!(";{name}={value}"));
    }
    Ok(canonical)
}
