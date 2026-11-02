/** Downloads: fetch through the governed network path, then save to the folder under a unique, inert name. */
use super::{authority::Permit, supervisor::Runtime};
use crate::{
    constants::WEB_TIMEOUT,
    network::WebRequest,
    protocol::{EngineError, EngineResult},
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

/// The newest downloads the toolbar list keeps; older rows drop off (the files stay on disk).
pub const DOWNLOADS_KEPT: usize = 50;
/// The highest " (n)" tried for a free file name before giving up, as Oya's NAME_ATTEMPTS.
pub const NAME_ATTEMPTS: usize = 1000;
/// File names are capped so a page cannot hand macOS an over-long name.
const NAME_CHARS: usize = 120;
/// Written files are readable and never executable.
const FILE_MODE: u32 = 0o644;

/** One file fetched for saving: where it came from, what it is, and its bytes. */
pub struct Fetched {
    /// The final address after redirects.
    pub url: String,
    /// The response media type, without parameters.
    pub mime: String,
    /// The body, already bounded by the network service.
    pub bytes: Vec<u8>,
    /// The server's Content-Disposition, which names the file when it carries `filename=`.
    pub disposition: Option<String>,
}

impl Fetched {
    /** The bytes of a response the network service marked as a download. */
    pub(super) fn from_download(response: crate::network::WebResponse) -> EngineResult<Self> {
        Ok(Self {
            bytes: super::images::decode_base64_within(
                &response.source,
                crate::constants::HTML_BYTES,
            )?,
            url: response.url,
            mime: response.mime,
            disposition: response.disposition,
        })
    }
}

/** Where a download stands, as the toolbar list shows it. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Queued or fetching.
    Progress,
    /// Saved at this path.
    Done(PathBuf),
    /// Not saved, with a short reason.
    Failed(String),
}

/** One row of the downloads list. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    /// Unique within this session, so a finished fetch finds its row.
    pub id: u64,
    /// The address the person asked for.
    pub url: String,
    /// The file name shown (the final one once saved).
    pub name: String,
    /// Bytes written, once saved.
    pub bytes: usize,
    /// Progress, done or failed.
    pub state: State,
}

/** The session's downloads, newest first and bounded. */
#[derive(Default)]
pub struct Downloads {
    /// Newest first.
    pub items: Vec<Download>,
    next: u64,
}

impl Downloads {
    /** List a new download as in progress and return its id. */
    pub fn start(&mut self, url: &str) -> u64 {
        self.next += 1;
        self.items.insert(
            0,
            Download {
                id: self.next,
                url: url.into(),
                name: file_name(url, "", None),
                bytes: 0,
                state: State::Progress,
            },
        );
        self.items.truncate(DOWNLOADS_KEPT);
        self.next
    }

    /** Record how one download ended. */
    pub fn finish(&mut self, id: u64, result: Result<(PathBuf, usize), String>) {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return;
        };
        match result {
            Ok((path, bytes)) => {
                item.name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                item.bytes = bytes;
                item.state = State::Done(path);
            }
            Err(reason) => item.state = State::Failed(reason),
        }
    }

    /** Whether any download is still running, for the toolbar icon. */
    pub fn active(&self) -> bool {
        self.items.iter().any(|item| item.state == State::Progress)
    }
}

/** A safe file name for a download: the server's attachment name, else the address's last segment, else "download", with an extension from the type when it has none. */
pub fn file_name(url: &str, mime: &str, disposition: Option<&str>) -> String {
    let from_header = disposition.and_then(disposition_name);
    let from_url = url::Url::parse(url).ok().and_then(|parsed| {
        parsed
            .path_segments()
            .and_then(|mut segments| segments.next_back().map(str::to_owned))
            .filter(|segment| !segment.is_empty())
            .map(|segment| percent_decode(&segment))
    });
    let mut name = clean(&from_header.or(from_url).unwrap_or_default());
    if name.is_empty() {
        name = "download".into();
    }
    if !name.contains('.')
        && let Some(extension) = extension(mime)
    {
        name.push('.');
        name.push_str(extension);
    }
    name
}

