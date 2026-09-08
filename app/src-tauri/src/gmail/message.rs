use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;

/// base64url decoder that tolerates optional `=` padding. Gmail returns
/// `body.data` as standard (padded) base64url, not the no-pad variant.
const BASE64URL: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct GmailMessage {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub body: String,
    pub html_body: Option<String>,
    pub unsubscribe: Option<UnsubscribeAction>,
    pub attachments: Vec<Attachment>,
}

/// A real file attachment — excludes inline images (e.g. signature logos)
/// that are embedded in the HTML body via `cid:` references.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub attachment_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

/// RFC 2369 / 8058 `List-Unsubscribe` action. JS sees this tagged as
/// `{ kind: 'OneClick' | 'Url' | 'Mailto', ... }`.
#[derive(Debug, Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum UnsubscribeAction {
    /// RFC 8058 one-click: HTTPS URL plus
    /// `List-Unsubscribe-Post: List-Unsubscribe=One-Click` header. The client
    /// POSTs `List-Unsubscribe=One-Click` with no user interaction.
    OneClick { url: String },
    /// Plain HTTP/HTTPS unsubscribe page — open in the user's browser.
    Url { url: String },
    /// Send an empty mail to `to` with optional `subject`.
    Mailto { to: String, subject: Option<String> },
}

pub fn parse_unsubscribe(headers: &[Value]) -> Option<UnsubscribeAction> {
    let raw = extract_header(headers, "List-Unsubscribe");
    if raw.is_empty() {
        return None;
    }

    let mut https_url: Option<String> = None;
    let mut http_url: Option<String> = None;
    let mut mailto: Option<(String, Option<String>)> = None;

    for chunk in raw.split(',') {
        let trimmed = chunk.trim();
        let Some(inside) = trimmed.strip_prefix('<').and_then(|s| s.strip_suffix('>')) else {
            continue;
        };
        let inside = inside.trim();
        if let Some(rest) = inside.strip_prefix("mailto:") {
            if mailto.is_none() {
                mailto = Some(parse_mailto(rest));
            }
        } else if inside.starts_with("https://") {
            if https_url.is_none() {
                https_url = Some(inside.to_string());
            }
        } else if inside.starts_with("http://") && http_url.is_none() {
            http_url = Some(inside.to_string());
        }
    }

    let one_click = extract_header(headers, "List-Unsubscribe-Post")
        .eq_ignore_ascii_case("List-Unsubscribe=One-Click");

    if let Some(url) = https_url {
        if one_click {
            return Some(UnsubscribeAction::OneClick { url });
        }
        return Some(UnsubscribeAction::Url { url });
    }
    if let Some(url) = http_url {
        return Some(UnsubscribeAction::Url { url });
    }
    mailto.map(|(to, subject)| UnsubscribeAction::Mailto { to, subject })
}

fn parse_mailto(rest: &str) -> (String, Option<String>) {
    let (to, query) = match rest.split_once('?') {
        Some((t, q)) => (t.trim(), Some(q)),
        None => (rest.trim(), None),
    };
    let subject = query.and_then(|q| {
        q.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            k.eq_ignore_ascii_case("subject").then(|| v.to_string())
        })
    });
    (to.to_string(), subject)
}

