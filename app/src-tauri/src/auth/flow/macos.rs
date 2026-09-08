use crate::auth::error::AuthError;
use crate::auth::pkce;
use crate::auth::token_exchange::{self, FlowKind, TokenResponse};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPES: &str = "openid email https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/gmail.settings.basic";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

const HTML_RESPONSE: &str = "<!doctype html><html lang=\"uk\"><head><meta charset=\"utf-8\"><title>MyMail</title></head><body><h1>Готово, можете закрити це вікно.</h1></body></html>";

pub async fn run_login_flow(app: &AppHandle, client_id: &str) -> Result<TokenResponse, AuthError> {
    let pair = pkce::generate();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AuthError::Platform(format!("bind loopback: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AuthError::Platform(format!("local_addr: {e}")))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = random_state();
    let auth_url = build_auth_url(client_id, &redirect_uri, &pair.challenge, &state);

    app.opener()
        .open_url(&auth_url, None::<&str>)
        .map_err(|e| AuthError::Platform(format!("open browser: {e}")))?;

    let (code, returned_state) =
        tokio::time::timeout(CALLBACK_TIMEOUT, wait_for_callback(listener))
            .await
            .map_err(|_| AuthError::Cancelled)??;

    if returned_state != state {
        return Err(AuthError::OAuth("CSRF state mismatch".into()));
    }

    token_exchange::exchange_code(
        client_id,
        Some(crate::auth::config::desktop_client_secret()),
        &code,
        &pair.verifier,
        &redirect_uri,
        FlowKind::Desktop,
    )
    .await
}

fn random_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn build_auth_url(client_id: &str, redirect_uri: &str, challenge: &str, state: &str) -> String {
    let mut url = String::from(AUTH_ENDPOINT);
    url.push('?');
    push_query(&mut url, "client_id", client_id);
    url.push('&');
    push_query(&mut url, "redirect_uri", redirect_uri);
    url.push_str("&response_type=code");
    url.push('&');
    push_query(&mut url, "scope", SCOPES);
    url.push('&');
    push_query(&mut url, "code_challenge", challenge);
    url.push_str("&code_challenge_method=S256");
    url.push('&');
    push_query(&mut url, "state", state);
    url.push_str("&access_type=offline&prompt=consent");
    url
}

fn push_query(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

async fn wait_for_callback(listener: TcpListener) -> Result<(String, String), AuthError> {
    let (stream, _) = listener
        .accept()
        .await
        .map_err(|e| AuthError::Platform(format!("accept: {e}")))?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| AuthError::Platform(format!("read request: {e}")))?;

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        HTML_RESPONSE.len(),
        HTML_RESPONSE
    );
    let _ = write_half.write_all(response.as_bytes()).await;
    let _ = write_half.shutdown().await;

    parse_callback_query(&request_line)
}

fn parse_callback_query(request_line: &str) -> Result<(String, String), AuthError> {
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| AuthError::OAuth("malformed callback request".into()))?;
    let query = target
        .split_once('?')
        .map(|(_, q)| q)
        .ok_or_else(|| AuthError::OAuth("missing query in callback".into()))?;

    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = url_decode(v);
            match k {
                "code" => code = Some(decoded),
                "state" => state = Some(decoded),
                "error" => return Err(AuthError::OAuth(decoded)),
                _ => {}
            }
        }
    }

    Ok((
        code.ok_or_else(|| AuthError::OAuth("no code in callback".into()))?,
        state.ok_or_else(|| AuthError::OAuth("no state in callback".into()))?,
    ))
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_auth_url_contains_required_oauth_params() {
        let url = build_auth_url("CID", "http://127.0.0.1:7777/cb", "CHALLENGE", "STATE");
        assert!(url.contains("client_id=CID"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A7777%2Fcb"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge=CHALLENGE"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=STATE"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("scope=openid%20email%20https"));
        assert!(url.contains("gmail.settings.basic"));
    }

    #[test]
    fn parses_code_and_state_from_callback_request_line() {
        let line = "GET /callback?code=ABC123&state=XYZ HTTP/1.1\r\n";
        let (code, state) = parse_callback_query(line).unwrap();
        assert_eq!(code, "ABC123");
        assert_eq!(state, "XYZ");
    }

    #[test]
    fn returns_error_when_google_returned_error_param() {
        let line = "GET /callback?error=access_denied HTTP/1.1\r\n";
        let err = parse_callback_query(line).unwrap_err();
        assert!(matches!(err, AuthError::OAuth(_)));
    }

    #[test]
    fn returns_error_when_query_missing() {
        let line = "GET /callback HTTP/1.1\r\n";
        let err = parse_callback_query(line).unwrap_err();
        assert!(matches!(err, AuthError::OAuth(_)));
    }

    #[test]
    fn url_decodes_percent_and_plus() {
        assert_eq!(url_decode("hello%20world"), "hello world");
        assert_eq!(url_decode("a+b"), "a b");
        assert_eq!(url_decode("%21%40%23"), "!@#");
    }
}
