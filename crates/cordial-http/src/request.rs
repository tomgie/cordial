//! Request bodies and the `Endpoint` trait.

use std::sync::atomic::{AtomicU64, Ordering};

use bytes::{BufMut, Bytes, BytesMut};
use cordial_model::validate::ValidationError;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::route::Route;

/// A request to one endpoint of the Discord API.
pub trait Endpoint: Send + Sync {
    /// The type of the response body. `()` for "204 No Content".
    type Response: DeserializeOwned + Send + 'static;

    /// Gives the route.
    fn route(&self) -> Route;

    /// Gives the body. When `validate` is `true`, the client-side checks run first.
    ///
    /// # Errors
    ///
    /// Gives an error when a value breaks a documented limit, or the body cannot be encoded.
    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        let _ = validate;
        Ok(RequestBody::None)
    }

    /// Gives the reason for the audit log, if the endpoint supports one.
    fn audit_reason(&self) -> Option<&str> {
        None
    }

    /// Tells if a POST request is safe to retry, for example a message with `enforce_nonce`.
    fn retry_safe(&self) -> bool {
        self.route().method.is_idempotent()
    }
}

/// The body of a request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestBody {
    /// No body.
    #[default]
    None,
    /// A JSON body.
    Json(Bytes),
    /// A form body (`application/x-www-form-urlencoded`), for OAuth2.
    Form(Bytes),
    /// A multipart body with files.
    Multipart(Multipart),
}

impl RequestBody {
    /// Makes a JSON body.
    ///
    /// # Errors
    ///
    /// Gives an error when the value cannot be encoded.
    pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<Self, Error> {
        serde_json::to_vec(value)
            .map(|bytes| Self::Json(bytes.into()))
            .map_err(Error::Encode)
    }

    /// Makes a JSON body, or a multipart body when there are files.
    ///
    /// # Errors
    ///
    /// Gives an error when the value cannot be encoded.
    pub fn json_with_files<T: Serialize + ?Sized>(
        value: &T,
        files: &[FileUpload],
    ) -> Result<Self, Error> {
        if files.is_empty() {
            return Self::json(value);
        }
        let payload = serde_json::to_vec(value).map_err(Error::Encode)?;
        Ok(Self::Multipart(Multipart {
            payload_json: Some(payload.into()),
            files: files.to_vec(),
            fields: Vec::new(),
        }))
    }

    /// Gives the `Content-Type` and the bytes of the body.
    #[must_use]
    pub fn encode(&self) -> Option<(String, Bytes)> {
        match self {
            Self::None => None,
            Self::Json(bytes) => Some(("application/json".to_owned(), bytes.clone())),
            Self::Form(bytes) => Some((
                "application/x-www-form-urlencoded".to_owned(),
                bytes.clone(),
            )),
            Self::Multipart(multipart) => Some(multipart.encode()),
        }
    }
}

/// A file to upload.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileUpload {
    /// The file name.
    pub filename: String,
    /// The content.
    pub data: Bytes,
    /// The media type. The default is `application/octet-stream`.
    pub content_type: Option<String>,
}

impl FileUpload {
    /// Makes a file upload.
    pub fn new(filename: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Self {
            filename: filename.into(),
            data: data.into(),
            content_type: None,
        }
    }

    /// Sets the media type, for example `audio/ogg` for a voice message.
    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

/// A `multipart/form-data` body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Multipart {
    /// The JSON part `payload_json`.
    pub payload_json: Option<Bytes>,
    /// The files. File `n` goes in the part `files[n]`.
    pub files: Vec<FileUpload>,
    /// Other plain text fields, for example the fields of a sticker upload.
    pub fields: Vec<(String, String)>,
}

impl Multipart {
    /// Makes an empty multipart body.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a plain text field.
    #[must_use]
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// Adds a file part with a specific part name, for example `file`.
    #[must_use]
    pub fn named_file(mut self, part_name: &str, file: FileUpload) -> Self {
        let mut file = file;
        // The part name goes in front of the file name, separated by a NUL that file names
        // cannot have. `encode` splits it again.
        file.filename = format!("{part_name}\0{}", file.filename);
        self.files.push(file);
        self
    }

    /// Gives the `Content-Type` with the boundary, and the encoded body.
    #[must_use]
    pub fn encode(&self) -> (String, Bytes) {
        let boundary = make_boundary();
        let mut body = BytesMut::new();

        if let Some(payload) = &self.payload_json {
            write_part_header(
                &mut body,
                &boundary,
                "payload_json",
                None,
                Some("application/json"),
            );
            body.put_slice(payload);
            body.put_slice(b"\r\n");
        }

        for (name, value) in &self.fields {
            write_part_header(&mut body, &boundary, name, None, None);
            body.put_slice(value.as_bytes());
            body.put_slice(b"\r\n");
        }

        for (index, file) in self.files.iter().enumerate() {
            let (part_name, filename) = match file.filename.split_once('\0') {
                Some((part_name, filename)) => (part_name.to_owned(), filename),
                None => (format!("files[{index}]"), file.filename.as_str()),
            };
            let content_type = file
                .content_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            write_part_header(
                &mut body,
                &boundary,
                &part_name,
                Some(filename),
                Some(content_type),
            );
            body.put_slice(&file.data);
            body.put_slice(b"\r\n");
        }

        body.put_slice(format!("--{boundary}--\r\n").as_bytes());
        (
            format!("multipart/form-data; boundary={boundary}"),
            body.freeze(),
        )
    }
}

