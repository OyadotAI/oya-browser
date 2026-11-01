/**
 * The H.264 decoder configuration an avcC record carries: the NAL unit length size that frames each sample's NAL
 * units, and the sequence and picture parameter sets a decoder needs before its first sample. Parsed here, where it
 * is platform-neutral and tested, and handed to the native decoder that builds the format description from it.
 */

/// The record's fixed header: version, profile, compatibility, level, the length-size byte and the SPS-count byte.
const FIXED: usize = 6;
/// The record version this parser reads; any other is refused rather than guessed at.
const VERSION: u8 = 1;
/// The low two bits of byte 4 hold the NAL length size minus one.
const LENGTH_MASK: u8 = 0b11;
/// The low five bits of the SPS-count byte hold the count; the top three are reserved.
const COUNT_MASK: u8 = 0b1_1111;
/// Every parameter set is prefixed by its length as a big-endian 16-bit number.
const SET_LENGTH: usize = 2;

/** One avcC record's configuration: the NAL length size and the parameter sets, each without its length prefix. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvcConfig {
    /** Bytes of each NAL unit's length prefix inside a sample (1 to 4). */
    pub nal_length_size: usize,
    /** The sequence parameter sets, in record order. */
    pub sps: Vec<Vec<u8>>,
    /** The picture parameter sets, in record order. */
    pub pps: Vec<Vec<u8>>,
}

/** Why an avcC record was refused. Each reason is a fixed description, never bytes from the record. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvcError {
    /** The record is cut short, or a parameter set runs past its end. */
    Truncated,
    /** The version is not 1, or the record names no sequence parameter set. */
    Unsupported,
}

/** Parse an avcC record: the header, then each SPS, then each PPS, every one length-prefixed. */
pub fn parse_avcc(record: &[u8]) -> Result<AvcConfig, AvcError> {
    if record.len() < FIXED || record[0] != VERSION {
        return Err(AvcError::Unsupported);
    }
    let nal_length_size = usize::from(record[4] & LENGTH_MASK) + 1;
    let mut rest = &record[FIXED..];
    let sps = sets(&mut rest, usize::from(record[5] & COUNT_MASK))?;
    let pps_count = usize::from(*rest.first().ok_or(AvcError::Truncated)?);
    rest = &rest[1..];
    let pps = sets(&mut rest, pps_count)?;
    if sps.is_empty() {
        return Err(AvcError::Unsupported);
    }
    Ok(AvcConfig {
        nal_length_size,
        sps,
        pps,
    })
}

/** `count` length-prefixed parameter sets read from the front of `rest`, which advances past them. */
fn sets(rest: &mut &[u8], count: usize) -> Result<Vec<Vec<u8>>, AvcError> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let length = rest.get(..SET_LENGTH).ok_or(AvcError::Truncated)?;
        let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
        let body = rest
            .get(SET_LENGTH..SET_LENGTH + length)
            .ok_or(AvcError::Truncated)?;
        out.push(body.to_vec());
        *rest = &rest[SET_LENGTH + length..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /** The avcC of tests/fixtures/tiny-h264.mp4, made by ffmpeg (baseline profile, 4-byte NAL lengths). */
    const FIXTURE_AVCC: &[u8] = &[
        0x01, 0x42, 0xc0, 0x0a, 0xff, 0xe1, 0x00, 0x17, 0x67, 0x42, 0xc0, 0x0a, 0xd9, 0x04, 0x7b,
        0x01, 0x10, 0x00, 0x00, 0x03, 0x00, 0x10, 0x00, 0x00, 0x03, 0x01, 0x40, 0xf1, 0x22, 0x64,
        0x80, 0x01, 0x00, 0x05, 0x68, 0xcb, 0x83, 0xcb, 0x20,
    ];

    /** The fixture's record yields its one SPS and one PPS, and a four-byte NAL length size. */
    #[test]
    fn avcc_yields_the_parameter_sets_and_length_size() {
        let config = super::parse_avcc(FIXTURE_AVCC).unwrap();
        assert_eq!(config.nal_length_size, 4);
        assert_eq!(config.sps.len(), 1);
        assert_eq!(config.sps[0][0], 0x67);
        assert_eq!(config.pps, vec![vec![0x68, 0xcb, 0x83, 0xcb, 0x20]]);
    }

    /** A record cut inside its PPS is refused as truncated, never sliced past its end. */
    #[test]
    fn avcc_refuses_a_parameter_set_past_the_record() {
        let cut = &FIXTURE_AVCC[..FIXTURE_AVCC.len() - 2];
        assert_eq!(super::parse_avcc(cut), Err(super::AvcError::Truncated));
    }

    /** A record of another version, or one with no sequence parameter set, is unsupported. */
    #[test]
    fn avcc_refuses_another_version_and_a_record_without_sps() {
        let mut version_two = FIXTURE_AVCC.to_vec();
        version_two[0] = 2;
        assert_eq!(super::parse_avcc(&version_two), Err(super::AvcError::Unsupported));
        let no_sps = [1, 0x42, 0xc0, 0x0a, 0xff, 0xe0, 0];
        assert_eq!(super::parse_avcc(&no_sps), Err(super::AvcError::Unsupported));
    }
}
