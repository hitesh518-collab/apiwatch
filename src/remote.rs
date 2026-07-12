use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;

pub const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug)]
pub struct RemoteOpenApi {
    pub text: String,
    pub is_json: bool,
}

pub fn fetch(input: &str) -> Result<Option<RemoteOpenApi>> {
    let Some(url) = remote_url(input)? else {
        return Ok(None);
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(Policy::limited(5))
        .build()
        .context("failed to build remote OpenAPI client")?;
    let response = client
        .get(url)
        .send()
        .context("failed to request remote OpenAPI document")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "remote OpenAPI request returned a non-success status"
        ));
    }

    let is_json = response_is_json(&response);
    let text = read_limited_body(response)?;

    Ok(Some(RemoteOpenApi { text, is_json }))
}

fn remote_url(input: &str) -> Result<Option<reqwest::Url>> {
    let Some((scheme, remainder)) = input.split_once(':') else {
        return Ok(None);
    };

    if !remainder.starts_with("//") {
        return Ok(None);
    }

    if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        let url =
            reqwest::Url::parse(input).map_err(|error| anyhow!("invalid OpenAPI URL: {error}"))?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(anyhow!("remote OpenAPI URL credentials are not allowed"));
        }
        return Ok(Some(url));
    }

    Err(anyhow!("unsupported OpenAPI URL scheme"))
}

fn response_is_json(response: &Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(is_json_media_type)
        || response.url().path().ends_with(".json")
}

fn is_json_media_type(content_type: &str) -> bool {
    let media_type = content_type
        .split_once(';')
        .map_or(content_type, |(media_type, _)| media_type)
        .trim();
    let media_type = media_type.to_ascii_lowercase();

    media_type == "application/json" || media_type.ends_with("+json")
}

fn read_limited_body(reader: impl Read) -> Result<String> {
    let mut body = Vec::with_capacity(MAX_RESPONSE_BYTES + 1);
    let mut reader = reader.take((MAX_RESPONSE_BYTES + 1) as u64);
    reader
        .read_to_end(&mut body)
        .context("failed to read remote OpenAPI response")?;

    if body.len() > MAX_RESPONSE_BYTES {
        return Err(anyhow!("remote OpenAPI response exceeds 10 MiB"));
    }

    String::from_utf8(body).context("remote OpenAPI response is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use std::net::TcpListener;

    #[test]
    fn fetch_rejects_an_unsupported_url_scheme() {
        let error = fetch("ftp://example.test/openapi.yaml")
            .expect_err("unsupported scheme should be rejected");
        assert!(error.to_string().contains("unsupported OpenAPI URL scheme"));
    }

    #[test]
    fn read_body_rejects_more_than_ten_mebibytes() {
        let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let error = read_limited_body(std::io::Cursor::new(body))
            .expect_err("oversized body should be rejected");
        assert!(error
            .to_string()
            .contains("remote OpenAPI response exceeds 10 MiB"));
    }

    #[test]
    fn fetch_rejects_username_credentials_without_making_a_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        listener
            .set_nonblocking(true)
            .expect("listener should become nonblocking");
        let address = listener
            .local_addr()
            .expect("listener should have an address");

        let error = fetch(&format!("http://username@{address}/openapi.yaml"))
            .expect_err("username credentials should be rejected");

        assert_eq!(
            error.to_string(),
            "remote OpenAPI URL credentials are not allowed"
        );
        assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));
    }

    #[test]
    fn fetch_rejects_password_credentials_without_making_a_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        listener
            .set_nonblocking(true)
            .expect("listener should become nonblocking");
        let address = listener
            .local_addr()
            .expect("listener should have an address");

        let error = fetch(&format!("http://username:password@{address}/openapi.yaml"))
            .expect_err("password credentials should be rejected");

        assert_eq!(
            error.to_string(),
            "remote OpenAPI URL credentials are not allowed"
        );
        assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));
    }
}
