/**
 * CoreMIDI through its C API: endpoint listing with names and unique IDs, one process-wide
 * client with an output port and an input port, MIDISend for output and a read callback that
 * queues incoming packets for the coordinator to drain.
 */
use objc2::rc::Retained;
use objc2_foundation::NSString;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

/** noErr. */
const NO_ERR: i32 = 0;
/** Bytes of a MIDIPacketList that holds one message of up to MAX_MESSAGE bytes. */
const PACKET_LIST_BYTES: usize = 1024;
/** The longest message one send carries (a long sysex is split by the caller's data anyway). */
const MAX_MESSAGE: usize = 768;
/** Bytes before a packet's data: a 64-bit timestamp and a 16-bit length. */
const PACKET_HEADER: usize = 10;
/** Messages kept for the coordinator; the oldest are dropped past this. */
const QUEUED_MESSAGES: usize = 1024;

#[link(name = "CoreMIDI", kind = "framework")]
unsafe extern "C" {
    static kMIDIPropertyName: *const c_void;
    static kMIDIPropertyManufacturer: *const c_void;
    static kMIDIPropertyUniqueID: *const c_void;
    static kMIDIPropertyDriverVersion: *const c_void;
    fn MIDIGetNumberOfSources() -> usize;
    fn MIDIGetSource(index: usize) -> u32;
    fn MIDIGetNumberOfDestinations() -> usize;
    fn MIDIGetDestination(index: usize) -> u32;
    fn MIDIObjectGetStringProperty(
        object: u32,
        property: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    fn MIDIObjectGetIntegerProperty(object: u32, property: *const c_void, value: *mut i32) -> i32;
    fn MIDIClientCreate(
        name: *const c_void,
        notify: *const c_void,
        context: *mut c_void,
        client: *mut u32,
    ) -> i32;
    fn MIDIOutputPortCreate(client: u32, name: *const c_void, port: *mut u32) -> i32;
    fn MIDIInputPortCreate(
        client: u32,
        name: *const c_void,
        read: extern "C" fn(*const u8, *mut c_void, *mut c_void),
        context: *mut c_void,
        port: *mut u32,
    ) -> i32;
    fn MIDIPortConnectSource(port: u32, source: u32, context: *mut c_void) -> i32;
    fn MIDIPortDisconnectSource(port: u32, source: u32) -> i32;
    fn MIDIPacketListInit(list: *mut u8) -> *mut u8;
    fn MIDIPacketListAdd(
        list: *mut u8,
        size: usize,
        current: *mut u8,
        time: u64,
        length: usize,
        data: *const u8,
    ) -> *mut u8;
    fn MIDISend(port: u32, destination: u32, list: *const u8) -> i32;
    fn CFRelease(object: *const c_void);
}

/** The process's client and its two ports. */
struct Client {
    output: u32,
    input: u32,
}

/** Input packets CoreMIDI delivered, by source unique ID. */
static INBOX: Mutex<VecDeque<(i32, Vec<u8>)>> = Mutex::new(VecDeque::new());
/** The client, made on first use. */
static CLIENT: OnceLock<Option<Client>> = OnceLock::new();

/** A CFString-compatible NSString. */
fn cf(text: &str) -> Retained<NSString> {
    NSString::from_str(text)
}

/** The client, creating it once. */
fn client() -> Option<&'static Client> {
    CLIENT
        .get_or_init(|| {
            let (mut client, mut output, mut input) = (0, 0, 0);
            let name = cf("Mars Web MIDI");
            let pointer = (&*name as *const NSString).cast();
            // SAFETY: valid out pointers; the read callback is a plain extern fn without context.
            unsafe {
                if MIDIClientCreate(pointer, std::ptr::null(), std::ptr::null_mut(), &mut client)
                    != NO_ERR
                    || MIDIOutputPortCreate(client, pointer, &mut output) != NO_ERR
                    || MIDIInputPortCreate(client, pointer, read, std::ptr::null_mut(), &mut input)
                        != NO_ERR
                {
                    return None;
                }
            }
            Some(Client { output, input })
        })
        .as_ref()
}

/** CoreMIDI's read callback: queue each packet's bytes under the source's unique ID. */
extern "C" fn read(list: *const u8, _: *mut c_void, source: *mut c_void) {
    if list.is_null() {
        return;
    }
    // SAFETY: CoreMIDI passes a valid MIDIPacketList for the duration of the callback.
    let packets = unsafe { std::ptr::read_unaligned(list.cast::<u32>()) };
    let mut offset = std::mem::size_of::<u32>();
    let Ok(mut inbox) = INBOX.lock() else {
        return;
    };
    for _ in 0..packets {
        // SAFETY: each packet's length field follows its timestamp; data follows the length.
        let length =
            usize::from(unsafe { std::ptr::read_unaligned(list.add(offset + 8).cast::<u16>()) });
        let data = unsafe { std::slice::from_raw_parts(list.add(offset + PACKET_HEADER), length) }
            .to_vec();
        if inbox.len() >= QUEUED_MESSAGES {
            inbox.pop_front();
        }
        inbox.push_back((source as usize as i32, data));
        offset = next_packet(offset + PACKET_HEADER + length);
    }
}

