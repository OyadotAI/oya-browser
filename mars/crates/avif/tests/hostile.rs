/** Hostile input through the public API: seeded byte mutations of every fixture must never panic, and the sizes a file can claim (a 65535-square header, a clean aperture that crops to nothing) are refused with a typed error before any picture buffer is built. */
use mars_avif::{Error, decode, dimensions};
use std::panic::{self, AssertUnwindSafe};

/** Every fixture the crate ships: 8-bit 4:2:0, 4:4:4, clap-cropped, monochrome and a still picture. */
const FIXTURES: &[&[u8]] = &[
    include_bytes!("fixtures/airbnb-240.avif"),
    include_bytes!("fixtures/chimera-8bit.avif"),
    include_bytes!("fixtures/mexico-yuv444.avif"),
    include_bytes!("fixtures/monochrome.avif"),
    include_bytes!("fixtures/still-picture.avif"),
];

/** Mutated copies made per fixture. The 4:4:4 fixture is the largest picture, so it gets fewer cases to keep a debug run practical. */
const CASES_PER_FIXTURE: [u64; 5] = [800, 800, 400, 800, 800];
/** Bytes at the front of a file, where the container and the sequence and frame headers sit. Half the mutations land here, so header fields are fuzzed as hard as tile data. */
const HEADER_WINDOW: usize = 512;
/** Most bytes one mutated copy changes. */
const MAX_FLIPS: u64 = 4;
/** Seed of the xorshift generator, so every run mutates the same bytes. */
const SEED: u64 = 0x9e37_79b9_7f4a_7c15;
/** Bytes in a FullBox header (type is excluded): version and 24 flag bits. */
const FULLBOX_HEADER_LEN: usize = 4;
/** Bytes in a box type, the four characters after the size. */
const BOX_TYPE_LEN: usize = 4;
/** Bytes in one unsigned 32-bit field. */
const U32_LEN: usize = 4;
/** Side length a hostile ispe claims: the largest a 16-bit field can carry. */
const HOSTILE_SIDE: u32 = 0xffff;

/** A small deterministic generator (xorshift64*), so the corpus is reproducible without a dependency. */
struct Rng(u64);

impl Rng {
    /** The next value of the sequence. */
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /** A value in `0..bound`, for a bound that is not zero. */
    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

/** A copy of `fixture` with a few bytes changed, and sometimes cut short, chosen by `rng`. */
fn mutate(fixture: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut file = fixture.to_vec();
    let flips = 1 + rng.below(MAX_FLIPS);
    let window = file.len().min(HEADER_WINDOW) as u64;
    for _ in 0..flips {
        let span = if rng.below(2) == 0 {
            window
        } else {
            file.len() as u64
        };
        let at = rng.below(span) as usize;
        file[at] ^= (rng.next() as u8) | 1;
    }
    if rng.below(8) == 0 {
        let keep = rng.below(file.len() as u64) as usize;
        file.truncate(keep);
    }
    file
}

/** Run both entry points on `bytes` and report whether either panicked. */
fn panics_on(bytes: &[u8]) -> bool {
    panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = dimensions(bytes);
        let _ = decode(bytes);
    }))
    .is_err()
}

/** Every fixture, mutated by seeded byte changes and truncations, decodes or is refused without a panic. The failing case is named by fixture and index so a regression reproduces. */
#[test]
fn mutated_fixtures_never_panic() {
    let mut rng = Rng(SEED);
    let mut failures = Vec::new();
    for (index, fixture) in FIXTURES.iter().enumerate() {
        for case in 0..CASES_PER_FIXTURE[index] {
            let file = mutate(fixture, &mut rng);
            if panics_on(&file) {
                failures.push(format!(
                    "fixture {index}, case {case}, {} bytes",
                    file.len()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "panicked on: {failures:?}");
}

/** The offset of the first body byte of the box `kind` in `file`, past its type. */
fn body_of(file: &[u8], kind: &[u8; 4]) -> usize {
    let at = file
        .windows(kind.len())
        .position(|w| w == kind)
        .expect("the fixture carries the box");
    at + BOX_TYPE_LEN
}

/** Overwrite a big-endian u32 at `at`. */
fn put_u32(file: &mut [u8], at: usize, value: u32) {
    file[at..at + U32_LEN].copy_from_slice(&value.to_be_bytes());
}

/** A header that claims a 65535 by 65535 picture is refused as a limit before any buffer is sized from it. */
#[test]
fn a_header_claiming_a_huge_picture_is_refused_before_allocation() {
    let mut file = FIXTURES[0].to_vec();
    let ispe = body_of(&file, b"ispe") + FULLBOX_HEADER_LEN;
    put_u32(&mut file, ispe, HOSTILE_SIDE);
    put_u32(&mut file, ispe + U32_LEN, HOSTILE_SIDE);
    assert_eq!(decode(&file), Err(Error::Limit("picture side")));
    assert!(dimensions(&file).is_err());
}

/** A picture within the side limit but over the area limit is refused as a limit too. */
#[test]
fn a_header_claiming_too_many_pixels_is_refused() {
    let mut file = FIXTURES[0].to_vec();
    let ispe = body_of(&file, b"ispe") + FULLBOX_HEADER_LEN;
    put_u32(&mut file, ispe, 16384);
    put_u32(&mut file, ispe + U32_LEN, 16384);
    assert_eq!(decode(&file), Err(Error::Limit("picture area")));
}

/** A clean aperture whose width or height is zero crops to nothing; it is Malformed, not an empty picture or an underflow. The chimera clap box carries a 480-wide crop, and each side is zeroed in turn. */
#[test]
fn a_clean_aperture_that_crops_to_nothing_is_malformed() {
    const CLAP_WIDTH: usize = 0;
    const CLAP_HEIGHT: usize = 2 * U32_LEN;
    for side in [CLAP_WIDTH, CLAP_HEIGHT] {
        let mut file = FIXTURES[1].to_vec();
        let clap = body_of(&file, b"clap");
        put_u32(&mut file, clap + side, 0);
        assert_eq!(decode(&file), Err(Error::Malformed("cropped size")));
        assert_eq!(dimensions(&file), Err(Error::Malformed("cropped size")));
    }
}
