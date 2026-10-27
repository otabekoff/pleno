//! Cookies: parsing the `Cookie` request header and writing `Set-Cookie`
//! (RFC 6265).

use indexmap::IndexMap;
use pleno_value::Value;

/// Whether `byte` may appear in an HTTP token, such as a header or cookie name.
pub(crate) fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

pub(crate) fn is_token(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(is_token_byte)
}

/// Whether `byte` may appear unencoded in a cookie value (RFC 6265
/// `cookie-octet`). `%` is excluded too, since it starts an escape.
fn is_cookie_octet(byte: u8) -> bool {
    matches!(byte, 0x21 | 0x23..=0x2B | 0x2D..=0x3A | 0x3C..=0x5B | 0x5D..=0x7E) && byte != b'%'
}

/// Decodes `%XX` escapes. An escape that is not two hex digits is kept as
/// written; `None` if the result is not UTF-8.
pub(crate) fn percent_decode(text: &str, plus_as_space: bool) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = |at: usize| bytes.get(at).and_then(|b| (*b as char).to_digit(16));
                match (hex(i + 1), hex(i + 2)) {
                    (Some(high), Some(low)) => {
                        out.push((high * 16 + low) as u8);
                        i += 3;
                        continue;
                    }
                    _ => out.push(b'%'),
                }
            }
            b'+' if plus_as_space => out.push(b' '),
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// The cookies of a `Cookie` header, by name. When a name appears twice the
/// first one wins: browsers send the cookie with the most specific path first.
pub(crate) fn parse(header: &str) -> IndexMap<String, String> {
    let mut cookies = IndexMap::new();
    for pair in header.split(';') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || cookies.contains_key(name) {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        let value = percent_decode(value, false).unwrap_or_else(|| value.to_string());
        cookies.insert(name.to_string(), value);
    }
    cookies
}

