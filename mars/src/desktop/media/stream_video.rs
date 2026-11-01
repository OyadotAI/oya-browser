/*!
 * The video of a Media Source player: the first video lane's samples are fed to a VideoToolbox decoder a little ahead of
 * the playhead, and the picture due at the playhead is handed to the painter. Feeding starts at a sync frame and
 * restarts after a seek, so the decoder never sees a frame whose references it does not hold.
 */
use super::decoder::Decoder;
use crate::runtime::media::avc::parse_avcc;
use crate::runtime::media::stream::{Lane, Stream};
use mars_mp4::TrackKind;
use objc2::rc::Retained;
use objc2_app_kit::NSImage;
use std::collections::VecDeque;

/// How far ahead of the playhead samples are sent to the decoder, in seconds.
const LOOKAHEAD: f64 = 1.0;

/** A stream's decoder and how far it has been fed. */
#[derive(Default)]
pub struct StreamVideo {
    decoder: Option<Decoder>,
    /** The decode time of the last sample handed to the decoder; None until a sync frame starts the run. */
    fed: Option<f64>,
}

impl StreamVideo {
    /** Forget the decoder and the feed position, as a seek does: the next feed starts again at a sync frame. */
    pub fn reset(&mut self) {
        self.decoder = None;
        self.fed = None;
    }

    /** One step: open the decoder once the track is known, feed it ahead of `playhead`, and return the picture due now. */
    pub fn step(&mut self, stream: &Stream, playhead: f64) -> Option<Retained<NSImage>> {
        let lane = stream
            .lanes()
            .find(|lane| lane.kind() == TrackKind::Video)?;
        if self.decoder.is_none() {
            let config = parse_avcc(&lane.track.config).ok()?;
            self.decoder = Decoder::new(&config).ok();
        }
        let decoder = self.decoder.as_mut()?;
        feed(decoder, &lane.frames, &mut self.fed, playhead + LOOKAHEAD);
        decoder.picture_at(playhead)
    }
}

/** Hand the decoder every frame after the last fed one, up to `until`; the first frame fed is a sync frame. */
fn feed(
    decoder: &mut Decoder,
    frames: &VecDeque<crate::runtime::media::stream::Frame>,
    fed: &mut Option<f64>,
    until: f64,
) {
    let start = match *fed {
        Some(last) => frames.partition_point(|frame| frame.decode <= last),
        None => match frames.iter().position(|frame| frame.sync) {
            Some(index) => index,
            None => return,
        },
    };
    for frame in frames.range(start..) {
        if frame.decode > until {
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
