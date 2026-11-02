/** Pollable bounded NDJSON keeps stdin and blocked stdout outside the window lifetime. */
use super::broker::{Command, POLL};
use crate::{
    constants::FRAME_BYTES,
    protocol::{EngineError, EngineResult, Request, Response, decode, encode},
    runtime::authority::{self, Owner, Permit, SharedAuthority},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

/** OS descriptor flags are restored after both tasks relinquish their descriptors. */
pub(super) struct Descriptor {
    pub fd: i32,
    flags: i32,
}
impl Descriptor {
    /** Nonblocking descriptors make cancellation effective even with an idle pipe. */
    pub fn new(fd: i32) -> EngineResult<Self> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(EngineError::invalid(
                "Cannot configure cancellable agent pipes.",
            ));
        }
        Ok(Self { fd, flags })
    }
}
impl Drop for Descriptor {
    /** Restore the caller-owned stream flags after use. */
    fn drop(&mut self) {
        unsafe { libc::fcntl(self.fd, libc::F_SETFL, self.flags) };
    }
}

/** Where requests go: commands to the worker, and window observations to the UI thread when a
test or the native transport supplies that channel. */
pub(super) type Route = (
    SyncSender<Command>,
    Option<SyncSender<super::broker::Event>>,
);

/** Window observations and screenshots go straight to the UI thread. Queued behind the worker's
command channel they waited for its current page step, which on a busy page is seconds long. */
pub(super) fn start(
    commands: SyncSender<Command>,
    events: SyncSender<super::broker::Event>,
    output: Receiver<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) -> EngineResult<Vec<JoinHandle<()>>> {
    let input = Descriptor::new(libc::STDIN_FILENO)?;
    let stdout = Descriptor::new(libc::STDOUT_FILENO)?;
    Ok(start_descriptors(
        input,
        stdout,
        (commands, Some(events)),
        output,
        authority,
        cancel,
        connected,
    ))
}

/** Descriptor injection keeps the native framing and shutdown path for hermetic tests, which
pass no event channel: window requests then take the command path, as on the MCP transport. */
pub(super) fn start_descriptors(
    input: Descriptor,
    stdout: Descriptor,
    route: Route,
    output: Receiver<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) -> Vec<JoinHandle<()>> {
    start_routed(input, stdout, route, output, authority, cancel, connected)
}

/** Optional native test routing shares the same bounded framing and shutdown path. */
fn start_routed(
    input: Descriptor,
    stdout: Descriptor,
    route: Route,
    output: Receiver<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) -> Vec<JoinHandle<()>> {
    let (guard, stop, live) = (authority.clone(), cancel.clone(), connected.clone());
    let reader = thread::spawn(move || {
        read(input, &route, &guard, &stop);
        disconnect(&guard, &live);
    });
    let writer = thread::spawn(move || {
        write(stdout, output, &cancel);
        disconnect(&authority, &connected);
    });
    vec![reader, writer]
}
/** EOF, malformed framing and saturated admission all revoke ownership immediately. */
fn read(
    input: Descriptor,
    route: &(
        SyncSender<Command>,
        Option<SyncSender<super::broker::Event>>,
    ),
    authority: &SharedAuthority,
    cancel: &AtomicBool,
) {
    let mut pending = Vec::new();
    let mut bytes = [0_u8; 4096];
    while !cancel.load(Ordering::Relaxed) {
        if !ready(input.fd, libc::POLLIN) {
            continue;
        }
        let count = unsafe { libc::read(input.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
        if count <= 0 {
            if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                continue;
            }
            return;
        }
        for byte in &bytes[..count as usize] {
            if *byte == b'\n' {
                let Ok(request) = decode::<Request>(&pending) else {
                    return;
                };
                if request.validate().is_err() {
                    return;
                }
                pending.clear();
                let Ok(state) = authority::lock(authority) else {
                    return;
                };
                let key = request
                    .params
                    .get("tab_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|id| crate::runtime::tab_key(id).ok());
                let permit = key
                    .and_then(|id| state.observe_for(Owner::Agent, id).ok())
                    .unwrap_or(Permit {
                        owner: Owner::Agent,
                        epoch: state.epoch,
                        tab_id: key,
                    });
                drop(state);
                let window_op =
                    request.op.starts_with("window.") || request.op == "page.screenshot";
                if let Some(events) = route.1.as_ref().filter(|_| window_op) {
                    if events
                        .try_send(super::broker::Event::Window(request, permit))
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                if route.0.try_send(Command::Agent(request, permit)).is_err() {
                    return;
                }
            } else {
                pending.push(*byte);
                if pending.len() >= FRAME_BYTES {
                    return;
                }
            }
        }
    }
}
/** Partial writes and pipe backpressure are bounded by cancellation, not a blocking flush. */
fn write(output: Descriptor, responses: Receiver<Response>, cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        let response = match responses.recv_timeout(POLL) {
            Ok(response) => response,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return,
        };
        let timed = std::time::Instant::now();
        let Ok(bytes) = encode(&response) else {
            return;
        };
        crate::runtime::perf::record("io.encode", timed, || bytes.len().to_string());
        let timed = std::time::Instant::now();
        let mut offset = 0;
        while offset < bytes.len() && !cancel.load(Ordering::Relaxed) {
            if !ready(output.fd, libc::POLLOUT) {
                continue;
            }
            let count = unsafe {
                libc::write(
                    output.fd,
                    bytes[offset..].as_ptr().cast(),
                    bytes.len() - offset,
                )
            };
            if count > 0 {
                offset += count as usize;
            } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                return;
            }
        }
        crate::runtime::perf::record("io.write", timed, String::new);
    }
}
/** Fixed short polling intervals cap shutdown latency without busy waiting. */
pub(super) fn ready(fd: i32, events: i16) -> bool {
    let mut descriptor = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    unsafe { libc::poll(&mut descriptor, 1, POLL.as_millis() as i32) > 0 }
}
/** Disconnect invalidates both queued admissions and work already waiting on an engine. */
fn disconnect(authority: &SharedAuthority, connected: &AtomicBool) {
    connected.store(false, Ordering::Relaxed);
    if let Ok(mut state) = authority::lock(authority) {
        state.change(Owner::Human);
    }
}

/** MCP distinguishes descriptor faults from ordinary idle polls so ready sessions cannot hang. */
pub(super) fn poll_ready(fd: i32, events: i16) -> io::Result<bool> {
    let mut descriptor = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let count = unsafe { libc::poll(&mut descriptor, 1, POLL.as_millis() as i32) };
    if count < 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(error)
        };
    }
    if descriptor.revents & libc::POLLNVAL != 0 {
        return Err(io::Error::from(io::ErrorKind::BrokenPipe));
    }
    Ok(count > 0)
}
