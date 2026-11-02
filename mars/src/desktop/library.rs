/** The person's history, bookmarks and settings, loaded once and saved off the UI thread. */
use crate::{
    profile::{Record, RecordStore},
    runtime::{bookmarks::Bookmarks, history::History, settings::Settings},
};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::Duration,
};

/// A failed save is retried after this long; the newest record replaces an older pending one.
const RETRY: Duration = Duration::from_secs(2);

/** In-memory library state; every change is queued for the secure store, newest wins. */
pub(super) struct Library {
    pub history: History,
    pub bookmarks: Bookmarks,
    pub settings: Settings,
    writer: Writer,
}

impl Library {
    /** Read each record as untrusted input; unreadable or invalid records start empty and are replaced on the next change. */
    pub fn load(store: Arc<dyn RecordStore>) -> Self {
        let read = |record| store.load(record).ok().flatten();
        let history = read(Record::History)
            .and_then(|bytes| History::decode(&bytes).ok())
            .unwrap_or_default();
        let bookmarks = read(Record::Bookmarks)
            .and_then(|bytes| Bookmarks::decode(&bytes).ok())
            .unwrap_or_default();
        let settings = read(Record::Settings)
            .and_then(|bytes| Settings::decode(&bytes).ok())
            .unwrap_or_default();
        Self {
            history,
            bookmarks,
            settings,
            writer: Writer::start(store),
        }
    }

    /** Close the write queue and join only after its final secure-store attempt has returned. */
    pub fn shutdown_finished(&mut self) -> bool {
        self.writer.sender.take();
        if self
            .writer
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            return false;
        }
        if let Some(thread) = self.writer.thread.take() {
            let _ = thread.join();
        }
        true
    }

    /** Queue the current state of one record for saving; an unencodable state is never written. */
    pub fn save(&self, record: Record) {
        let bytes = match record {
            Record::History => self.history.encode(),
            Record::Bookmarks => self.bookmarks.encode(),
            Record::Settings => self.settings.encode(),
            // The runtime saves the platform APIs' records itself; the library never holds them.
            Record::ServiceWorkers | Record::CacheStorage | Record::Credentials => return,
        };
        if let Ok(bytes) = bytes {
            self.writer.send(record, bytes);
        }
    }
}

/** One background thread owns secure-store writes so a slow Keychain never stalls the window. */
struct Writer {
    sender: Option<mpsc::Sender<(Record, Vec<u8>)>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /** Start the writer; pending records are retried until they save or the window closes. */
    fn start(store: Arc<dyn RecordStore>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("oya-library-writer".into())
            .spawn(move || write_loop(store.as_ref(), &receiver))
            .ok();
        Self {
            sender: Some(sender),
            thread,
        }
    }

    /** Hand one complete record to the writer. */
    fn send(&self, record: Record, bytes: Vec<u8>) {
        if let Some(sender) = &self.sender {
            let _ = sender.send((record, bytes));
        }
    }
}

impl Drop for Writer {
    /** Closing the window makes one last attempt at every pending record, then joins the writer. */
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/** Coalesce queued records, newest per kind, and keep failed ones for the next attempt. */
fn write_loop(store: &dyn RecordStore, receiver: &mpsc::Receiver<(Record, Vec<u8>)>) {
    let mut pending: HashMap<Record, Vec<u8>> = HashMap::new();
    loop {
        let closed = match receiver.recv_timeout(RETRY) {
            Ok((record, bytes)) => {
                pending.insert(record, bytes);
                while let Ok((record, bytes)) = receiver.try_recv() {
                    pending.insert(record, bytes);
                }
                false
            }
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            Err(mpsc::RecvTimeoutError::Disconnected) => true,
        };
        pending.retain(|record, bytes| store.replace(*record, bytes).is_err());
        if closed {
            return;
        }
    }
}