pub fn extract_header(headers: &[Value], name: &str) -> String {
    let target = name.to_ascii_lowercase();
    for h in headers {
        let key = h.get("name").and_then(Value::as_str).unwrap_or("");
        if key.to_ascii_lowercase() == target {
            return h
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
    }
    String::new()
}

pub fn extract_html_body(payload: &Value) -> Option<String> {
    find_part(payload, "text/html")
}

/// Collects real file attachments, excluding inline images (e.g. signature
/// logos) that are embedded in the HTML body via a `cid:` reference or
/// marked `Content-Disposition: inline`.
pub fn extract_attachments(payload: &Value, html_body: Option<&str>) -> Vec<Attachment> {
    let mut out = Vec::new();
    collect_attachments(payload, html_body, &mut out);
    out
}

fn collect_attachments(node: &Value, html_body: Option<&str>, out: &mut Vec<Attachment>) {
    let filename = node.get("filename").and_then(Value::as_str).unwrap_or("");
    let attachment_id = node
        .get("body")
        .and_then(|b| b.get("attachmentId"))
        .and_then(Value::as_str);

    if let (false, Some(attachment_id)) = (filename.is_empty(), attachment_id) {
        let empty_headers: Vec<Value> = Vec::new();
        let headers = node
            .get("headers")
            .and_then(Value::as_array)
            .unwrap_or(&empty_headers);
        let disposition = extract_header(headers, "Content-Disposition").to_ascii_lowercase();
        let content_id = extract_header(headers, "Content-ID");
        let cid = content_id
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');
        let referenced_in_body =
            !cid.is_empty() && html_body.is_some_and(|html| html.contains(&format!("cid:{cid}")));
        let is_inline = disposition.starts_with("inline") || referenced_in_body;

        if !is_inline {
            let size = node
                .get("body")
                .and_then(|b| b.get("size"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let mime_type = node
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            out.push(Attachment {
                attachment_id: attachment_id.to_string(),
                filename: filename.to_string(),
                mime_type,
                size,
            });
        }
    }

    if let Some(parts) = node.get("parts").and_then(Value::as_array) {
        for p in parts {
            collect_attachments(p, html_body, out);
        }
    }
}

/// Returns meaningful plain text, preferring `text/plain` and falling back to HTML.
pub fn extract_plain_text(payload: &Value) -> String {
    if let Some(text) = find_part(payload, "text/plain") {
        if !text.trim().is_empty() {
            return text;
        }
    }
    if let Some(html) = find_part(payload, "text/html") {
        return strip_html(&html);
    }
    String::new()
}

fn find_part(node: &Value, target_mime: &str) -> Option<String> {
    let mime = node.get("mimeType").and_then(Value::as_str).unwrap_or("");
    if mime == target_mime {
        if let Some(data) = node
            .get("body")
            .and_then(|b| b.get("data"))
            .and_then(Value::as_str)
        {
            return decode_part(data, &part_charset(node));
        }
        return None;
    }
    if let Some(parts) = node.get("parts").and_then(Value::as_array) {
        for p in parts {
            if let Some(found) = find_part(p, target_mime) {
                return Some(found);
            }
        }
    }
    None
}

/// Extracts the `charset` parameter from a part's `Content-Type` header.
/// Returns an empty string when absent — callers fall back to UTF-8.
fn part_charset(node: &Value) -> String {
    let Some(headers) = node.get("headers").and_then(Value::as_array) else {
        return String::new();
    };
    let content_type = extract_header(headers, "Content-Type").to_ascii_lowercase();
    for param in content_type.split(';') {
        if let Some(value) = param.trim().strip_prefix("charset=") {
            return value.trim().trim_matches('"').to_string();
        }
    }
    String::new()
}

/// Decodes Gmail `body.data` (base64url) into raw bytes, e.g. for an
/// attachment payload fetched via the `attachments.get` endpoint.
pub fn decode_base64url_bytes(data: &str) -> Option<Vec<u8>> {
    BASE64URL.decode(data.as_bytes()).ok()
}

/// Decodes Gmail `body.data` (base64url) and converts the bytes to a `String`
/// using the part's declared charset, falling back to UTF-8 for an absent or
/// unknown charset. Decoding is lossy, so non-empty data always yields text.
///
/// Some senders mislabel a UTF-8 body with a legacy single-byte charset (e.g.
/// `windows-1251`) in the `Content-Type` header. A legacy charset's non-ASCII
/// bytes essentially never happen to form valid UTF-8 by chance, so if the
/// declared charset isn't already UTF-8 but the raw bytes validate as UTF-8
/// anyway, trust the bytes over the mislabeled header to avoid mojibake.
fn decode_part(data: &str, charset: &str) -> Option<String> {
    let bytes = BASE64URL.decode(data.as_bytes()).ok()?;
    let label = if !charset.eq_ignore_ascii_case("utf-8")
        && !charset.eq_ignore_ascii_case("utf8")
        && std::str::from_utf8(&bytes).is_ok()
    {
        "utf-8"
    } else {
        charset
    };
    let encoding = encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = encoding.decode(&bytes);
    Some(text.into_owned())
}

fn strip_html(html: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").expect("static regex");
    let stripped = re.replace_all(html, "");
    let decoded = html_escape::decode_html_entities(&stripped);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_header_finds_case_insensitive() {
        let headers = vec![
            json!({"name": "From", "value": "alice@example.com"}),
            json!({"name": "Subject", "value": "Hi"}),
        ];
        assert_eq!(extract_header(&headers, "from"), "alice@example.com");
        assert_eq!(extract_header(&headers, "SUBJECT"), "Hi");
    }

    #[test]
    fn extract_header_returns_empty_when_missing() {
        let headers = vec![json!({"name": "From", "value": "a@b"})];
        assert_eq!(extract_header(&headers, "Date"), "");
    }

    #[test]
    fn extract_header_returns_empty_for_empty_list() {
        assert_eq!(extract_header(&[], "From"), "");
    }

    fn b64url(s: &str) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s.as_bytes())
    }

    #[test]
    fn extract_plain_text_from_plain_payload() {
        let payload = json!({
            "mimeType": "text/plain",
            "body": { "data": b64url("hello world") }
        });
        assert_eq!(extract_plain_text(&payload), "hello world");
    }

    #[test]
    fn extract_plain_text_from_html_payload_strips_tags() {
        let html = "<p>Hello <b>world</b>!</p>";
        let payload = json!({
            "mimeType": "text/html",
            "body": { "data": b64url(html) }
        });
        assert_eq!(extract_plain_text(&payload), "Hello world!");
    }

    #[test]
    fn extract_plain_text_decodes_html_entities() {
        let html = "Hello &amp; bye &lt;3";
        let payload = json!({
            "mimeType": "text/html",
            "body": { "data": b64url(html) }
        });
        assert_eq!(extract_plain_text(&payload), "Hello & bye <3");
    }

    #[test]
    fn extract_plain_text_prefers_plain_in_alternative() {
        let payload = json!({
            "mimeType": "multipart/alternative",
            "parts": [
                {"mimeType": "text/plain", "body": {"data": b64url("plain version")}},
                {"mimeType": "text/html", "body": {"data": b64url("<b>html version</b>")}}
            ]
        });
        assert_eq!(extract_plain_text(&payload), "plain version");
    }

    #[test]
    fn extract_plain_text_falls_back_to_html_when_no_plain() {
        let payload = json!({
            "mimeType": "multipart/alternative",
            "parts": [
                {"mimeType": "text/html", "body": {"data": b64url("<p>only html</p>")}}
            ]
        });
        assert_eq!(extract_plain_text(&payload), "only html");
    }

    #[test]
    fn extract_plain_text_falls_back_to_html_when_plain_is_empty() {
        let payload = json!({
            "mimeType": "multipart/alternative",
            "parts": [
                {"mimeType": "text/plain", "body": {"data": b64url("  \n")}},
                {"mimeType": "text/html", "body": {"data": b64url("<p>calendar event</p>")}}
            ]
        });
        assert_eq!(extract_plain_text(&payload), "calendar event");
    }

    #[test]
    fn extract_plain_text_recurses_into_nested_multipart() {
        let payload = json!({
            "mimeType": "multipart/mixed",
            "parts": [
                {
                    "mimeType": "multipart/alternative",
                    "parts": [
                        {"mimeType": "text/plain", "body": {"data": b64url("nested plain")}}
                    ]
                }
            ]
        });
        assert_eq!(extract_plain_text(&payload), "nested plain");
    }

    #[test]
    fn extract_plain_text_returns_empty_when_no_text_part() {
        let payload = json!({
            "mimeType": "multipart/mixed",
            "parts": [
                {"mimeType": "image/png", "body": {"data": b64url("BINARY")}}
            ]
        });
        assert_eq!(extract_plain_text(&payload), "");
    }

    #[test]
    fn extract_plain_text_returns_empty_for_missing_data() {
        let payload = json!({"mimeType": "text/plain"});
        assert_eq!(extract_plain_text(&payload), "");
    }

    #[test]
    fn extract_plain_text_decodes_padded_base64url() {
        // Gmail returns body.data as standard base64url WITH `=` padding;
        // "aGVsbG8=" is "hello" (5 bytes → one padding char).
        let payload = json!({
            "mimeType": "text/plain",
            "body": { "data": "aGVsbG8=" }
        });
        assert_eq!(extract_plain_text(&payload), "hello");
    }

    #[test]
    fn extract_plain_text_decodes_iso_8859_1_body() {
        // "café" in ISO-8859-1 — é is byte 0xE9, which is invalid as UTF-8.
        let data = base64::engine::general_purpose::URL_SAFE.encode([b'c', b'a', b'f', 0xE9_u8]);
        let payload = json!({
            "mimeType": "text/plain",
            "headers": [{"name": "Content-Type", "value": "text/plain; charset=ISO-8859-1"}],
            "body": { "data": data }
        });
        assert_eq!(extract_plain_text(&payload), "café");
    }

    #[test]
    fn extract_plain_text_decodes_windows_1251_body() {
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode("Привіт");
        let data = base64::engine::general_purpose::URL_SAFE.encode(bytes.as_ref());
        let payload = json!({
            "mimeType": "text/plain",
            "headers": [{"name": "Content-Type", "value": "text/plain; charset=windows-1251"}],
            "body": { "data": data }
        });
        assert_eq!(extract_plain_text(&payload), "Привіт");
    }

    #[test]
    fn extract_plain_text_prefers_valid_utf8_over_mislabeled_charset() {
        // Sender declares windows-1251 but the body bytes are actually UTF-8 —
        // a real-world mislabeling that used to produce mojibake.
        let data = base64::engine::general_purpose::URL_SAFE.encode("Здравствуйте".as_bytes());
        let payload = json!({
            "mimeType": "text/plain",
            "headers": [{"name": "Content-Type", "value": "text/plain; charset=windows-1251"}],
            "body": { "data": data }
        });
        assert_eq!(extract_plain_text(&payload), "Здравствуйте");
    }

    #[test]
    fn parse_unsubscribe_returns_none_when_header_missing() {
        let headers = vec![json!({"name": "From", "value": "x@y"})];
        assert_eq!(parse_unsubscribe(&headers), None);
    }

    #[test]
    fn parse_unsubscribe_parses_https_url() {
        let headers =
            vec![json!({"name": "List-Unsubscribe", "value": "<https://example.com/unsub?id=1>"})];
        assert_eq!(
            parse_unsubscribe(&headers),
            Some(UnsubscribeAction::Url {
                url: "https://example.com/unsub?id=1".into(),
            })
        );
    }

    #[test]
    fn parse_unsubscribe_parses_mailto() {
        let headers =
            vec![json!({"name": "List-Unsubscribe", "value": "<mailto:unsub@example.com>"})];
        assert_eq!(
            parse_unsubscribe(&headers),
            Some(UnsubscribeAction::Mailto {
                to: "unsub@example.com".into(),
                subject: None,
            })
        );
    }

    #[test]
    fn parse_unsubscribe_parses_mailto_with_subject() {
        let headers = vec![json!({
            "name": "List-Unsubscribe",
            "value": "<mailto:unsub@example.com?subject=unsubscribe>"
        })];
        assert_eq!(
            parse_unsubscribe(&headers),
            Some(UnsubscribeAction::Mailto {
                to: "unsub@example.com".into(),
                subject: Some("unsubscribe".into()),
            })
        );
    }

    #[test]
    fn parse_unsubscribe_prefers_https_over_mailto_when_no_one_click() {
        let headers = vec![json!({
            "name": "List-Unsubscribe",
            "value": "<mailto:u@x.com>, <https://x.com/u>"
        })];
        assert_eq!(
            parse_unsubscribe(&headers),
            Some(UnsubscribeAction::Url {
                url: "https://x.com/u".into(),
            })
        );
    }

    #[test]
    fn parse_unsubscribe_upgrades_to_one_click_when_post_header_present() {
        let headers = vec![
            json!({
                "name": "List-Unsubscribe",
                "value": "<mailto:u@x.com>, <https://x.com/u>"
            }),
            json!({
                "name": "List-Unsubscribe-Post",
                "value": "List-Unsubscribe=One-Click"
            }),
        ];
        assert_eq!(
            parse_unsubscribe(&headers),
            Some(UnsubscribeAction::OneClick {
                url: "https://x.com/u".into(),
            })
        );
    }

    #[test]
    fn parse_unsubscribe_ignores_one_click_for_http_url() {
        // RFC 8058 §3.1: one-click requires HTTPS — fall back to plain URL.
        let headers = vec![
            json!({"name": "List-Unsubscribe", "value": "<http://x.com/u>"}),
            json!({
                "name": "List-Unsubscribe-Post",
                "value": "List-Unsubscribe=One-Click"
            }),
        ];
        assert_eq!(
            parse_unsubscribe(&headers),
            Some(UnsubscribeAction::Url {
                url: "http://x.com/u".into(),
            })
        );
    }

    #[test]
    fn parse_unsubscribe_returns_none_when_no_angle_brackets() {
        let headers = vec![json!({"name": "List-Unsubscribe", "value": "https://x.com/u"})];
        assert_eq!(parse_unsubscribe(&headers), None);
    }

    #[test]
    fn extract_attachments_returns_regular_attachment() {
        let payload = json!({
            "mimeType": "multipart/mixed",
            "parts": [
                {"mimeType": "text/plain", "body": {"data": b64url("hi")}},
                {
                    "mimeType": "application/pdf",
                    "filename": "invoice.pdf",
                    "body": {"attachmentId": "abc123", "size": 4096}
                }
            ]
        });
        let attachments = extract_attachments(&payload, None);
        assert_eq!(
            attachments,
            vec![Attachment {
                attachment_id: "abc123".into(),
                filename: "invoice.pdf".into(),
                mime_type: "application/pdf".into(),
                size: 4096,
            }]
        );
    }

    #[test]
    fn extract_attachments_excludes_inline_content_disposition() {
        let payload = json!({
            "mimeType": "multipart/related",
            "parts": [
                {
                    "mimeType": "image/png",
                    "filename": "logo.png",
                    "headers": [{"name": "Content-Disposition", "value": "inline; filename=\"logo.png\""}],
                    "body": {"attachmentId": "sig1", "size": 512}
                }
            ]
        });
        assert_eq!(extract_attachments(&payload, None), Vec::new());
    }

    #[test]
    fn extract_attachments_excludes_images_referenced_by_cid_in_html_body() {
        let payload = json!({
            "mimeType": "multipart/related",
            "parts": [
                {
                    "mimeType": "image/png",
                    "filename": "image001.png",
                    "headers": [{"name": "Content-ID", "value": "<sig-logo@example.com>"}],
                    "body": {"attachmentId": "sig2", "size": 1024}
                }
            ]
        });
        let html = "<p>Regards</p><img src=\"cid:sig-logo@example.com\">";
        assert_eq!(extract_attachments(&payload, Some(html)), Vec::new());
    }

    #[test]
    fn extract_attachments_keeps_named_file_not_referenced_in_body() {
        let payload = json!({
            "mimeType": "multipart/mixed",
            "parts": [
                {
                    "mimeType": "image/png",
                    "filename": "screenshot.png",
                    "headers": [{"name": "Content-ID", "value": "<not-used@example.com>"}],
                    "body": {"attachmentId": "att1", "size": 2048}
                }
            ]
        });
        let html = "<p>See attached</p>";
        let attachments = extract_attachments(&payload, Some(html));
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].filename, "screenshot.png");
    }

    #[test]
    fn decode_base64url_bytes_decodes_padded_data() {
        assert_eq!(decode_base64url_bytes("aGVsbG8="), Some(b"hello".to_vec()));
    }

    #[test]
    fn decode_base64url_bytes_returns_none_for_invalid_input() {
        assert_eq!(decode_base64url_bytes("not base64!!"), None);
    }

    #[test]
    fn extract_attachments_ignores_parts_without_attachment_id() {
        let payload = json!({
            "mimeType": "multipart/alternative",
            "parts": [
                {"mimeType": "text/plain", "body": {"data": b64url("hi")}},
                {"mimeType": "text/html", "body": {"data": b64url("<p>hi</p>")}}
            ]
        });
        assert_eq!(extract_attachments(&payload, None), Vec::new());
    }
}
