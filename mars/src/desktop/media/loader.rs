/**
 * The native player's only source of bytes. AVFoundation asks this resource loader for byte
 * ranges of a private `mars-media:` URL; it answers from what the coordinator fetched through
 * Relay (or from a Blob's bytes) and records the first byte it is missing, so the player never
 * opens a socket of its own and every fetch keeps Mars's cookies, persona and egress rules.
 */
use crate::runtime::media::bytes::Bytes;
use crate::runtime::media::hub::Shared;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_av_foundation::{
    AVAssetResourceLoader, AVAssetResourceLoaderDelegate, AVAssetResourceLoadingRequest,
};
use objc2_foundation::{NSData, NSError, NSString};
use std::cell::RefCell;

/// Bytes handed to AVFoundation per response call; it accepts any size, this bounds the copy.
const RESPOND_BYTES: usize = 1024 * 1024;
/// The NSError domain loading failures carry.
const ERROR_DOMAIN: &str = "MarsMedia";
/// The error code a failed fetch reports to AVFoundation.
const ERROR_CODE: isize = -1;

/// Loading requests AVFoundation is waiting on, oldest first.
#[derive(Default)]
pub(super) struct Pending {
    requests: RefCell<Vec<Retained<AVAssetResourceLoadingRequest>>>,
}

define_class!(
    #[unsafe(super=NSObject)]
    #[thread_kind=MainThreadOnly]
    #[ivars=Pending]
    pub(super) struct Loader;
    unsafe impl NSObjectProtocol for Loader {}
    unsafe impl AVAssetResourceLoaderDelegate for Loader {
        /** Every request waits for bytes the coordinator fetches; none is refused here. */
        #[unsafe(method(resourceLoader:shouldWaitForLoadingOfRequestedResource:))]
        fn should_wait(
            &self,
            _loader: &AVAssetResourceLoader,
            request: &AVAssetResourceLoadingRequest,
        ) -> bool {
            self.ivars().requests.borrow_mut().push(request.retain());
            true
        }
        /** A superseded request (a seek elsewhere) is dropped. */
        #[unsafe(method(resourceLoader:didCancelLoadingRequest:))]
        fn cancelled(
            &self,
            _loader: &AVAssetResourceLoader,
            request: &AVAssetResourceLoadingRequest,
        ) {
            self.ivars()
                .requests
                .borrow_mut()
                .retain(|kept| !std::ptr::eq(&**kept, request));
        }
    }
);

impl Loader {
    /** A loader with nothing pending. */
    pub(super) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Pending::default());
        // SAFETY: NSObject's designated initializer on a freshly allocated instance.
        unsafe { msg_send![super(this), init] }
    }
    /** Answer every pending request from the shared bytes; record the first byte still missing. */
    pub(super) fn serve(&self, shared: &mut Shared) {
        let mut requests = self.ivars().requests.borrow_mut();
        let mut need = None;
        requests.retain(|request| {
            // SAFETY: the request is retained and only touched on the main thread.
            let (open, missing) = unsafe { answer(request, &shared.bytes) };
            if need.is_none() {
                need = missing;
            }
            open
        });
        shared.need = need;
    }
}

/** Answer one request; returns whether it stays pending and the byte it waits for. */
unsafe fn answer(request: &AVAssetResourceLoadingRequest, bytes: &Bytes) -> (bool, Option<u64>) {
    // SAFETY (whole body): AVFoundation's loading request API on the main thread.
    unsafe {
        if request.isCancelled() || request.isFinished() {
            return (false, None);
        }
        if bytes.failed.is_some() {
            let domain = NSString::from_str(ERROR_DOMAIN);
            let error = NSError::errorWithDomain_code_userInfo(&domain, ERROR_CODE, None);
            request.finishLoadingWithError(Some(&error));
            return (false, None);
        }
        let Some(length) = bytes.length else {
            return (true, Some(0));
        };
        if let Some(info) = request.contentInformationRequest() {
            info.setContentType(Some(&NSString::from_str(super::types::uti(&bytes.mime))));
            info.setContentLength(length as i64);
            info.setByteRangeAccessSupported(true);
        }
        let Some(data) = request.dataRequest() else {
            request.finishLoading();
            return (false, None);
        };
        let end = if data.requestsAllDataToEndOfResource() {
            length
        } else {
            (data.requestedOffset() as u64 + data.requestedLength() as u64).min(length)
        };
        let mut offset = data.currentOffset() as u64;
        while offset < end {
            let chunk = bytes.read(offset, RESPOND_BYTES.min((end - offset) as usize));
            if chunk.is_empty() {
                return (true, Some(offset));
            }
            data.respondWithData(&NSData::with_bytes(chunk));
            offset += chunk.len() as u64;
        }
        request.finishLoading();
        (false, None)
    }
}
