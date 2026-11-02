/** Container rules through the public API: brands, handlers, item tables, properties, size limits, payload bounds and hostile input. */
mod support;

use mars_avif::{Error, decode, dimensions};
use support::*;

/** The real AVIF served by the Airbnb CDN, 7.7 KB, used as the golden container. */
const AIRBNB: &[u8] = include_bytes!("fixtures/airbnb-240.avif");

/** The av01 item id every synthetic file here declares. */
const ITEM: u16 = 1;

/** A file whose single av01 item has the given properties and links, and one iloc extent at `offset`/`length`. */
fn file_with(props: &[Vec<u8>], links: &[u8], offset: u32, length: u32) -> Vec<u8> {
    let header = ftyp(b"avif", &[b"mif1"]);
    let meta_box = |at: u32| {
        let entries = [loc(ITEM, 0, &[(at, length)])];
        let body = [
            hdlr(b"pict"),
            pitm(ITEM),
            iinf(&[infe(ITEM, b"av01")]),
            iloc(&entries),
            iprp(props, links),
        ];
        fb(b"meta", 0, 0, &body.concat())
    };
    let at = (header.len() + meta_box(0).len() + 8) as u32 + offset;
    [header, meta_box(at), bx(b"mdat", &[7; 4])].concat()
}

/** A file with the standard ispe and av1C properties and the given essential-free property slots. */
fn standard(width: u32, height: u32) -> Vec<u8> {
    let props = [ispe(width, height), av1c()];
    file_with(&props, &ipma(&[(ITEM, &[1, 2])]), 0, 4)
}

#[test]
fn the_airbnb_avif_reports_its_declared_size() {
    assert_eq!(dimensions(AIRBNB), Ok((240, 180)));
    let image = decode(AIRBNB).expect("the Airbnb picture decodes");
    assert_eq!((image.width, image.height), (240, 180));
    assert_eq!(image.rgba.len(), 240 * 180 * 4);
}

#[test]
fn a_synthetic_single_item_file_reports_its_size() {
    let file = single_item(64, 32, &[1, 2, 3]);
    assert_eq!(dimensions(&file), Ok((64, 32)));
    assert_eq!(decode(&file), Err(Error::Malformed("no sequence header")));
}

#[test]
fn a_quarter_turn_irot_swaps_the_output_sides() {
    let props = [ispe(64, 32), av1c(), bx(b"irot", &[1])];
    let file = file_with(&props, &ipma(&[(ITEM, &[1, 2, 3])]), 0, 4);
    assert_eq!(dimensions(&file), Ok((32, 64)));
}

#[test]
fn a_file_without_an_avif_brand_is_malformed() {
    let mut file = standard(8, 8);
    file[8..12].copy_from_slice(b"heic");
    file[16..20].copy_from_slice(b"heic");
    assert_eq!(dimensions(&file), Err(Error::Malformed("not an AVIF file")));
}

#[test]
fn an_avis_major_brand_is_accepted() {
    let mut file = standard(8, 8);
    file[8..12].copy_from_slice(b"avis");
    assert_eq!(dimensions(&file), Ok((8, 8)));
}

#[test]
fn a_meta_handler_other_than_pict_is_malformed() {
    let header = ftyp(b"avif", &[b"mif1"]);
    let meta_box = fb(
        b"meta",
        0,
        0,
        &[hdlr(b"vide"), pitm(ITEM), iinf(&[]), iloc(&[])].concat(),
    );
    let file = [header, meta_box].concat();
    assert_eq!(
        dimensions(&file),
        Err(Error::Malformed("handler is not pict"))
    );
}

#[test]
fn a_primary_item_that_is_not_av1_is_unsupported() {
    let mut file = standard(8, 8);
    let at = file.windows(4).position(|w| w == b"av01").unwrap();
    file[at..at + 4].copy_from_slice(b"hvc1");
    assert_eq!(
        dimensions(&file),
        Err(Error::Unsupported("primary item type"))
    );
    assert_eq!(decode(&file), Err(Error::Unsupported("primary item type")));
}

#[test]
fn an_essential_property_mars_does_not_understand_refuses_the_item() {
    let props = [ispe(8, 8), av1c(), bx(b"zzzz", &[0])];
    let file = file_with(&props, &ipma(&[(ITEM, &[1, 2, 0x83])]), 0, 4);
    assert_eq!(
        dimensions(&file),
        Err(Error::Unsupported("essential item property"))
    );
}

#[test]
fn a_non_essential_unknown_property_is_ignored() {
    let props = [ispe(8, 8), av1c(), bx(b"zzzz", &[0])];
    let file = file_with(&props, &ipma(&[(ITEM, &[1, 2, 3])]), 0, 4);
    assert_eq!(dimensions(&file), Ok((8, 8)));
}

#[test]
fn a_clean_aperture_crop_sets_the_output_size_and_decode_refuses_it() {
    // clap fractions: width 5/1, height 3/1, then offsets 0/1 and 0/1.
    let mut clap = Vec::new();
    for v in [5u32, 1, 3, 1, 0, 1, 0, 1] {
        clap.extend_from_slice(&v.to_be_bytes());
    }
    let props = [ispe(8, 8), av1c(), bx(b"clap", &clap)];
    let file = file_with(&props, &ipma(&[(ITEM, &[1, 2, 3])]), 0, 4);
    assert_eq!(dimensions(&file), Ok((5, 3)));
    assert!(
        decode(&file).is_err(),
        "decode must not return pixels for a crop"
    );
}

