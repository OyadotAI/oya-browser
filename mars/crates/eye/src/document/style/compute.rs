/** Winning declarations become computed values: `var()`, CSS-wide keywords, then dependency order. */
use super::{
    ComputedStyle,
    cascade::Chosen,
    declaration::{Drop, dropped, wide},
    hiding::hiding,
    vars::Scopes,
};
use crate::{
    document::{Appearance, RenderContext},
    protocol::EngineResult,
};
/** Properties other values depend on, computed first in this order. The font properties come
 * first, as in Blink's high-priority set, so `ch` and `ic` below see the element's own face. */
const FIRST: [&str; 7] = [
    "font-family",
    "font-style",
    "font-size",
    "font-weight",
    "line-height",
    "color-scheme",
    "color",
];
/** Corner radii resolve after width so percentages can use it. */
const LAST: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];
/** One element's resolution context. */
pub(super) struct Resolver<'a> {
    /// Custom property scopes.
    pub scopes: &'a Scopes,
    /// This element's scope.
    pub scope: usize,
    /// Viewport and appearance.
    pub context: RenderContext,
    /// Form controls default to visible border styles.
    pub control: bool,
    /// `@keyframes` rules running animations rest on.
    pub keyframes: &'a super::animation::Keyframes,
}
impl Resolver<'_> {
    /** Computes `style`; true when a value Eye dropped, or an animation it could not evaluate, could have hidden the element. */
    pub(super) fn resolve(
        &self,
        style: &mut ComputedStyle,
        parent: &ComputedStyle,
        mut chosen: Chosen,
        diagnostics: &mut Vec<String>,
    ) -> EngineResult<bool> {
        super::values::set_own_line(None);
        self.substitute(style, parent, &mut chosen);
        // Resting animation frames override the cascade; their own `var()`s substitute next.
        let mut hides = super::animation::animate(&mut chosen, self.keyframes, diagnostics);
        self.substitute(style, parent, &mut chosen);
        bottom_layer_clip(&mut chosen);
        let rtl = super::logical::rtl(&chosen, style.rtl);
        let (vertical, lr) =
            super::logical::writing(&chosen, (style.vertical_writing, style.writing_lr));
        let mode = super::logical::Mode { rtl, vertical, lr };
        super::logical::resolve(&mut chosen, mode);
        // Border longhands under a keyword are copied after the border sides resolve below.
        let mut border_keywords = Vec::new();
        chosen.retain(|key, (_, value)| {
            if !wide(value) {
                return true;
            }
            if super::border::longhand(key).is_some() {
                border_keywords.push((key.clone(), value.clone()));
            } else {
                super::resolve::keyword(style, parent, key, value);
            }
            false
        });
        // Font-relative units in the font properties themselves resolve against the parent face.
        // Picking a face costs a font match, so only elements that name `ch` or `ic` pay it.
        let measured = chosen.values().any(|(_, value)| font_unit(value));
        super::values::set_font_face(measured.then(|| super::values::style_face(parent)));
        for key in FIRST {
            if let Some((_, value)) = chosen.remove(key) {
                hides |= self.apply(style, key, &value, diagnostics);
            }
            if key == "font-size" {
                super::font::generic_change(style, parent);
            }
            if key == "line-height" {
                // `lh` in every later property is this element's own line height.
                super::values::set_own_line(Some(crate::document::render::line_advance(style)));
            }
            if key == "font-weight" && measured {
                super::values::set_font_face(Some(super::values::style_face(style)));
            }
            if key == "color-scheme" {
                style.used_scheme =
                    super::resolve::used_scheme(style.color_scheme, self.context.appearance);
            }
        }
        let values: std::collections::BTreeMap<String, String> = chosen
            .iter()
            .map(|(key, (_, value))| (key.clone(), value.to_string()))
            .collect();
        let mut values = values;
        let sides = super::border::take(&mut values);
        super::border::resolve(style, &sides, self.context, self.control);
        for (key, value) in border_keywords {
            super::resolve::keyword(style, parent, &key, &value);
        }
        let late: Vec<(String, String)> = LAST
            .iter()
            .filter_map(|key| values.remove(*key).map(|value| (key.to_string(), value)))
            .collect();
        for (key, value) in values.into_iter().chain(late) {
            hides |= self.apply(style, &key, &value, diagnostics);
        }
        super::values::set_font_face(None);
        default_foreground(style);
        super::blockify::blockify_table_part(style);
        writing_adjust(style, parent);
        contents_box(style);
        super::apply::line_clamp_box(style);
        style.validate()?;
        Ok(hides)
    }
    /** Replaces `var()` references; invalid ones become `unset`, pending shorthands expand. */
    fn substitute(&self, style: &mut ComputedStyle, parent: &ComputedStyle, chosen: &mut Chosen) {
        let pending: Vec<String> = chosen
            .iter()
            .filter(|(_, (_, value))| value.to_ascii_lowercase().contains("var("))
            .map(|(key, _)| key.clone())
            .collect();
        for key in pending {
            let Some((priority, value)) = chosen.remove(&key) else {
                continue;
            };
            let Some(value) = self.scopes.substitute(self.scope, &value) else {
                super::resolve::keyword(style, parent, &key, "unset");
                continue;
            };
            if super::shorthand::longhands(&key).is_none() {
                chosen.insert(key, (priority, value.into()));
                continue;
            }
            for (name, value) in super::shorthand::expand(&key, &value).unwrap_or_default() {
                if chosen
                    .get(&name)
                    .is_none_or(|(existing, _)| priority >= *existing)
                {
                    chosen.insert(name, (priority, value.into()));
                }
            }
        }
    }
    /** Applies one value; a failure is dropped with a diagnostic and reports whether it could hide. */
    fn apply(
        &self,
        style: &mut ComputedStyle,
        key: &str,
        value: &str,
        diagnostics: &mut Vec<String>,
    ) -> bool {
        let value = &super::values::integer_math(key, value);
        let drop = match super::apply::apply(style, key, value, self.context, diagnostics) {
            Ok(true) => return false,
            Ok(false) => Drop::Property,
            Err(_) => Drop::Value,
        };
        dropped(key, drop, diagnostics);
        hiding(key, value)
    }
}
/** Whether a value may use `ch`, `ic`, `ex` or `cap`, the units that measure the element's
 * face; `ex` and `cap` count only after a number, so `flex` or `text` does not. */
