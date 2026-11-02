/** Synchronous worker imports use correlated bounded callbacks on the existing private pipe. */
use crate::constants::{SCRIPT_BYTES, SCRIPT_TOTAL_BYTES, URL_BYTES, VERSION};
use crate::protocol::{EngineError, EngineResult, decode, encode, read_frame};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::sync::Mutex;

/** Each exchange allows one outstanding callback and a finite number of imported scripts. */
pub(super) const CALLBACKS: u32 = 32;
/** A distinct discriminator keeps source requests separate from terminal protocol replies. */
const KIND: &str = "worker_import";

/** Only URL data crosses from V9; the coordinator already owns the worker's policy and permit. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub version: u32,
    pub callback: String,
    pub id: String,
    pub sequence: u32,
    pub url: String,
}

/** One loading outcome answers exactly one callback; engine failures stay uncatchable in V9. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    version: u32,
    id: String,
    sequence: u32,
    source: Option<String>,
    error: Option<EngineError>,
}

/** Correlation validates before any callback can cause an acquisition. */
pub(super) fn request(frame: &[u8], id: &str, sequence: u32) -> EngineResult<Option<Request>> {
    #[derive(Deserialize)]
    struct Probe {
        callback: Option<String>,
    }
    let probe: Probe = decode(frame)?;
    if probe.callback.is_none() {
        return Ok(None);
    }
    let request: Request = decode(frame)?;
    if request.version != VERSION
        || request.callback != KIND
        || request.id != id
        || request.sequence != sequence
        || sequence == 0
        || sequence > CALLBACKS
        || request.url.len() > URL_BYTES
    {
        return Err(EngineError::service("v9", false));
    }
    Ok(Some(request))
}

/** Encode a single outcome after governed loading; the transport does not interpret source. */
pub(super) fn answer(
    request: Request,
    result: EngineResult<Option<String>>,
) -> EngineResult<Vec<u8>> {
    let (source, error) = match result {
        Ok(source) => (source, None),
        Err(error) => (None, Some(error)),
    };
    encode(&Answer {
        version: VERSION,
        id: request.id,
        sequence: request.sequence,
        source,
        error,
    })
}

/** The VM borrows the service loop's streams; no second reader or persistent capability exists. */
pub(super) struct Pipe<'a> {
    id: &'a str,
    state: Mutex<Streams<'a>>,
}
/** The mutex makes a borrowed callback available on the VM's bounded execution-stack thread. */
struct Streams<'a> {
    input: &'a mut (dyn BufRead + Send),
    output: &'a mut (dyn Write + Send),
    sequence: u32,
    bytes: usize,
}

impl<'a> Pipe<'a> {
    /** One capability exists only while the service is servicing this outer worker call. */
    pub fn new(
        id: &'a str,
        input: &'a mut (dyn BufRead + Send),
        output: &'a mut (dyn Write + Send),
    ) -> Self {
        Self {
            id,
            state: Mutex::new(Streams {
                input,
                output,
                sequence: 0,
                bytes: 0,
            }),
        }
    }
}

impl crate::v9::ImportScriptsHost for Pipe<'_> {
    /** Release the stream lock before imported JavaScript executes, permitting nested imports. */
    fn load(&self, url: &str) -> EngineResult<Option<String>> {
        let mut stream = self
            .state
            .lock()
            .map_err(|_| EngineError::service("v9", false))?;
        if stream.sequence >= CALLBACKS || url.len() > URL_BYTES {
            return Err(EngineError::limit(
                "worker import callbacks",
                CALLBACKS as usize,
            ));
        }
        stream.sequence += 1;
        let request = Request {
            version: VERSION,
            callback: KIND.into(),
            id: self.id.into(),
            sequence: stream.sequence,
            url: url.into(),
        };
        stream
            .output
            .write_all(&encode(&request)?)
            .map_err(closed)?;
        stream.output.flush().map_err(closed)?;
        let frame =
            read_frame(&mut stream.input)?.ok_or_else(|| EngineError::service("v9", false))?;
        let answer: Answer = decode(&frame)?;
        if answer.version != VERSION
            || answer.id != self.id
            || answer.sequence != stream.sequence
            || (answer.source.is_some() && answer.error.is_some())
        {
            return Err(EngineError::service("v9", false));
        }
        if let Some(error) = answer.error {
            return Err(error);
        }
        let size = answer.source.as_ref().map_or(0, String::len);
        stream.bytes = stream.bytes.saturating_add(size);
        if size > SCRIPT_BYTES || stream.bytes > SCRIPT_TOTAL_BYTES {
            return Err(EngineError::limit(
                "worker imported source bytes",
                SCRIPT_TOTAL_BYTES,
            ));
        }
        Ok(answer.source)
    }
}

