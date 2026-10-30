/** Each image hop derives credentials and origin-clean admission from actual governed responses. */
use crate::{
    network::{
        Destination, WebRequest, WebResponse,
        cors::{Flow, Mode, Tainting, allows},
    },
    protocol::{EngineError, EngineResult},
    v9::ImageCors,
};
use url::{Origin, Url};

/** Redirect provenance is candidate-local and can only become more restrictive during acquisition. */
pub(super) struct ImageFlow {
    cors: ImageCors,
    flow: Flow,
    origin: Origin,
    clean: bool,
}

impl ImageFlow {
    /** HTML anonymous uses same-origin credentials; missing and use-credentials both include cookies. */
    pub(super) fn new(base: &Url, cors: ImageCors) -> Self {
        let mode = if cors == ImageCors::NoCors {
            Mode::NoCors
        } else {
            Mode::Cors
        };
        Self {
            cors,
            flow: Flow::new(base.origin(), mode),
            origin: base.origin(),
            clean: true,
        }
    }

    /** Keep the Image destination and binary bounds while selecting host-only Origin and cookie policy. */
    pub(super) fn request(&mut self, url: &Url, bitmap: bool) -> (WebRequest, bool) {
        self.flow.enter(url);
        self.clean &= url.origin() == self.origin;
        let cors = self.cors != ImageCors::NoCors;
        let credentials =
            self.cors != ImageCors::Anonymous || self.flow.tainting() == Tainting::Basic;
        let request = WebRequest {
            destination: Destination::Image,
            image: bitmap,
            mode: cors.then_some(Mode::Cors),
            origin: cors.then(|| self.flow.serialized()),
            ..WebRequest::get(url.as_str())
        };
        (request, credentials)
    }

    /** Every redirect and final response must pass before any bytes can acquire clean canvas provenance. */
    pub(super) fn admit(&mut self, hop: &Url, response: &WebResponse) -> EngineResult<()> {
        let actual = Url::parse(&response.url).map_err(|_| refused())?;
        self.clean &= actual.origin() == self.origin;
        if self.cors == ImageCors::NoCors {
            return Ok(());
        }
        if [
            "access-control-allow-origin",
            "access-control-allow-credentials",
        ]
        .iter()
        .any(|name| {
            response
                .headers
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case(name))
                .count()
                > 1
        }) {
            return Err(refused());
        }
        // A hidden cross-origin redirect would omit a required response check; never trust its final URL alone.
        if actual.origin() != hop.origin()
            || (self.flow.tainting() == Tainting::Cors
                && !allows(
                    &response.headers,
                    &self.flow.serialized(),
                    self.cors == ImageCors::UseCredentials,
                ))
        {
            return Err(refused());
        }
        Ok(())
    }

    /** Cross-origin redirect chains may require Origin:null; the shared Fetch algorithm owns that rule. */
    pub(super) fn redirect(&mut self, from: &Url, to: &Url) {
        self.flow.redirect(from, to);
    }

    /** A CORS success is clean because every response passed; no-CORS retains all-hop conservative taint. */
    pub(super) fn origin_clean(&self) -> bool {
        self.cors != ImageCors::NoCors || self.clean
    }
}

/** A CORS refusal is not an unsupported image format, so it cannot enter the SVG retry path. */
fn refused() -> EngineError {
    EngineError::invalid("The image CORS response was refused.")
}

#[cfg(test)]
mod tests;