fn font_unit(value: &str) -> bool {
    let bytes = value.as_bytes();
    let after_number =
        |at: usize| at > 0 && (bytes[at - 1].is_ascii_digit() || bytes[at - 1] == b'.');
    bytes.windows(2).enumerate().any(|(at, pair)| {
        pair.eq_ignore_ascii_case(b"ch")
            || pair.eq_ignore_ascii_case(b"ic")
            || (pair.eq_ignore_ascii_case(b"ex") && after_number(at))
            || (pair.eq_ignore_ascii_case(b"ca") && after_number(at))
    })
}
/** A painted background without an authored color gets the scheme's default text color. */
fn default_foreground(style: &mut ComputedStyle) {
    if style.color.is_none()
        && style
            .background
            .as_deref()
            .is_some_and(|color| color != "transparent")
    {
        style.color = Some(
            match style.used_scheme {
                Appearance::Light => "#000000",
                Appearance::Dark => "#ffffff",
            }
            .into(),
        );
    }
}

/** Blink's StyleAdjuster for `writing-mode`: rows, row groups, columns and column groups take
their parent's (the property does not apply to them, so a table's progression ignores theirs),
and an inline in a different writing mode than its parent becomes an inline-block, the
orthogonal flow it must be. */
fn writing_adjust(style: &mut ComputedStyle, parent: &ComputedStyle) {
    use super::TableBox;
    let part = matches!(
        style.table_box,
        TableBox::Row
            | TableBox::RowGroup
            | TableBox::HeaderGroup
            | TableBox::FooterGroup
            | TableBox::Column
    );
    if part {
        style.vertical_writing = parent.vertical_writing;
        style.writing_lr = parent.writing_lr;
        style.writing_sideways = parent.writing_sideways;
    }
    let differs = (
        style.vertical_writing,
        style.writing_lr,
        style.writing_sideways,
    ) != (
        parent.vertical_writing,
        parent.writing_lr,
        parent.writing_sideways,
    );
    if style.inline && !style.inline_block && style.table_box == TableBox::None && differs {
        style.inline_block = true;
    }
}

/** css-display-3 `display: contents`: the element generates no box, so none of its own box
properties (margin, border, padding, background, size, offsets) apply. Eye lays out a contents
element that holds text as an inline wrapper around it, so it keeps only what it passes down:
its inherited values and its generated content. A border on it painted red around its text. */
fn contents_box(style: &mut ComputedStyle) {
    if !style.display_contents {
        return;
    }
    let mut bare = super::resolve::inherited(style);
    bare.generated = std::mem::take(&mut style.generated);
    bare.inline = style.inline;
    bare.display_contents = true;
    *style = bare;
}

/** CSS Backgrounds: the layer count is `background-image`'s, a longer `background-clip` list is
truncated and a shorter one repeats, and the color takes the bottom (last) layer's clip. The
clip is narrowed to that one value here, where both declarations are known. */
fn bottom_layer_clip(chosen: &mut Chosen) {
    let layers = chosen.get("background-image").map_or(1, |(_, value)| {
        super::syntax::split_top(value, b',').len().max(1)
    });
    for key in ["background-clip", "-webkit-background-clip"] {
        let Some((priority, value)) = chosen.get(key).cloned() else {
            continue;
        };
        let clips = super::syntax::split_top(&value, b',');
        if clips.len() > 1 {
            let bottom = clips[(layers - 1) % clips.len()].trim().to_string();
            chosen.insert(key.into(), (priority, bottom.into()));
        }
    }
}