/** A broken callback pipe ends this execution, never falling back to direct networking. */
fn closed(_: std::io::Error) -> EngineError {
    EngineError::service("v9", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v9::ImportScriptsHost;
    use std::io::Cursor;

    /** A source callback has only its outer exchange correlation and inert requested URL. */
    fn callback(sequence: u32) -> Request {
        Request {
            version: VERSION,
            callback: KIND.into(),
            id: "outer-7".into(),
            sequence,
            url: "https://example.test/import.js".into(),
        }
    }

    #[test]
    fn callbacks_require_exact_outer_identity_sequence_and_schema() {
        let good = encode(&callback(1)).unwrap();
        assert!(request(&good, "outer-7", 1).unwrap().is_some());
        assert!(request(&good, "other", 1).is_err());
        assert!(request(&good, "outer-7", 2).is_err());
        assert!(
            request(
                &encode(&callback(CALLBACKS + 1)).unwrap(),
                "outer-7",
                CALLBACKS + 1
            )
            .is_err()
        );
        assert!(request(br#"{"callback":"worker_import","version":2,"id":"outer-7","sequence":1,"url":"https://example.test/","owner":"other"}"#, "outer-7", 1).is_err());
    }

    #[test]
    fn sequential_nested_loads_use_the_same_reader_without_holding_a_vm_borrow() {
        let mut input = Cursor::new(
            [
                answer(callback(1), Ok(Some("first".into()))).unwrap(),
                answer(callback(2), Ok(Some("second".into()))).unwrap(),
            ]
            .concat(),
        );
        let mut output = Vec::new();
        let pipe = Pipe::new("outer-7", &mut input, &mut output);
        assert_eq!(
            pipe.load("https://example.test/one").unwrap().as_deref(),
            Some("first")
        );
        assert_eq!(
            pipe.load("https://example.test/two").unwrap().as_deref(),
            Some("second")
        );
        drop(pipe);
        let mut sent = Cursor::new(output);
        assert!(
            request(&read_frame(&mut sent).unwrap().unwrap(), "outer-7", 1)
                .unwrap()
                .is_some()
        );
        assert!(
            request(&read_frame(&mut sent).unwrap().unwrap(), "outer-7", 2)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn malformed_answers_and_eof_end_the_callback_without_fallback() {
        for bytes in [
            Vec::new(),
            answer(callback(2), Ok(Some("stale".into()))).unwrap(),
        ] {
            let mut input = Cursor::new(bytes);
            let mut output = Vec::new();
            assert!(
                Pipe::new("outer-7", &mut input, &mut output)
                    .load("https://example.test/")
                    .is_err()
            );
        }
    }

    #[test]
    fn host_resource_and_authority_failures_remain_fatal_engine_errors() {
        let denied = EngineError::limit("test callback", 1);
        let mut input = Cursor::new(answer(callback(1), Err(denied.clone())).unwrap());
        let mut output = Vec::new();
        let error = Pipe::new("outer-7", &mut input, &mut output)
            .load("https://example.test/")
            .unwrap_err();
        assert_eq!(error.code, denied.code);
    }
}
