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
/** Properties other values depend on, computed first in this order. */
const FIRST: [&str; 4] = ["font-size", "color-scheme", "font-weight", "color"];
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
        self.substitute(style, parent, &mut chosen);
        // Resting animation frames override the cascade; their own `var()`s substitute next.
        let mut hides = super::animation::animate(&mut chosen, self.keyframes, diagnostics);
        self.substitute(style, parent, &mut chosen);
        let rtl = super::logical::rtl(&chosen, style.rtl);
        super::logical::resolve(&mut chosen, rtl);
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
        for key in FIRST {
            if let Some((_, value)) = chosen.remove(key) {
                hides |= self.apply(style, key, &value, diagnostics);
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
        default_foreground(style);
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
        let drop = match super::apply::apply(style, key, value, self.context, diagnostics) {
            Ok(true) => return false,
            Ok(false) => Drop::Property,
            Err(_) => Drop::Value,
        };
        dropped(key, drop, diagnostics);
        hiding(key, value)
    }
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