fn encode_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if is_cookie_octet(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The attribute values `res.cookie` accepts for `path` and `domain`: no
/// control characters, spaces or `;`, which would end or split the header.
fn check_attribute(option: &str, value: &str) -> Result<(), String> {
    if value
        .chars()
        .any(|c| c.is_control() || c == ';' || c == ' ' || c == ',')
    {
        return Err(format!(
            "res.cookie(): the {option} option cannot contain spaces, ',', ';', CR, LF or other control characters"
        ));
    }
    Ok(())
}

/// What `res.cookie` writes: the `Set-Cookie` value and the key that a later
/// cookie with the same name, path and domain replaces.
pub(crate) struct SetCookie {
    pub key: String,
    pub header: String,
}

/// Builds a `Set-Cookie` value. `HttpOnly` and `SameSite=Lax` are on unless
/// the options turn them off. `clear` is for `clearCookie`: an expired cookie.
pub(crate) fn serialize(
    function: &str,
    name: &Value,
    value: &str,
    options: Option<&Value>,
    clear: bool,
) -> Result<SetCookie, String> {
    let Value::String(name) = name else {
        return Err(format!("res.{function}() expects a cookie name (String)"));
    };
    if name.contains(['\r', '\n']) || value.contains(['\r', '\n', '\0']) {
        return Err(format!(
            "res.{function}(): cookie names and values cannot contain CR, LF or NUL"
        ));
    }
    if !is_token(name) {
        return Err(format!(
            "res.{function}(): invalid cookie name '{name}': use letters, digits and !#$%&'*+-.^_`|~"
        ));
    }

    let mut max_age: Option<i64> = None;
    let mut path = "/".to_string();
    let mut domain: Option<String> = None;
    let mut http_only = true;
    let mut secure = false;
    let mut same_site: Option<&'static str> = Some("Lax");
    match options {
        None | Some(Value::Null) => {}
        Some(Value::Map(options)) => {
            for (key, option) in options.borrow().entries.iter() {
                let wrong = |expected: &str| {
                    format!("res.{function}(): the {key} option must be {expected}")
                };
                match (key.as_str(), option) {
                    ("maxAge", Value::Int(seconds)) => max_age = Some(*seconds),
                    ("maxAge", Value::Float(seconds)) if seconds.is_finite() => {
                        max_age = Some(seconds.floor() as i64)
                    }
                    ("maxAge", _) => return Err(wrong("a number of seconds")),
                    ("path", Value::String(p)) => {
                        check_attribute("path", p)?;
                        path = p.to_string();
                    }
                    ("path", _) => return Err(wrong("a String")),
                    ("domain", Value::String(d)) => {
                        check_attribute("domain", d)?;
                        domain = Some(d.to_string());
                    }
                    ("domain", _) => return Err(wrong("a String")),
                    ("httpOnly", Value::Bool(b)) => http_only = *b,
                    ("httpOnly", _) => return Err(wrong("true or false")),
                    ("secure", Value::Bool(b)) => secure = *b,
                    ("secure", _) => return Err(wrong("true or false")),
                    ("sameSite", Value::Bool(false)) => same_site = None,
                    ("sameSite", Value::String(s)) => {
                        same_site = Some(match s.to_ascii_lowercase().as_str() {
                            "strict" => "Strict",
                            "lax" => "Lax",
                            "none" => "None",
                            _ => return Err(wrong("\"Strict\", \"Lax\", \"None\" or false")),
                        })
                    }
                    ("sameSite", _) => return Err(wrong("\"Strict\", \"Lax\", \"None\" or false")),
                    _ => {
                        return Err(format!(
                            "res.{function}(): unknown option '{key}'; the options are maxAge, path, domain, httpOnly, secure and sameSite"
                        ));
                    }
                }
            }
        }
        Some(_) => return Err(format!("res.{function}(): options must be a map")),
    }
    if same_site == Some("None") && !secure {
        return Err(format!(
            "res.{function}(): sameSite \"None\" needs secure: true; browsers reject the cookie otherwise"
        ));
    }

    let mut header = format!("{name}={}", encode_value(value));
    if clear {
        header.push_str("; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT");
    } else if let Some(seconds) = max_age {
        header.push_str(&format!("; Max-Age={}", seconds.max(0)));
    }
    if let Some(domain) = &domain {
        header.push_str(&format!("; Domain={domain}"));
    }
    header.push_str(&format!("; Path={path}"));
    if http_only {
        header.push_str("; HttpOnly");
    }
    if secure {
        header.push_str("; Secure");
    }
    if let Some(same_site) = same_site {
        header.push_str(&format!("; SameSite={same_site}"));
    }
    Ok(SetCookie {
        key: format!("{name};{path};{}", domain.unwrap_or_default()),
        header,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cookie_headers() {
        let cookies = parse("a=1; b=\"two words\"; c=x%3Dy; a=ignored; broken; =empty");
        assert_eq!(cookies.get("a").map(String::as_str), Some("1"));
        assert_eq!(cookies.get("b").map(String::as_str), Some("two words"));
        assert_eq!(cookies.get("c").map(String::as_str), Some("x=y"));
        assert_eq!(cookies.len(), 3);
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("a%20b+c", true).as_deref(), Some("a b c"));
        assert_eq!(percent_decode("100%", false).as_deref(), Some("100%"));
        assert_eq!(percent_decode("%zz%4", false).as_deref(), Some("%zz%4"));
        assert_eq!(percent_decode("%2e%2E", false).as_deref(), Some(".."));
        assert_eq!(percent_decode("%ff", false), None);
    }

    #[test]
    fn encodes_values_that_need_it() {
        let name = Value::String(std::rc::Rc::new("session".to_string()));
        let cookie = serialize("cookie", &name, "a b;c\"%", None, false).unwrap();
        assert_eq!(
            cookie.header,
            "session=a%20b%3Bc%22%25; Path=/; HttpOnly; SameSite=Lax"
        );
        assert!(serialize("cookie", &name, "x\r\nSet-Cookie: evil=1", None, false).is_err());
        let bad = Value::String(std::rc::Rc::new("a b".to_string()));
        assert!(serialize("cookie", &bad, "x", None, false).is_err());
    }
}