#[test]
fn a_clap_with_a_zero_denominator_is_malformed() {
    let props = [ispe(8, 8), av1c(), bx(b"clap", &[0; 32])];
    let file = file_with(&props, &ipma(&[(ITEM, &[1, 2, 3])]), 0, 4);
    assert_eq!(dimensions(&file), Err(Error::Malformed("clap denominator")));
}

#[test]
fn a_primary_without_ispe_is_malformed() {
    let file = file_with(&[av1c()], &ipma(&[(ITEM, &[1])]), 0, 4);
    assert_eq!(
        dimensions(&file),
        Err(Error::Malformed("primary item without ispe"))
    );
}

#[test]
fn a_zero_side_is_malformed() {
    assert_eq!(
        dimensions(&standard(0, 8)),
        Err(Error::Malformed("picture size"))
    );
}

#[test]
fn a_side_past_the_av1_level_limit_is_refused() {
    assert_eq!(
        dimensions(&standard(20_000, 1)),
        Err(Error::Limit("picture side"))
    );
}

#[test]
fn an_area_past_the_av1_level_limit_is_refused() {
    assert_eq!(
        dimensions(&standard(16_000, 16_000)),
        Err(Error::Limit("picture area"))
    );
}

#[test]
fn an_extent_past_the_end_of_the_file_is_truncated_not_unsupported() {
    let props = [ispe(8, 8), av1c()];
    let file = file_with(&props, &ipma(&[(ITEM, &[1, 2])]), 1_000_000, 16);
    assert_eq!(decode(&file), Err(Error::Truncated));
    assert_eq!(dimensions(&file), Ok((8, 8)));
}

#[test]
fn an_idat_location_reads_inside_meta_and_a_missing_idat_is_malformed() {
    let header = ftyp(b"avif", &[b"mif1"]);
    let props = [ispe(8, 8), av1c()];
    let links = ipma(&[(ITEM, &[1, 2])]);
    let entries = [loc(ITEM, 1, &[(0, 2)])];
    let meta_body = [
        hdlr(b"pict"),
        pitm(ITEM),
        iinf(&[infe(ITEM, b"av01")]),
        iloc(&entries),
        iprp(&props, &links),
    ]
    .concat();
    let without_idat = [header.clone(), fb(b"meta", 0, 0, &meta_body)].concat();
    assert_eq!(decode(&without_idat), Err(Error::Malformed("idat missing")));

    let with_idat = [
        header,
        fb(b"meta", 0, 0, &[meta_body, bx(b"idat", &[9, 9])].concat()),
    ]
    .concat();
    assert!(decode(&with_idat).is_err());
}

#[test]
fn an_alpha_plane_linked_by_auxl_is_fetched_and_a_bad_alpha_extent_is_a_truncation() {
    let header = ftyp(b"avif", &[b"mif1"]);
    let auxc = fb(
        b"auxC",
        0,
        0,
        b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0",
    );
    let props = [ispe(8, 8), av1c(), auxc];
    let links = ipma(&[(ITEM, &[1, 2]), (2, &[1, 2, 3])]);
    let mut auxl_body = 2u16.to_be_bytes().to_vec();
    auxl_body.extend_from_slice(&1u16.to_be_bytes());
    auxl_body.extend_from_slice(&ITEM.to_be_bytes());
    let refs = fb(b"iref", 0, 0, &bx(b"auxl", &auxl_body));
    let meta_for = |offset: u32| {
        let entries = [loc(ITEM, 0, &[(offset, 1)]), loc(2, 0, &[(offset + 1, 50)])];
        let body = [
            hdlr(b"pict"),
            pitm(ITEM),
            iinf(&[infe(ITEM, b"av01"), infe(2, b"av01")]),
            iloc(&entries),
            refs.clone(),
            iprp(&props, &links),
        ];
        fb(b"meta", 0, 0, &body.concat())
    };
    let offset = (header.len() + meta_for(0).len() + 8) as u32;
    let file = [header, meta_for(offset), bx(b"mdat", &[1, 2])].concat();
    assert_eq!(decode(&file), Err(Error::Truncated));
}

#[test]
fn a_box_claiming_more_bytes_than_the_input_is_malformed() {
    let mut file = standard(8, 8);
    file[0..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(dimensions(&file), Err(Error::Malformed("box size")));
}

#[test]
fn a_box_smaller_than_its_own_header_is_malformed() {
    let file = [0u8, 0, 0, 4, b'f', b'r', b'e', b'e'];
    assert_eq!(dimensions(&file), Err(Error::Malformed("box size")));
}

#[test]
fn a_flood_of_top_level_boxes_hits_the_box_count_limit() {
    let mut file = Vec::new();
    for _ in 0..=4096 {
        file.extend_from_slice(&bx(b"free", &[]));
    }
    assert_eq!(dimensions(&file), Err(Error::Limit("box count")));
}

#[test]
fn an_iloc_field_width_outside_the_spec_is_malformed() {
    let mut file = standard(8, 8);
    let at = file.windows(4).position(|w| w == b"iloc").unwrap();
    file[at + 8] = 0x33;
    assert_eq!(dimensions(&file), Err(Error::Malformed("iloc field width")));
}

#[test]
fn every_truncation_of_the_airbnb_file_is_an_error_and_never_a_panic() {
    for len in 0..AIRBNB.len() {
        let cut = &AIRBNB[..len];
        assert!(decode(cut).is_err(), "decode accepted a {len}-byte prefix");
    }
}

#[test]
fn corrupting_bytes_of_the_airbnb_file_never_panics() {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..4000 {
        let mut file = AIRBNB.to_vec();
        for _ in 0..3 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let at = (state >> 33) as usize % file.len();
            file[at] ^= (state >> 17) as u8 | 1;
        }
        let _ = dimensions(&file);
        let _ = decode(&file);
    }
}