/** MIDIPacketNext: packets are 4-byte aligned on ARM and packed elsewhere. */
fn next_packet(end: usize) -> usize {
    if cfg!(target_arch = "aarch64") {
        (end + 3) & !3
    } else {
        end
    }
}

/** One endpoint string property. */
fn string(object: u32, property: *const c_void) -> String {
    let mut value: *const c_void = std::ptr::null();
    // SAFETY: on success CoreMIDI returns a +1 CFString, read through NSString and released.
    unsafe {
        if MIDIObjectGetStringProperty(object, property, &mut value) != NO_ERR || value.is_null() {
            return String::new();
        }
        let text = (*value.cast::<NSString>()).to_string();
        CFRelease(value);
        text
    }
}

/** One endpoint integer property. */
fn integer(object: u32, property: *const c_void) -> i32 {
    let mut value = 0;
    // SAFETY: a valid out pointer.
    unsafe { MIDIObjectGetIntegerProperty(object, property, &mut value) };
    value
}

/** One endpoint as a port record. */
fn port(endpoint: u32, kind: &str) -> Value {
    // SAFETY: the property keys are CoreMIDI's exported constants.
    let (name, maker, unique, version) = unsafe {
        (
            kMIDIPropertyName,
            kMIDIPropertyManufacturer,
            kMIDIPropertyUniqueID,
            kMIDIPropertyDriverVersion,
        )
    };
    let prefix = if kind == "input" { "in" } else { "out" };
    json!({"id": format!("{prefix}-{}", integer(endpoint, unique)), "type": kind, "name": string(endpoint, name),
        "manufacturer": string(endpoint, maker), "version": integer(endpoint, version).to_string()})
}

/** Every source (input) and destination (output) now connected. */
pub fn ports() -> Vec<Value> {
    // SAFETY: CoreMIDI's endpoint enumeration.
    unsafe {
        let inputs = (0..MIDIGetNumberOfSources()).map(|i| port(MIDIGetSource(i), "input"));
        let outputs =
            (0..MIDIGetNumberOfDestinations()).map(|i| port(MIDIGetDestination(i), "output"));
        inputs.chain(outputs).collect()
    }
}

/** The endpoint with this unique ID among sources or destinations. */
fn endpoint(unique: i32, input: bool) -> Option<u32> {
    // SAFETY: CoreMIDI's endpoint enumeration and property read.
    unsafe {
        let property = kMIDIPropertyUniqueID;
        if input {
            (0..MIDIGetNumberOfSources())
                .map(|i| MIDIGetSource(i))
                .find(|e| integer(*e, property) == unique)
        } else {
            (0..MIDIGetNumberOfDestinations())
                .map(|i| MIDIGetDestination(i))
                .find(|e| integer(*e, property) == unique)
        }
    }
}

/** Start receiving from one source. */
pub fn connect(unique: i32) -> bool {
    let (Some(client), Some(source)) = (client(), endpoint(unique, true)) else {
        return false;
    };
    // SAFETY: the context pointer carries the unique ID back to `read`.
    unsafe { MIDIPortConnectSource(client.input, source, unique as usize as *mut c_void) == NO_ERR }
}

/** Stop receiving from one source. */
pub fn disconnect(unique: i32) {
    if let (Some(client), Some(source)) = (client(), endpoint(unique, true)) {
        // SAFETY: a port and source CoreMIDI gave us.
        unsafe { MIDIPortDisconnectSource(client.input, source) };
    }
}

/** Send one message sequence to a destination now. */
pub fn send(unique: i32, data: &[u8]) -> bool {
    let (Some(client), Some(destination)) = (client(), endpoint(unique, false)) else {
        return false;
    };
    if data.is_empty() || data.len() > MAX_MESSAGE {
        return data.is_empty();
    }
    let mut buffer = vec![0u8; PACKET_LIST_BYTES];
    // SAFETY: the buffer holds a packet list large enough for one MAX_MESSAGE packet.
    unsafe {
        let current = MIDIPacketListInit(buffer.as_mut_ptr());
        let added = MIDIPacketListAdd(
            buffer.as_mut_ptr(),
            PACKET_LIST_BYTES,
            current,
            0,
            data.len(),
            data.as_ptr(),
        );
        !added.is_null() && MIDISend(client.output, destination, buffer.as_ptr()) == NO_ERR
    }
}

/** Take every queued input message. */
pub fn drain() -> Vec<(i32, Vec<u8>)> {
    INBOX
        .lock()
        .map(|mut inbox| inbox.drain(..).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Listing works with or without devices, and every port has an id and type. */
    #[test]
    fn ports_list_without_devices() {
        for port in ports() {
            assert!(
                port["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("in-") || id.starts_with("out-"))
            );
        }
        assert!(drain().is_empty());
    }

    /** Packet stepping follows the architecture's alignment. */
    #[test]
    fn packets_step_with_alignment() {
        let expected = if cfg!(target_arch = "aarch64") {
            16
        } else {
            13
        };
        assert_eq!(next_packet(13), expected);
    }
}