/** The `filename=` of a Content-Disposition header, if it names one. */
fn disposition_name(value: &str) -> Option<String> {
    value.split(';').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key.trim().eq_ignore_ascii_case("filename")).then(|| value.trim().trim_matches('"').into())
    })
}

/** Strip path separators, control characters and leading dots, so a name cannot leave the folder or hide. */
fn clean(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| !c.is_control() && !['/', '\\', ':', '\0'].contains(c))
        .take(NAME_CHARS)
        .collect();
    kept.trim().trim_start_matches('.').trim().to_owned()
}

/** Decode %XX escapes in a path segment; invalid UTF-8 falls back to the raw text. */
fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(value)) => {
                out.push(value);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| segment.into())
}

/** The usual extension for the types the network service can deliver. */
fn extension(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "text/html" => "html",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => return None,
    })
}

/** Save bytes in `folder` under `name`, or "name (1).ext" and so on when taken. The file is claimed atomically, written read-only-for-others and never executable, and marked quarantined so macOS checks it before anything opens it. */
pub fn save(folder: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::OpenOptionsExt;
    for attempt in 0..NAME_ATTEMPTS {
        let path = folder.join(numbered(name, attempt));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&path);
        match file {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
                quarantine(&path);
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no free file name",
    ))
}

/** "report.pdf", then "report (1).pdf", "report (2).pdf", as Chrome and Oya number them. */
pub fn numbered(name: &str, attempt: usize) -> String {
    if attempt == 0 {
        return name.into();
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem} ({attempt}).{extension}"),
        _ => format!("{name} ({attempt})"),
    }
}

/** Tag the file as downloaded by Mars, so Gatekeeper asks before it runs and Finder warns. */
fn quarantine(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let value = format!("0081;{seconds:x};Mars;");
        let (Ok(file), Ok(name)) = (
            std::ffi::CString::new(path.as_os_str().as_bytes()),
            std::ffi::CString::new("com.apple.quarantine"),
        ) else {
            return;
        };
        unsafe {
            libc::setxattr(
                file.as_ptr(),
                name.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
                0,
            );
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;
}

impl Runtime {
    /** Fetch one address for saving, as the person, through the same governed path as navigation (egress, cookies, redirects, deadline). Nothing is parsed, run or shown. */
    pub fn download(&mut self, url: &str, permit: Permit) -> EngineResult<Fetched> {
        self.permit = Some(permit);
        let result = self.fetch_for_download(url);
        self.permit = None;
        result
    }

    /** A navigation that turned out to be a file, for the desktop to save; taken once. */
    pub fn take_download(&mut self) -> Option<Fetched> {
        self.ready_download.take()
    }

    /** Files and attachments come back as raw bytes and pages as text from a document request;
    when the service refuses the type (an image), ask again for image bytes. */
    fn fetch_for_download(&mut self, url: &str) -> EngineResult<Fetched> {
        let deadline = Instant::now() + WEB_TIMEOUT;
        let initiator = self
            .page
            .as_ref()
            .and_then(|page| url::Url::parse(&page.url).ok());
        match self.acquire(
            WebRequest::get(url),
            deadline,
            initiator.clone(),
            true,
            true,
        ) {
            Ok(acquired) if acquired.response.status != 200 => Err(not_saved()),
            Ok(acquired) if acquired.response.download => Fetched::from_download(acquired.response),
            Ok(acquired) => Ok(Fetched {
                url: acquired.response.url,
                mime: acquired.response.mime,
                bytes: acquired.response.source.into_bytes(),
                disposition: None,
            }),
            // Images display inline, so their bytes come from an explicit image request.
            Err(error) if error.code == "unsupported_feature" => {
                let mut request = WebRequest::get(url);
                request.image = true;
                let acquired = self.acquire(request, deadline, initiator, true, true)?;
                if acquired.response.status != 200 || !acquired.response.image_data {
                    return Err(not_saved());
                }
                Ok(Fetched {
                    url: acquired.response.url,
                    mime: acquired.response.mime,
                    bytes: super::images::decode_base64(&acquired.response.source)?,
                    disposition: None,
                })
            }
            Err(error) => Err(error),
        }
    }
}

/** The server answered, but not with a file Mars can save. */
fn not_saved() -> EngineError {
    EngineError::unsupported("The server did not send a file Mars can save.")
}
