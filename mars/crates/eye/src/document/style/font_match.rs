/**
 * Face selection per CSS Fonts 4 (5.2): the first family of a `font-family` list that has a
 * face covering the character, then the face nearest the wanted style and weight. The same
 * choice runs twice: over every `@font-face` of a page to list the files its text needs, and
 * over the files that arrived to shape each text run. The arrived faces sit in a per-thread
 * table the layout reads, installed from the document before each layout.
 */
use super::{
    FontFace, FontStyle,
    font_face::{FONT_FILES, LoadedFont, content_key},
    formatting::DocumentFormatting,
};
use crate::{document::Document, platform};
use std::{cell::RefCell, collections::BTreeMap};

/** One font file a page's text needs. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontRequest {
    /// Index into the document's `font_faces`.
    pub face: usize,
    /// Absolute address of the first fetchable source.
    pub url: String,
}

/** Keys of local fonts are tagged so a file and a name never collide. */
const LOCAL_TAG: u64 = 0x4c4f_4341_4c00_0000;
/** Keys of installed families are tagged apart from `local()` names. */
const FAMILY_TAG: u64 = 0x4641_4d49_4c59_0000;
/** Installed macOS families pages commonly name, which Chromium shapes with the real font when
 * no web font comes first. Others fall to the generic family the list ends with. */
const INSTALLED: [&str; 26] = [
    "arial",
    "arial black",
    "avenir",
    "avenir next",
    "baskerville",
    "comic sans ms",
    "courier new",
    "didot",
    "futura",
    "geneva",
    "georgia",
    "gill sans",
    "helvetica neue",
    "impact",
    "lucida grande",
    "menlo",
    "monaco",
    "optima",
    "palatino",
    "sf mono",
    "tahoma",
    "times new roman",
    "trebuchet ms",
    "verdana",
    "american typewriter",
    "charter",
];
/** Generic and system keywords, where family matching stops and the generic face applies. */
const GENERIC: [&str; 12] = [
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "-apple-system",
    "blinkmacsystemfont",
    "ui-sans-serif",
    "ui-serif",
    "ui-monospace",
    "emoji",
];

/// The files the document's visible text needs, in document order and within FONT_FILES: for
/// each text node the first listed family that has faces, then per character the face nearest
/// the node's style and weight among those covering it. A face whose `local()` font is
/// installed is used from there and not requested. Faces already loaded are not listed again.
pub fn font_requests(document: &Document) -> Vec<FontRequest> {
    let faces = &document.formatting.font_faces;
    let loaded: Vec<usize> = document
        .formatting
        .fonts
        .iter()
        .map(|font| font.face)
        .collect();
    let mut wanted: Vec<usize> = Vec::new();
    for node in document.nodes.iter().filter(|node| !node.hidden()) {
        let text = node.text.trim();
        if text.is_empty() {
            continue;
        }
        let Some(family) = first_family(&node.style.font_families, faces.iter().map(|f| &f.family))
        else {
            continue;
        };
        let candidates: Vec<(usize, &FontFace)> = faces
            .iter()
            .enumerate()
            .filter(|(_, face)| face.family == family)
            .collect();
        for index in chosen(
            &candidates,
            text,
            node.style.font_weight,
            node.style.font_style,
        ) {
            if !wanted.contains(&index) && !loaded.contains(&index) {
                wanted.push(index);
            }
        }
        if wanted.len() >= FONT_FILES {
            break;
        }
    }
    wanted
        .into_iter()
        .filter(|index| !faces[*index].locals().any(platform::local_font_exists))
        .filter_map(|index| {
            Some(FontRequest {
                face: index,
                url: faces[index].fetchable()?.to_owned(),
            })
        })
        .take(FONT_FILES)
        .collect()
}

/** The first family of a list that some face names. */
fn first_family<'a>(
    families: &'a [String],
    named: impl Iterator<Item = &'a String> + Clone,
) -> Option<&'a str> {
    families
        .iter()
        .find(|family| named.clone().any(|name| name == *family))
        .map(String::as_str)
}

/** The faces one text needs: one per distinct character when the family is split by
`unicode-range`, otherwise the single best face. */
fn chosen<T: Copy + PartialEq>(
    candidates: &[(T, &FontFace)],
    text: &str,
    weight: u16,
    style: FontStyle,
) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    let mut push = |pick: Option<T>| {
        if let Some(pick) = pick
            && !out.contains(&pick)
        {
            out.push(pick);
        }
    };
    if candidates.iter().all(|(_, face)| face.ranges.is_empty()) {
        push(best(candidates.iter().copied(), weight, style));
        return out;
    }
    let mut seen = std::collections::BTreeSet::new();
    for c in text.chars().filter(|c| !c.is_whitespace()) {
        if seen.insert(c) {
            let covering = candidates
                .iter()
                .copied()
                .filter(|(_, face)| face.covers(c));
            push(best(covering, weight, style));
        }
    }
    out
}