fn write_part_header(
    body: &mut BytesMut,
    boundary: &str,
    name: &str,
    filename: Option<&str>,
    content_type: Option<&str>,
) {
    body.put_slice(format!("--{boundary}\r\n").as_bytes());
    let mut disposition = format!(
        "Content-Disposition: form-data; name=\"{}\"",
        escape_quotes(name)
    );
    if let Some(filename) = filename {
        disposition.push_str(&format!("; filename=\"{}\"", escape_quotes(filename)));
    }
    body.put_slice(disposition.as_bytes());
    body.put_slice(b"\r\n");
    if let Some(content_type) = content_type {
        body.put_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
    }
    body.put_slice(b"\r\n");
}

fn escape_quotes(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ")
}

fn make_boundary() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("cordial-{time:x}-{count:x}")
}

/// Image data for a request, as a data URI: `data:image/png;base64,...`.
///
/// Endpoints that set an icon, an avatar, a banner, or an emoji take this type.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ImageData(String);

impl ImageData {
    /// Makes image data from bytes and a media type, for example `image/png`.
    #[must_use]
    pub fn new(content_type: &str, bytes: &[u8]) -> Self {
        Self(format!(
            "data:{content_type};base64,{}",
            encode_base64(bytes)
        ))
    }

    /// Makes image data from bytes. The media type comes from the first bytes
    /// (PNG, JPEG, GIF, WebP, or AVIF), else `image/png`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::new(sniff_image_type(bytes), bytes)
    }

    /// Makes image data from a complete data URI.
    #[must_use]
    pub fn from_data_uri(uri: impl Into<String>) -> Self {
        Self(uri.into())
    }

    /// Gives the data URI.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = self.0.split(',').next().unwrap_or_default();
        write!(formatter, "ImageData({header},<{} bytes>)", self.0.len())
    }
}

fn sniff_image_type(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [b'G', b'I', b'F', b'8', ..] => "image/gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        [
            _,
            _,
            _,
            _,
            b'f',
            b't',
            b'y',
            b'p',
            b'a',
            b'v',
            b'i',
            b'f',
            ..,
        ] => "image/avif",
        _ => "image/png",
    }
}

/// Encodes bytes as standard base64 with padding.
#[must_use]
pub fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let combined = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);

        output.push(char::from(ALPHABET[(combined >> 18) as usize & 63]));
        output.push(char::from(ALPHABET[(combined >> 12) as usize & 63]));
        output.push(if chunk.len() > 1 {
            char::from(ALPHABET[(combined >> 6) as usize & 63])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(ALPHABET[combined as usize & 63])
        } else {
            '='
        });
    }
    output
}

/// Runs a check when `validate` is `true`, and converts its error.
pub(crate) fn check(
    validate: bool,
    check: impl FnOnce() -> Result<(), ValidationError>,
) -> Result<(), Error> {
    if validate {
        check().map_err(Error::Validation)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_multipart() {
        let multipart = Multipart {
            payload_json: Some(Bytes::from_static(br#"{"content":"hi"}"#)),
            files: vec![FileUpload::new("a.txt", &b"hello"[..]).content_type("text/plain")],
            fields: Vec::new(),
        };

        let (content_type, body) = multipart.encode();
        let text = String::from_utf8(body.to_vec()).unwrap();
        let boundary = content_type
            .strip_prefix("multipart/form-data; boundary=")
            .unwrap();

        assert!(text.starts_with(&format!("--{boundary}\r\n")));
        assert!(text.contains("name=\"payload_json\"\r\nContent-Type: application/json\r\n\r\n{\"content\":\"hi\"}\r\n"));
        assert!(text.contains(
            "name=\"files[0]\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n"
        ));
        assert!(text.ends_with(&format!("--{boundary}--\r\n")));
    }

    #[test]
    fn encodes_base64_and_image_data() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert_eq!(encode_base64(b"foobar"), "Zm9vYmFy");

        let png = ImageData::from_bytes(&[0x89, b'P', b'N', b'G', 1]);
        assert!(png.as_str().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn named_file_part() {
        let multipart = Multipart::new()
            .field("name", "wave")
            .named_file("file", FileUpload::new("wave.png", &b"x"[..]));

        let (_, body) = multipart.encode();
        let text = String::from_utf8(body.to_vec()).unwrap();

        assert!(text.contains("name=\"file\"; filename=\"wave.png\""));
        assert!(text.contains("name=\"name\"\r\n\r\nwave\r\n"));
    }
}
