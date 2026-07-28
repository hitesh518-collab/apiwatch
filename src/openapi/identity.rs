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
    let value = value.split_once('#').map_or(value, |(value, _)| value);
    let (base, query) = value
        .split_once('?')
        .map_or((value, None), |(base, query)| (base, Some(query)));
    let (prepared, placeholders) = replace_placeholders(base)?;
    let network_relative = prepared.starts_with("//");
    let absolute = Url::parse(&prepared);
    let is_absolute = absolute.is_ok();
    let parsed = match absolute {
        Ok(url) => url,
        Err(_) if network_relative => {
            Url::parse(&format!("https:{prepared}")).map_err(|_| anyhow!("invalid server URL"))?
        }
        Err(_) => Url::parse("https://apiwatch.invalid/")
            .expect("constant URL should parse")
            .join(&prepared)
            .map_err(|_| anyhow!("invalid server URL"))?,
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(anyhow!("server URL contains credentials"));
    }

    let mut rendered = if is_absolute || network_relative {
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow!("invalid server URL"))?;
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        let port = parsed
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default();
        if network_relative {
            format!("//{host}{port}")
        } else {
            format!("{}://{host}{port}", parsed.scheme())
        }
    } else {
        String::new()
    };
    rendered.push_str(parsed.path());

    let mut rendered = restore_placeholders(&rendered, &placeholders);
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        rendered.push('?');
        rendered.push_str(&canonical_query(query));
    }
    Ok(crate::contract::ServerTemplate(rendered))
}

fn replace_placeholders(value: &str) -> Result<(String, Vec<(String, String)>)> {
    let mut prepared = String::new();
    let mut placeholders = Vec::new();
    let mut remainder = value;
    while let Some(start) = remainder.find('{') {
        let (prefix, after_start) = remainder.split_at(start);
        prepared.push_str(prefix);
        let Some(end) = after_start.find('}') else {
            prepared.push_str(after_start);
            return Ok((prepared, placeholders));
        };
        let placeholder = &after_start[..=end];
        let token = if is_port_placeholder(&prepared) {
            unique_port_token(value, placeholders.len())?
        } else {
            unique_text_token(value, placeholders.len())
        };
        prepared.push_str(&token);
        placeholders.push((token, placeholder.to_string()));
        remainder = &after_start[end + 1..];
    }
    prepared.push_str(remainder);
    Ok((prepared, placeholders))
}

fn is_port_placeholder(prefix: &str) -> bool {
    let authority = prefix
        .rsplit_once("://")
        .map(|(_, authority)| authority)
        .or_else(|| prefix.strip_prefix("//"));
    authority
        .is_some_and(|authority| authority.ends_with(':') && !authority.contains(['/', '?', '#']))
}

fn unique_text_token(value: &str, index: usize) -> String {
    let mut nonce = index;
    loop {
        let token = format!("apiwatchplaceholder{nonce}x");
        if !value.contains(&token) {
            return token;
        }
        nonce += 1;
    }
}

fn unique_port_token(value: &str, index: usize) -> Result<String> {
    for port in 60_000 + index..=65_535 {
        let token = port.to_string();
        if !value.contains(&token) {
            return Ok(token);
        }
    }
    Err(anyhow!("unable to safely normalize server template port"))
}

fn encode_query_key(key: &str) -> String {
    url::form_urlencoded::byte_serialize(key.as_bytes()).collect()
}

fn canonical_query(query: &str) -> String {
    let mut pairs = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = decode_query_key(key);
            (encode_query_key(&key), redact_query_value(value))
        })
        .collect::<Vec<_>>();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn decode_query_key(value: &str) -> String {
    let encoded = format!("{value}=");
    let (key, _) = url::form_urlencoded::parse(encoded.as_bytes())
        .next()
        .expect("constructed query pair should parse");
    key.into_owned()
}

fn redact_query_value(value: &str) -> String {
    let mut rendered = String::new();
    let mut remainder = value;
    while let Some(start) = remainder.find('{') {
        let (literal, after_start) = remainder.split_at(start);
        let Some(end) = after_start.find('}') else {
            break;
        };
        if !literal.is_empty() {
            rendered.push_str("{redacted}");
        }
        rendered.push_str(&after_start[..=end]);
        remainder = &after_start[end + 1..];
    }
    if !remainder.is_empty() || rendered.is_empty() {
        rendered.push_str("{redacted}");
    }
    rendered
}

fn restore_placeholders(value: &str, placeholders: &[(String, String)]) -> String {
    placeholders
        .iter()
        .fold(value.to_string(), |restored, (token, placeholder)| {
            restored.replace(token, placeholder)
        })
}
