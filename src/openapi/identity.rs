use anyhow::anyhow;
use anyhow::{Context, Result};
use url::Url;

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

pub(crate) fn canonical_server_template(value: &str) -> Result<crate::contract::ServerTemplate> {
    let (prepared, placeholders) = replace_placeholders(value);
    let absolute = Url::parse(&prepared);
    let is_absolute = absolute.is_ok();
    let parsed = match absolute {
        Ok(url) => url,
        Err(_) => Url::parse("https://apiwatch.invalid/")
            .expect("constant URL should parse")
            .join(&prepared)
            .map_err(|_| anyhow!("invalid server URL"))?,
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(anyhow!("server URL contains credentials"));
    }

    let mut rendered = if is_absolute {
        let serialized = parsed.as_str();
        let path_start = serialized.find("://").and_then(|scheme| {
            serialized[scheme + 3..]
                .find('/')
                .map(|offset| scheme + 3 + offset)
                .or_else(|| {
                    serialized[scheme + 3..]
                        .find('?')
                        .map(|offset| scheme + 3 + offset)
                })
                .or_else(|| {
                    serialized[scheme + 3..]
                        .find('#')
                        .map(|offset| scheme + 3 + offset)
                })
        });
        match path_start {
            Some(index) => serialized[..index].to_string(),
            None => serialized.trim_end_matches(['?', '#']).to_string(),
        }
    } else {
        String::new()
    };
    rendered.push_str(parsed.path());

    let mut query = parsed.query_pairs().collect::<Vec<_>>();
    query.sort_by(|left, right| left.0.cmp(&right.0));
    if !query.is_empty() {
        rendered.push('?');
        rendered.push_str(
            &query
                .into_iter()
                .map(|(key, value)| {
                    let value = restore_placeholders(&value, &placeholders);
                    if value.contains('{') {
                        format!("{key}={value}")
                    } else {
                        format!("{key}={{redacted}}")
                    }
                })
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    Ok(crate::contract::ServerTemplate(restore_placeholders(
        &rendered,
        &placeholders,
    )))
}

fn replace_placeholders(value: &str) -> (String, Vec<(String, String)>) {
    let mut prepared = String::new();
    let mut placeholders = Vec::new();
    let mut remainder = value;
    while let Some(start) = remainder.find('{') {
        let (prefix, after_start) = remainder.split_at(start);
        prepared.push_str(prefix);
        let Some(end) = after_start.find('}') else {
            prepared.push_str(after_start);
            return (prepared, placeholders);
        };
        let placeholder = &after_start[..=end];
        let token = format!("apiwatchplaceholder{}", placeholders.len());
        prepared.push_str(&token);
        placeholders.push((token, placeholder.to_string()));
        remainder = &after_start[end + 1..];
    }
    prepared.push_str(remainder);
    (prepared, placeholders)
}

fn restore_placeholders(value: &str, placeholders: &[(String, String)]) -> String {
    placeholders
        .iter()
        .fold(value.to_string(), |restored, (token, placeholder)| {
            restored.replace(token, placeholder)
        })
}
