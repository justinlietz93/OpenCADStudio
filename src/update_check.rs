// One-shot update check.
//
// `check_for_update()` runs on a background thread (joined inside an
// async wrapper, mirroring how the rest of `crate::io` wraps blocking
// work for iced's `Task::perform`). It hits the GitHub releases API and
// returns `Some(UpdateInfo)` when a newer release is available, or
// `None` when up to date / on network failure / on parse error.

const RELEASES_API: &str =
    "https://api.github.com/repos/HakanSeven12/OpenCADStudio/releases/latest";
pub const RELEASES_PAGE: &str =
    "https://github.com/HakanSeven12/OpenCADStudio/releases/latest";

/// What `check_for_update` reports when a newer release exists.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// `tag_name` with the leading `v` stripped (e.g. `0.3.7`).
    pub version: String,
    /// Release notes / markdown body from the GitHub release. May be empty
    /// when the release was published without notes.
    pub body: String,
}

pub async fn check_for_update() -> Option<UpdateInfo> {
    std::thread::spawn(fetch_latest_if_outdated)
        .join()
        .ok()
        .flatten()
}

fn fetch_latest_if_outdated() -> Option<UpdateInfo> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build()
        .into();
    let body = agent
        .get(RELEASES_API)
        .header("User-Agent", concat!("OpenCADStudio/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    let latest = extract_string_field(&body, "tag_name")?
        .trim_start_matches('v')
        .to_string();
    if latest == env!("CARGO_PKG_VERSION") {
        return None;
    }
    // Release notes are optional; treat missing as empty.
    let notes = extract_string_field(&body, "body").unwrap_or_default();
    Some(UpdateInfo { version: latest, body: notes })
}

/// Minimal extractor for a top-level string field in the releases JSON.
/// Avoids pulling in `serde_json` for two fields. Handles standard JSON
/// string escapes (`\"`, `\\`, `\n`, `\r`, `\t`, `\/`) which are all the
/// GitHub release body uses in practice.
fn extract_string_field(body: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\":\"", field);
    let start = body.find(&key)? + key.len();
    // Walk to the closing unescaped `"`, JSON-unescaping as we go.
    let mut out = String::new();
    let bytes = body.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            return Some(out);
        }
        if b == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    // \uXXXX — decode 4 hex digits. Surrogate pairs (BMP only
                    // for now) are uncommon in release notes; skip on parse
                    // failure.
                    if i + 5 < bytes.len() {
                        let hex = std::str::from_utf8(&bytes[i + 2..i + 6]).ok()?;
                        if let Ok(code) = u32::from_str_radix(hex, 16) {
                            if let Some(c) = char::from_u32(code) {
                                out.push(c);
                            }
                        }
                        i += 6;
                        continue;
                    }
                    return None;
                }
                other => {
                    // Unknown escape — keep the literal pair, GitHub doesn't emit these.
                    out.push('\\');
                    out.push(other as char);
                }
            }
            i += 2;
            continue;
        }
        // UTF-8 multi-byte chars: push the whole code-point so we don't
        // bisect a sequence.
        let ch = body[i..].chars().next()?;
        out.push(ch);
        i += ch.len_utf8();
    }
    None
}
