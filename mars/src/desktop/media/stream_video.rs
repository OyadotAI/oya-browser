/*!
 * The video of a Media Source player: the first video lane's samples are fed to a VideoToolbox decoder a little ahead of
 * the playhead, and the picture due at the playhead is handed to the painter. Feeding starts at a sync frame and
 * restarts after a seek or when the track's configuration changes, so the decoder never sees a frame whose references
 * it does not hold. Feeding stops while the decoder holds its cap of pictures, so a stream cannot fill memory.
 */
use super::decoder::Decoder;
use crate::runtime::media::avc::parse_avcc;
use crate::runtime::media::constants::MAX_PENDING_PICTURES;
use crate::runtime::media::stream::{Frame, Lane, Stream};
use mars_mp4::TrackKind;
use objc2::rc::Retained;
use objc2_app_kit::NSImage;
use std::collections::VecDeque;

/// How far ahead of the playhead samples are sent to the decoder, in seconds.
const LOOKAHEAD: f64 = 1.0;

/** A stream's decoder, the configuration it was opened from, and how far it has been fed. */
#[derive(Default)]
pub struct StreamVideo {
    decoder: Option<Decoder>,
    /** The track configuration the decoder was opened from; a different one means the frames were replaced. */
    config: Option<Vec<u8>>,
    /** The decode time of the last sample handed to the decoder; None until a sync frame starts the run. */
    fed: Option<f64>,
}

impl StreamVideo {
    /** Forget the decoder and the feed position, as a seek does: the next feed starts again at a sync frame. */
    pub fn reset(&mut self) {
        self.decoder = None;
        self.fed = None;
    }

    /** Whether a decoder is open, so the player can report that its video is ready. */
    pub fn ready(&self) -> bool {
        self.decoder.is_some()
    }

    /** Drop the pictures before `cut`, after eviction removed the frames they came from. */
    pub fn forget_before(&self, cut: f64) {
        if let Some(decoder) = &self.decoder {
            decoder.forget_before(cut);
        }
    }

    /** One step: open the decoder once the track is known, feed it ahead of `playhead`, and return the picture due now. */
    pub fn step(&mut self, stream: &Stream, playhead: f64) -> Option<Retained<NSImage>> {
        let lane = stream
            .lanes()
            .find(|lane| lane.kind() == TrackKind::Video)?;
        if self.config.as_deref() != Some(lane.track.config.as_slice()) {
            self.reset();
            self.config = Some(lane.track.config.clone());
        }
        if self.decoder.is_none() {
            let config = parse_avcc(&lane.track.config).ok()?;
            self.decoder = Decoder::new(&config).ok();
        }
        let decoder = self.decoder.as_mut()?;
        feed(
            decoder,
            &lane.frames,
            &mut self.fed,
            playhead + LOOKAHEAD,
            MAX_PENDING_PICTURES,
        );
        decoder.picture_at(playhead, LOOKAHEAD)
    }
}

/**
 * Hand the decoder every frame after the last fed one, up to `until`, stopping while it holds `cap` pictures; the first
 * frame fed is a sync frame.
 */
pub fn feed(
    decoder: &mut Decoder,
    frames: &VecDeque<Frame>,
    fed: &mut Option<f64>,
    until: f64,
    cap: usize,
) {
    let start = match *fed {
        Some(last) => frames.partition_point(|frame| frame.decode <= last),
        None => match frames.iter().position(|frame| frame.sync) {
            Some(index) => index,
            None => return,
        },
    };
    for frame in frames.range(start..) {
        if frame.decode > until || decoder.pending() >= cap {
            break;
        }
        if decoder.decode(frame).is_ok() {
            *fed = Some(frame.decode);
        }
    }
}

/** The first video lane of a stream, for the size the painter lays the element out at. */
pub fn video_lane(stream: &Stream) -> Option<&Lane> {
    stream.lanes().find(|lane| lane.kind() == TrackKind::Video)
}

#[cfg(test)]
mod tests {
    /**
     * Feeding against the real decoder and the fixture tests/fixtures/tiny-h264.mp4 (see the decoder's tests for how
     * it was made).
     */
    use super::{Decoder, feed};
    use crate::runtime::media::avc::parse_avcc;
    use crate::runtime::media::stream::{Append, Stream};

    const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/tiny-h264.mp4");

    /** The fixture's video lane, appended whole into a stream. */
    fn fixture_stream() -> Stream {
        let boxes = mars_mp4::complete_boxes(FIXTURE).unwrap();
        let init_len = boxes
            .iter()
            .find(|span| &span.kind == b"moof")
            .unwrap()
            .start;
        let mut stream = Stream::default();
        stream
            .append(
                1,
                Append {
                    bytes: FIXTURE,
                    init_len,
                    offset: 0.0,
                    window: (0.0, None),
                },
            )
            .unwrap();
        stream
    }

    /** Feeding stops once the decoder holds its cap of pictures, and resumes only when the queue drains. */
    #[test]
    fn feeding_stops_at_the_picture_cap() {
        let stream = fixture_stream();
        let lane = stream
            .lanes()
            .find(|lane| lane.kind() == mars_mp4::TrackKind::Video)
            .unwrap();
        let mut decoder = Decoder::new(&parse_avcc(&lane.track.config).unwrap()).unwrap();
        let mut fed = None;
        feed(&mut decoder, &lane.frames, &mut fed, f64::INFINITY, 3);
        assert_eq!(decoder.pending(), 3);
        assert!(fed.unwrap() < lane.frames.back().unwrap().decode);
        decoder.clear();
        feed(&mut decoder, &lane.frames, &mut fed, f64::INFINITY, 3);
        assert_eq!(decoder.pending(), 3);
    }
}
