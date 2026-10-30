/** Captured Blob workers inherit creator policy and never enter the network acquisition path. */
use super::*;
use crate::v9::BlobUrlSource;

impl Runtime {
    /** Source capture conveys bytes only; publication, origin and CSP still admit execution. */
    pub(in crate::runtime) fn worker_blob_source(
        &mut self,
        creator: &Policy,
        address: &str,
        captured: Option<&BlobUrlSource>,
    ) -> EngineResult<Option<Source>> {
        let started = Instant::now();
        self.publish(|_| ())
            .inspect_err(|error| record(true, started, Failure::engine(error)))?;
        let Some(source) = captured else {
            return refused(true, started, Failure::Address);
        };
        source.validate(address)?;
        let url =
            Url::parse(address).map_err(|_| EngineError::invalid("Invalid Blob worker URL."))?;
        if source.origin != creator.base.origin().ascii_serialization() {
            return refused(true, started, Failure::Origin);
        }
        if !csp::allows_worker(&creator.csp, &creator.base, &url, true) {
            return refused(true, started, Failure::Policy);
        }
        let text = String::from_utf8_lossy(&source.bytes)
            .trim_start_matches('\u{feff}')
            .to_owned();
        if text.len() > crate::constants::SCRIPT_BYTES {
            return refused(true, started, Failure::SourceLimit);
        }
        record(true, started, Failure::Loaded);
        Ok(Some(Source {
            text,
            policy: Policy {
                base: url,
                partition: creator.partition.clone(),
                csp: creator.csp.clone(),
            },
        }))
    }
}