/** The face nearest the wanted style, then weight (CSS Fonts 4, 5.2.4 and 5.2.5). */
fn best<'a, T>(
    candidates: impl Iterator<Item = (T, &'a FontFace)>,
    weight: u16,
    style: FontStyle,
) -> Option<T> {
    candidates
        .map(|(id, face)| {
            (
                (
                    style_rank(face.style, style),
                    weight_rank(face.weight, weight),
                ),
                id,
            )
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}
/** Italic wants italic, then oblique; normal wants normal, then oblique; oblique wants oblique, then italic. */
fn style_rank(face: FontStyle, wanted: FontStyle) -> u8 {
    use FontStyle::*;
    match (wanted, face) {
        (a, b) if a == b => 0,
        (Italic, Oblique) | (Normal, Oblique) | (Oblique, Italic) => 1,
        _ => 2,
    }
}
/** (tier, distance): an exact or in-range weight first; then the direction CSS prefers for the
wanted weight: 400..=500 look up to 500 before down, light weights look down, bold weights up. */
fn weight_rank((low, high): (u16, u16), wanted: u16) -> (u8, u16) {
    let face = wanted.clamp(low, high);
    if face == wanted {
        return (0, 0);
    }
    let distance = face.abs_diff(wanted);
    let tier = match wanted {
        400..=500 if face > wanted && face <= 500 => 1,
        400..=500 if face < wanted => 2,
        400..=500 => 3,
        ..400 if face < wanted => 1,
        ..400 => 2,
        _ if face > wanted => 1,
        _ => 2,
    };
    (tier, distance)
}

/** The faces of one family that this thread can shape with, each with its registry key. */
#[derive(Default)]
struct Table {
    /// Loaded faces by family name.
    families: BTreeMap<String, Vec<(u64, FontFace)>>,
    /// Installed families this thread registered, by lowercase name.
    installed: BTreeMap<&'static str, u64>,
}
thread_local! {
    /** The current document's loaded faces; replaced by every `install_fonts`. */
    static TABLE: RefCell<Table> = RefCell::new(Table::default());
}

/// Makes a document's fonts available to this thread: each loaded file is handed to the
/// platform once per key, each installed `local()` font is resolved by name, and the faces
/// that resolved become the matching table layout and painting read. Cheap when nothing
/// changed: a key the platform already holds is not decoded again.
pub fn install_fonts(formatting: &DocumentFormatting) {
    let mut table = Table::default();
    for font in &formatting.fonts {
        if let Some(face) = formatting.font_faces.get(font.face)
            && registered(font, face)
        {
            table
                .families
                .entry(face.family.clone())
                .or_default()
                .push((font.key, face.clone()));
        }
    }
    for face in &formatting.font_faces {
        let local = face.locals().find_map(|name| {
            let key = content_key(name.as_bytes()) ^ LOCAL_TAG;
            (platform::has_font(key) || platform::register_local(key, name)).then_some(key)
        });
        if let Some(key) = local {
            table
                .families
                .entry(face.family.clone())
                .or_default()
                .push((key, face.clone()));
        }
    }
    for name in INSTALLED {
        let key = content_key(name.as_bytes()) ^ FAMILY_TAG;
        if platform::has_font(key) || platform::register_family(key, name) {
            table.installed.insert(name, key);
        }
    }
    TABLE.with(|current| *current.borrow_mut() = table);
}
/** Whether the platform holds the font, registering it from its bytes on first sight. An entry
whose data an earlier entry with the same key carried is already registered by then. */
fn registered(font: &LoadedFont, _face: &FontFace) -> bool {
    platform::has_font(font.key)
        || font
            .bytes()
            .is_some_and(|bytes| platform::register_font(font.key, &bytes))
}

/// The registry key of the loaded face a text run with these `font-family` names, weight and
/// style shapes with, and the weight to shape at: the wanted weight clamped to the face's
/// declared range, which a variable file's `wght` axis takes (Blink does the same, so one file
/// declared for 500 and again for 700 renders both). The family is the first listed one with
/// loaded faces covering `sample` (the run's first letter); None falls back to the generic.
pub(crate) fn select(
    families: &[String],
    weight: u16,
    style: FontStyle,
    sample: Option<char>,
) -> Option<(u64, u16)> {
    TABLE.with(|table| {
        let table = table.borrow();
        // Families in the page's order: a loaded web family, else an installed one, until a
        // generic keyword hands the run to the generic face.
        for family in families {
            if let Some(faces) = table.families.get(family) {
                let covering = faces
                    .iter()
                    .filter(|(_, face)| sample.is_none_or(|c| face.covers(c)))
                    .map(|(key, face)| ((*key, face.weight), face));
                if let Some((key, (low, high))) = best(covering, weight, style) {
                    return Some((key, weight.clamp(low, high)));
                }
            } else if GENERIC.contains(&family.as_str()) {
                return None;
            } else if let Some(key) = table.installed.get(family.as_str()) {
                return Some((*key, weight));
            }
        }
        None
    })
}
