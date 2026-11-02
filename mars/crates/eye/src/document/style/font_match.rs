/**
 * Face selection per CSS Fonts 4 (5.2): the first family of a `font-family` list that has a
 * face covering the character, then the face nearest the wanted style and weight. The same
 * choice runs twice: over every `@font-face` of a page to list the files its text needs, and
 * over the files that arrived to shape each text run. The arrived faces sit in a per-thread
 * table the layout reads, installed from the document before each layout.
 */
use super::{
    ComputedStyle, FontFace, FontStyle,
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
    for (style, text) in styled_texts(document) {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        for index in family_faces(faces, style, text) {
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

/// The characters font-relative units measure (`ch` the zero, `ic` the CJK water ideograph):
/// an element without text of its own still needs the face that has them.
const UNIT_SAMPLE: &str = "0\u{6C34}";

/** Each visible node's style with its text (or UNIT_SAMPLE when it has none), then its
 * pseudo-elements' styles: `::before` and `::after` with their content, and `::first-letter`
 * and `::first-line` with the next text in document order (the text they style). Without them
 * a font named only by a `::first-letter` rule, or only measured by a `ch` width, was never
 * requested, and the letter or unit fell back to a built-in face. */
// ponytail: an empty element's sample can request a face no text paints; FONT_FILES bounds it.
fn styled_texts(document: &Document) -> Vec<(&ComputedStyle, &str)> {
    let visible: Vec<&crate::document::Node> = document
        .nodes
        .iter()
        .filter(|node| !node.hidden())
        .collect();
    let mut pairs = Vec::new();
    for (index, node) in visible.iter().enumerate() {
        let own = node.text.as_str();
        pairs.push((
            &*node.style,
            if own.trim().is_empty() {
                UNIT_SAMPLE
            } else {
                own
            },
        ));
        pseudo_texts(&visible[index..], &mut pairs);
    }
    pairs
}

/** Pushes the pseudo-element styles of `nodes[0]` with the text each styles (see
 * `styled_texts`); the following text is looked up only when a line or letter rule exists. */
fn pseudo_texts<'a>(
    nodes: &[&'a crate::document::Node],
    pairs: &mut Vec<(&'a ComputedStyle, &'a str)>,
) {
    let style = &nodes[0].style;
    let pseudo = [&style.first_letter, &style.first_line];
    if pseudo.iter().any(|rule| rule.is_some()) {
        let next = nodes
            .iter()
            .map(|n| n.text.as_str())
            .find(|t| !t.trim().is_empty());
        for rule in pseudo.into_iter().flatten() {
            pairs.push((&**rule, next.unwrap_or_default()));
        }
    }
    pairs.extend(
        style
            .generated
            .iter()
            .map(|g| (&*g.style, g.content.as_str())),
    );
}

/** The faces `text` in `style` needs, falling back along the family list as Blink does: each
 * listed family with faces takes the characters its faces cover, and the rest go on to the next
 * family, until a generic keyword takes them. Without the fallback a character outside the
 * first family's `unicode-range` (a CJK `ic` sample after a Latin-only face) was never
 * requested from the family that has it. */
fn family_faces(faces: &[FontFace], style: &ComputedStyle, text: &str) -> Vec<usize> {
    let mut rest: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = Vec::new();
    for family in &style.font_families {
        if rest.is_empty() || GENERIC.contains(&family.as_str()) {
            break;
        }
        let candidates: Vec<(usize, &FontFace)> = faces
            .iter()
            .enumerate()
            .filter(|(_, face)| &face.family == family)
            .collect();
        out.extend(chosen(
            &candidates,
            &rest,
            style.font_weight,
            style.font_style,
        ));
        rest.retain(|c| !candidates.iter().any(|(_, face)| face.covers(c)));
    }
    out
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
