/** Ordered UA, stylesheet and inline declarations select one bounded computed style per element. */
use super::{
    ComputedStyle, Position,
    constants::OFFSCREEN,
    declaration,
    media::Truth,
    note,
    parsed::Parsed,
    selectors::{Tree, matches, matches_compound, matches_wrapper, matches_wrapper_compound},
    sheet::{PseudoBox, Rule, Target},
    vars::{BODY, FIRST_NODE, HTML, Scopes},
};
use crate::{
    document::{Document, Node, RenderContext},
    protocol::EngineResult,
};
use std::{collections::BTreeMap, rc::Rc};
/** UA declarations and presentational hints, the two layers below author rules. */
type Defaults<'d> = (&'d [(&'d str, &'d str)], &'d [(&'d str, String)]);
/**
 * Cascade priority, after Blink's CascadePriority: importance, author origin, inline origin,
 * cascade layer, specificity, source order. The layer is its rank for normal declarations and
 * inverted for `!important` ones, so an earlier layer's important declaration wins.
 */
pub(super) type Priority = (bool, bool, bool, usize, usize, usize);
/** Each property's winning priority and value. */
pub(super) type Chosen = BTreeMap<String, (Priority, Rc<str>)>;
/// Required sheets must all parse within bounds before a candidate's actions become visible.
pub fn compute_styles(document: &Document, sheets: &[String]) -> EngineResult<Document> {
    compute_styles_with_context(document, sheets, RenderContext::default())
}
/** Viewport and appearance-dependent values are recomputed from retained source declarations. */
pub fn compute_styles_with_context(
    document: &Document,
    sheets: &[String],
    context: RenderContext,
) -> EngineResult<Document> {
    context.validate()?;
    cascade(document, sheets, context)
}
/// As `compute_styles_with_context` for a document embedded in a frame: viewport units and
/// media queries see the frame's content box, which `RenderContext::validate_frame` bounds.
pub fn compute_frame_styles(
    document: &Document,
    sheets: &[String],
    context: RenderContext,
) -> EngineResult<Document> {
    context.validate_frame()?;
    cascade(document, sheets, context)
}
/** The cascade under a context its caller already validated. */
fn cascade(
    document: &Document,
    sheets: &[String],
    context: RenderContext,
) -> EngineResult<Document> {
    crate::document::validate_styles(document)?;
    // Shadow trees style as the flat tree; the styles then return to the document's own nodes.
    let flat = crate::document::shadow::flat(document, true);
    let original = document;
    let document = flat.as_ref().map_or(document, |flat| &flat.document);
    super::values::set_root_font(crate::constants::CSS_ROOT_FONT_SIZE);
    super::values::set_root_line(0.0);
    let mut candidate = document.clone();
    let scoped = flat
        .as_ref()
        .map(|flat| flat.sheets.as_slice())
        .unwrap_or(&[]);
    let document_group = crate::document::shadow::DOCUMENT_GROUP;
    let (parsed, groups) = super::parsed::parsed(sheets, scoped, document_group, context)?;
    let mut cascade = Cascade {
        tree: Tree::new(document).with_shadow_hosts(original),
        groups,
        parsed,
        context,
        scopes: Scopes::default(),
        diagnostics: std::mem::take(&mut candidate.diagnostics),
        sizes: None,
        ancestry: Vec::new(),
        parent_scope: None,
        ua_margin_top: false,
    };
    cascade.scopes = Scopes::registered(cascade.parsed.properties.clone());
    for message in &cascade.parsed.diagnostics {
        note(&mut cascade.diagnostics, message.clone());
    }
    // The sheets' faces travel with the styled document; fonts already loaded for them stay.
    candidate.formatting.font_faces = cascade.parsed.font_faces.clone();
    // `ch` and `ic` measure this document's own loaded fonts, not the ones the last document
    // styled or laid out on this thread left installed.
    super::install_fonts(&candidate.formatting);
    super::values::take_measured();
    cascade.wrappers(document, &mut candidate)?;
    cascade.nodes(document, &mut candidate)?;
    // `@container` conditions and `cq*` units need laid-out sizes: one layout of these styles,
    // then one more cascade that reads them. Bounded to that single extra pass; nothing iterates
    // to a fixpoint.
    if cascade.parsed.container_rules
        && let Some(sizes) = super::container::sizes(&candidate, context)
    {
        cascade.sizes = Some(sizes);
        cascade.scopes = Scopes::registered(cascade.parsed.properties.clone());
        super::values::set_root_font(crate::constants::CSS_ROOT_FONT_SIZE);
        super::values::set_root_line(0.0);
        cascade.wrappers(document, &mut candidate)?;
        cascade.nodes(document, &mut candidate)?;
    }
    candidate.formatting.font_metrics = super::values::take_measured();
    if cascade.parsed.untrusted {
        untrusted(&mut candidate);
    }
    candidate.diagnostics = cascade.diagnostics;
    for note in crate::document::render::degrade::notes(&candidate) {
        super::note(&mut candidate.diagnostics, note.into());
    }
    Ok(match flat {
        Some(_) => crate::document::shadow::restore(original, candidate),
        None => candidate,
    })
}
/** One style pass's shared state. */
struct Cascade<'a> {
    /// Prebuilt matching index.
    tree: Tree<'a>,
    /// Each rule's shadow style group, when the document has shadow trees (empty otherwise).
    groups: Vec<usize>,
    /// The sheets' rules, buckets and keyframes, shared with the page's next cascade.
    parsed: std::rc::Rc<Parsed>,
    /// Viewport and appearance.
    context: RenderContext,
    /// Custom property scopes: html, body, then nodes in order.
    scopes: Scopes,
    /// Value-free diagnostics for the document.
    diagnostics: Vec<String>,
    /// Container content boxes from the first layout; None during the first pass.
    sizes: Option<super::container::Sizes>,
    /// Named and size containers above the element being styled, nearest first.
    ancestry: Vec<super::container::Ancestor>,
    /// The custom-property scope of the styled element's parent, which answers its unnamed
    /// style queries; None while the root wrappers are styled.
    parent_scope: Option<usize>,
    /// The element styled last took its top margin from the UA sheet, for the quirks-mode
    /// margin quirk.
    ua_margin_top: bool,
}
impl<'a> Cascade<'a> {
    /** Styles every element in document order; parents come before their children. */
    fn nodes(&mut self, document: &Document, candidate: &mut Document) -> EngineResult<()> {
        let positions: BTreeMap<usize, usize> = document
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.local_id, index))
            .collect();
        for index in 0..candidate.nodes.len() {
            let parent = document.nodes[index]
                .parent
                .and_then(|id| positions.get(&id).copied());
            if self.parsed.container_rules {
                self.ancestry = ancestry(candidate, &positions, parent);
                self.parent_scope = Some(parent.map_or(BODY, |index| index + FIRST_NODE));
                let extent = super::container::extent(&self.ancestry, self.sizes.as_ref());
                super::values::set_container_extent(extent);
            }
            let mut style = self.node(&document.nodes[index], parent, candidate)?;
            // Only an in-flow box's margin collapses into its container's top.
            let in_flow = !matches!(style.position, Position::Absolute | Position::Fixed)
                && style.float == super::Float::None;
            if self.ua_margin_top
                && in_flow
                && candidate.formatting.quirks
                && opens_quirk_container(candidate, index, parent)
            {
                style.quirk_top = true;
            }
            candidate.nodes[index].style = style;
        }
        self.ancestry.clear();
        self.parent_scope = None;
        super::values::set_container_extent([None, None]);
        Ok(())
    }
    /** Root and body are formatting subjects, never surrogate declarations on actual children. */
    fn wrappers(&mut self, document: &Document, candidate: &mut Document) -> EngineResult<()> {
        let formatting = &document.formatting;
        let (html, hides) = self.winning(
            "html",
            &formatting.html.attributes,
            &[],
            &super::hints::html(&formatting.html.attributes),
            &|target, tree| {
                Ok(match target {
                    Target::Selector(selector) => matches_wrapper(tree, "html", selector),
                    Target::Mask(compound) => matches_wrapper_compound(tree, "html", compound),
                })
            },
        )?;
        // The root's box frames the body (render/root_box.rs).
        let root = ComputedStyle::default();
        let mut style = root.clone();
        direction(&formatting.html.attributes, &mut style);
        self.resolve(&mut style, &root, html, HTML, false)?;
        style.interactive &= !hides;
        style.hidden |= formatting.html.attributes.contains_key("hidden");
        self.block_wrapper(&mut style, false);
        // ponytail: a vertical root's flow is the writing-modes work's; its generated boxes wait
        // for that root layout.
        if !style.vertical_writing {
            style.generated =
                self.wrapper_generated("html", &formatting.html.attributes, (&style, HTML))?;
        }
        candidate.formatting.html.style = style.clone();
        // `rem` below the root is relative to the root element's computed font size.
        super::values::set_root_font(style.font_size);
        super::values::set_root_line(crate::document::render::line_advance(&style));
        let (body, hides) = self.winning(
            "body",
            &formatting.body.attributes,
            &super::ua::body(),
            &super::hints::body(&formatting.body.attributes),
            &|target, tree| {
                Ok(match target {
                    Target::Selector(selector) => matches_wrapper(tree, "body", selector),
                    Target::Mask(compound) => matches_wrapper_compound(tree, "body", compound),
                })
            },
        )?;
        candidate.formatting.body_box_authored = true;
        let mut body_style = super::resolve::inherited(&style);
        direction(&formatting.body.attributes, &mut body_style);
        self.resolve(&mut body_style, &style, body, BODY, false)?;
        body_style.interactive &= !hides;
        body_style.hidden |= formatting.body.attributes.contains_key("hidden");
        self.block_wrapper(&mut body_style, true);
        body_style.generated =
            self.wrapper_generated("body", &formatting.body.attributes, (&body_style, BODY))?;
        candidate.formatting.body.style = body_style;
        Ok(())
    }
    /** Wrappers lay out as blocks; other display types are reported and ignored, except a flex
     * or grid body, which layout gives a container node of its own. */
    fn block_wrapper(&mut self, style: &mut ComputedStyle, body: bool) {
        if style.inline || (style.display_flex && !body) {
            note(
                &mut self.diagnostics,
                "Root and body display types other than block are not supported.".into(),
            );
            style.inline = false;
            style.display_flex = false;
        }
    }
    /** One element's computed style from its parent's. */
    fn node(
        &mut self,
        node: &Node,
        parent: Option<usize>,
        candidate: &Document,
    ) -> EngineResult<ComputedStyle> {
        let parent_style = parent
            .map(|index| &candidate.nodes[index].style)
            .unwrap_or(&candidate.formatting.body.style);
        let mut style = super::resolve::inherited(parent_style);
        super::ua::defaults(node, &mut style);
        direction(&node.attributes, &mut style);
        let ua = super::ua::declarations(node);
        let hints = super::hints::hints(node, candidate);
        let (mut chosen, hides) = self.winning(
            &node.tag,
            &node.attributes,
            &ua,
            &hints,
            &|target, tree| match target {
                Target::Selector(selector) => matches(tree, node, selector),
                Target::Mask(compound) => Ok(matches_compound(tree, node, compound)),
            },
        )?;
        super::attr::substitute(&mut chosen, &node.attributes);
        let default_link_color = node.href.is_some() && !chosen.contains_key("color");
        let scope_parent = parent.map_or(BODY, |index| index + FIRST_NODE);
        let control = ["input", "button", "select", "textarea"].contains(&node.tag.as_str());
        let clip_text = super::visual::clips_to_text(&chosen);
        // The UA sheet sets the top margin as `margin-block-start` (html.css); an author top
        // margin in either spelling replaces it.
        let from = |key: &str, by_author: bool| {
            chosen
                .get(key)
                .is_some_and(|((_, author, ..), _)| *author == by_author)
        };
        self.ua_margin_top = (from("margin-block-start", false) || from("margin-top", false))
            && !from("margin-top", true)
            && !from("margin-block-start", true);
        let authored = control.then(|| authored_control(&chosen));
        self.resolve_in(
            &mut style,
            parent_style,
            chosen,
            Some(scope_parent),
            control,
        )?;
        if default_link_color {
            style.color = super::resolve::link_foreground(&style);
        }
        if clip_text {
            super::visual::clip_to_text(&mut style);
        }
        if let Some((font, edges)) = authored {
            (style.font_authored, style.edges_authored) = (font, edges);
        }
        style.interactive &= !hides && !unreachable(&style);
        self.repair(node, &mut style);
        let parent_node = parent.map(|index| &candidate.nodes[index]);
        let summary = parent_node.and_then(|p| self.tree.first_summary(p.local_id));
        // `inert` subtrees can be seen but never acted on.
        style.inert |= node.attributes.contains_key("inert");
        style.interactive &= !style.inert;
        style.hidden |= super::ua::closed(node, parent_node, summary)
            || node.attributes.contains_key("hidden")
            || node.tag == "option"
            || node.tag == "#comment"
            || (node.tag == "input" && node.input_type() == "hidden");
        // pointer-events:auto re-enables a box under pointer-events:none, as in CSS, but nothing
        // inside a hidden subtree is ever actionable.
        style.interactive &= !style.hidden && !style.invisible;
        if self.parsed.pseudo_rules && !style.hidden {
            let scope = self.scopes.len() - 1;
            for after in [false, true] {
                if let Some(generated) = self.generated(node, &style, scope, after)? {
                    style.generated.push(generated);
                }
            }
            style.first_letter = self.first_part(node, &style, scope, PseudoBox::FirstLetter)?;
            style.first_line = self.first_part(node, &style, scope, PseudoBox::FirstLine)?;
            if ["input", "textarea"].contains(&node.tag.as_str()) {
                let placeholder = self.first_part(node, &style, scope, PseudoBox::Placeholder)?;
                style.placeholder_color = placeholder.and_then(|part| part.color);
            }
            if style.list_item {
                let marker = self.first_part(node, &style, scope, PseudoBox::Marker)?;
                style.marker_color = marker.and_then(|part| part.color);
            }
            self.scroll_marker_boxes(node, &mut style, scope)?;
        }
        if style.validate().is_err() {
            // One element's out-of-range author values must not fail the page's whole cascade:
            // it falls back to inherited and user-agent styles, as if its rules were dropped.
            note(
                &mut self.diagnostics,
                "An element's CSS values were outside supported bounds and were ignored.".into(),
            );
            style = super::resolve::inherited(parent_style);
            super::ua::defaults(node, &mut style);
            style.interactive &= !hides;
        }
        style.validate()?;
        Ok(style)
    }
    /** The element's `::before` or `::after` box, when a rule gives it content. */
    fn generated(
        &mut self,
        node: &Node,
        element: &ComputedStyle,
        scope: usize,
        after: bool,
    ) -> EngineResult<Option<super::Generated>> {
        let matcher = |target: &Target, tree: &Tree<'a>| match target {
            Target::Selector(selector) => matches(tree, node, selector),
            Target::Mask(_) => Ok(false),
        };
        let subject = (node.tag.as_str(), &node.attributes);
        self.generated_for(subject, &matcher, (element, scope), after)
    }
    /** A wrapper's (`html` or `body`) `::before` and `::after` boxes, in that order: Blink
     * generates them for the root and body like for any element (render/root_generated.rs
     * lays them out). */
    fn wrapper_generated(
        &mut self,
        tag: &'static str,
        attributes: &BTreeMap<String, String>,
        (element, scope): (&ComputedStyle, usize),
    ) -> EngineResult<Vec<super::Generated>> {
        if !self.parsed.pseudo_rules || element.hidden {
            return Ok(Vec::new());
        }
        let matcher = |target: &Target, tree: &Tree<'a>| match target {
            Target::Selector(selector) => Ok(matches_wrapper(tree, tag, selector)),
            Target::Mask(_) => Ok(false),
        };
        let mut boxes = Vec::new();
        for after in [false, true] {
            boxes.extend(self.generated_for(
                (tag, attributes),
                &matcher,
                (element, scope),
                after,
            )?);
        }
        Ok(boxes)
    }
    /** The `::before` (or `::after`) box of the subject `matcher` matches, when a rule gives it
     * content. */
    fn generated_for(
        &mut self,
        (tag, attributes): (&str, &BTreeMap<String, String>),
        matcher: &dyn Fn(&Target, &Tree<'a>) -> EngineResult<bool>,
        (element, scope): (&ComputedStyle, usize),
        after: bool,
    ) -> EngineResult<Option<super::Generated>> {
        let (mut chosen, _) = self.winning_for(
            tag,
            attributes,
            (&[], &[]),
            Some(if after {
                PseudoBox::After
            } else {
                PseudoBox::Before
            }),
            matcher,
        )?;
        let Some((_, content)) = chosen.remove("content") else {
            return Ok(None);
        };
        // The pseudo-element is its own element for custom properties (Blink computes it with
        // the originating element as parent), so its `--*` declarations and `var()` in
        // `content` resolve in a scope of its own, dropped once the box is computed.
        let custom: BTreeMap<Rc<str>, Rc<str>> = chosen
            .iter()
            .filter(|(key, _)| key.starts_with("--"))
            .map(|(key, (_, value))| (Rc::from(key.as_str()), value.clone()))
            .collect();
        chosen.retain(|key, _| !key.starts_with("--"));
        self.scopes.push(Some(scope), custom);
        let result = self.generated_box(attributes, element, chosen, &content, after);
        self.scopes.pop();
        result
    }
    /** The element's `::first-letter` or `::first-line` style when a rule styles it: inline,
     * inheriting from the element, in a custom-property scope of its own like `::before`.
     * Inline boxes, flex and grid containers have neither (only block containers do). */
    fn first_part(
        &mut self,
        node: &Node,
        element: &ComputedStyle,
        scope: usize,
        part: PseudoBox,
    ) -> EngineResult<Option<Box<ComputedStyle>>> {
        let block_part = matches!(part, PseudoBox::FirstLetter | PseudoBox::FirstLine);
        if block_part
            && ((element.inline && !element.inline_block)
                || element.display_flex
                || element.display_grid)
        {
            return Ok(None);
        }
        let (mut chosen, _) = self.winning_for(
            &node.tag,
            &node.attributes,
            (&[], &[]),
            Some(part),
            &|target, tree| match target {
                Target::Selector(selector) => matches(tree, node, selector),
                Target::Mask(_) => Ok(false),
            },
        )?;
        if chosen.is_empty() {
            return Ok(None);
        }
        let custom: BTreeMap<Rc<str>, Rc<str>> = chosen
            .iter()
            .filter(|(key, _)| key.starts_with("--"))
            .map(|(key, (_, value))| (Rc::from(key.as_str()), value.clone()))
            .collect();
        chosen.retain(|key, _| !key.starts_with("--"));
        self.scopes.push(Some(scope), custom);
        let mut style = super::resolve::inherited(element);
        style.inline = true;
        let resolver = super::compute::Resolver {
            scopes: &self.scopes,
            scope: self.scopes.len() - 1,
            context: self.context,
            control: false,
            keyframes: &self.parsed.keyframes,
        };
        let resolved = resolver.resolve(&mut style, element, chosen, &mut self.diagnostics);
        self.scopes.pop();
        resolved?;
        Ok(Some(Box::new(style)))
    }
    /** The element's `::scroll-marker` box (a generated box with content) and, when it asks
     * for one with `scroll-marker-group`, its `::scroll-marker-group` box, a block. */
    fn scroll_marker_boxes(
        &mut self,
        node: &Node,
        style: &mut ComputedStyle,
        scope: usize,
    ) -> EngineResult<()> {
        let marker = self.pseudo_box(node, style, scope, PseudoBox::ScrollMarker, true)?;
        style.scroll_marker = marker.map(Box::new);
        let mut buttons = Vec::new();
        for button in super::scroll_button::BUTTONS {
            let pseudo = PseudoBox::ScrollButton(button);
            if let Some(mut generated) = self.pseudo_box(node, style, scope, pseudo, true)? {
                // A button is an inline-block, like the `<button>` it stands for.
                generated.style.inline_block |= generated.style.inline;
                buttons.push(generated);
            }
        }
        style.scroll_buttons = buttons;
        if style.marker_group_side.is_some() {
            let group = self.pseudo_box(node, style, scope, PseudoBox::ScrollMarkerGroup, false)?;
            style.marker_group = group.map(|group| {
                let mut group = group.style;
                super::scroll_marker::adjust_group(&mut group);
                group
            });
        }
        Ok(())
    }
    /** One pseudo-element box of `node`, computed like `::before`; `needs_content` drops it
     * without a `content` declaration. A group box needs none and starts as a block. */
    fn pseudo_box(
        &mut self,
        node: &Node,
        element: &ComputedStyle,
        scope: usize,
        pseudo: PseudoBox,
        needs_content: bool,
    ) -> EngineResult<Option<super::Generated>> {
        let (mut chosen, _) = self.winning_for(
            &node.tag,
            &node.attributes,
            (&[], &[]),
            Some(pseudo),
            &|target, tree| match target {
                Target::Selector(selector) => matches(tree, node, selector),
                Target::Mask(_) => Ok(false),
            },
        )?;
        let content = match chosen.remove("content") {
            Some((_, content)) => content.to_string(),
            None if needs_content => return Ok(None),
            None => "\"\"".to_string(),
        };
        let custom: BTreeMap<Rc<str>, Rc<str>> = chosen
            .iter()
            .filter(|(key, _)| key.starts_with("--"))
            .map(|(key, (_, value))| (Rc::from(key.as_str()), value.clone()))
            .collect();
        chosen.retain(|key, _| !key.starts_with("--"));
        self.scopes.push(Some(scope), custom);
        let result = self.generated_box(&node.attributes, element, chosen, &content, false);
        self.scopes.pop();
        result
    }
    /** Computes a generated box's style in the pseudo-element scope pushed last. */
    fn generated_box(
        &mut self,
        attributes: &BTreeMap<String, String>,
        element: &ComputedStyle,
        chosen: Chosen,
        content: &str,
        after: bool,
    ) -> EngineResult<Option<super::Generated>> {
        let scope = self.scopes.len() - 1;
        let content = if content.to_ascii_lowercase().contains("var(") {
            match self.scopes.substitute(scope, content) {
                Some(content) => content,
                None => return Ok(None),
            }
        } else {
            content.to_string()
        };
        let Some(content) = super::content::content(&content, attributes) else {
            return Ok(None);
        };
        let mut style = super::resolve::inherited(element);
        // Generated boxes are inline unless a rule says otherwise.
        style.inline = true;
        let resolver = super::compute::Resolver {
            scopes: &self.scopes,
            scope,
            context: self.context,
            control: false,
            keyframes: &self.parsed.keyframes,
        };
        resolver.resolve(&mut style, element, chosen, &mut self.diagnostics)?;
        Ok(Some(super::Generated {
            after,
            content,
            style: Box::new(style),
        }))
    }
    /** Unsupported resolved combinations fall back to supported layout instead of refusing. */
    fn repair(&mut self, node: &Node, style: &mut ComputedStyle) {
        let table = [
            "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
        ];
        if style.margin_auto.iter().any(|auto| *auto) && table.contains(&node.tag.as_str()) {
            note(
                &mut self.diagnostics,
                "Automatic margins in table layout are ignored.".into(),
            );
            style.margin_auto = [false; 4];
        }
    }
    /** Wrapper resolution: html has no parent scope, body's is html. */
    fn resolve(
        &mut self,
        style: &mut ComputedStyle,
        parent: &ComputedStyle,
        chosen: Chosen,
        scope: usize,
        control: bool,
    ) -> EngineResult<()> {
        let parent_scope = (scope == BODY).then_some(HTML);
        self.resolve_in(style, parent, chosen, parent_scope, control)
    }
    /** Pushes this element's custom properties, then computes every winning declaration. */
    fn resolve_in(
        &mut self,
        style: &mut ComputedStyle,
        parent: &ComputedStyle,
        mut chosen: Chosen,
        parent_scope: Option<usize>,
        control: bool,
    ) -> EngineResult<()> {
        let custom: BTreeMap<Rc<str>, Rc<str>> = chosen
            .iter()
            .filter(|(key, _)| key.starts_with("--"))
            .map(|(key, (_, value))| (Rc::from(key.as_str()), value.clone()))
            .collect();
        chosen.retain(|key, _| !key.starts_with("--"));
        self.scopes.push(parent_scope, custom);
        let scope = self.scopes.len() - 1;
        let resolver = super::compute::Resolver {
            scopes: &self.scopes,
            scope,
            context: self.context,
            control,
            keyframes: &self.parsed.keyframes,
        };
        let hides = resolver.resolve(style, parent, chosen, &mut self.diagnostics)?;
        style.interactive &= !hides;
        Ok(())
    }
    /** Each property's cascade winner: UA first, then matching rules, then the style attribute. */
    fn winning(
        &mut self,
        tag: &str,
        attributes: &BTreeMap<String, String>,
        ua: &[(&str, &str)],
        hints: &[(&str, String)],
        matcher: &dyn Fn(&Target, &Tree<'a>) -> EngineResult<bool>,
    ) -> EngineResult<(Chosen, bool)> {
        self.winning_for(tag, attributes, (ua, hints), None, matcher)
    }
    /**
     * Cascade winners for an element (`pseudo` None) or its `::before`/`::after` box. A winner
     * of `revert` or `revert-layer` rolls its property back (Cascade 5 section 7.3): the
     * cascade reruns with that origin's or layer's declarations of it left out, as Blink's
     * StyleCascade looks up the reverted origin or layer, until no winner reverts.
     */
    fn winning_for(
        &mut self,
        tag: &str,
        attributes: &BTreeMap<String, String>,
        defaults: Defaults<'_>,
        pseudo: Option<PseudoBox>,
        matcher: &dyn Fn(&Target, &Tree<'a>) -> EngineResult<bool>,
    ) -> EngineResult<(Chosen, bool)> {
        let mut rolled = Rolled::new();
        let mut round = 0;
        loop {
            let (mut chosen, hides) =
                self.gather(tag, attributes, defaults, pseudo, matcher, &rolled)?;
            let reverting: Vec<(String, Revert)> = chosen
                .iter()
                .filter_map(|(key, (priority, value))| {
                    Some((key.clone(), revert_of(priority, value)?))
                })
                .collect();
            round += 1;
            if reverting.is_empty() || round > REVERT_ROUNDS {
                chosen.retain(|_, (priority, value)| revert_of(priority, value).is_none());
                return Ok((chosen, hides));
            }
            for (key, revert) in reverting {
                rolled.entry(key).or_default().push(revert);
            }
        }
    }
    /** One cascade pass, leaving out the declarations an earlier `revert` rolled back. */
    fn gather(
        &mut self,
        tag: &str,
        attributes: &BTreeMap<String, String>,
        (ua, hints): Defaults<'_>,
        pseudo: Option<PseudoBox>,
        matcher: &dyn Fn(&Target, &Tree<'a>) -> EngineResult<bool>,
        rolled: &Rolled,
    ) -> EngineResult<(Chosen, bool)> {
        let mut chosen = Chosen::new();
        let mut hides = false;
        let choose = |chosen: &mut Chosen, key: &str, value: Rc<str>, priority: Priority| {
            if !rolled_back(rolled, key, &priority) {
                choose(chosen, key, value, priority);
            }
        };
        for (key, value) in ua {
            choose(
                &mut chosen,
                key,
                value.to_string().into(),
                (false, false, false, 0, 0, 0),
            );
        }
        // Presentational hints are author-level with zero specificity, so any rule beats them.
        for (key, value) in hints {
            choose(
                &mut chosen,
                key,
                value.as_str().into(),
                (false, true, false, 0, 0, 0),
            );
        }
        for index in self.parsed.buckets.candidates(tag, attributes, pseudo) {
            if let Some(group) = self.groups.get(index)
                && !crate::document::shadow::applies(*group, attributes)
            {
                continue;
            }
            let rule = &self.parsed.rules[index];
            // A mask demanding ancestors cannot match wrappers that lack them (see Rule).
            let rootless =
                (tag == "html" && rule.ancestors >= 1) || (tag == "body" && rule.ancestors >= 2);
            if !super::scroll_button::matches(rule.pseudo, pseudo) || rootless {
                continue;
            }
            self.tree.exhausted.set(false);
            if !matcher(&rule.target, &self.tree)? {
                if self.tree.exhausted.get() && rule.hiding && masked(&self.tree, rule, matcher)? {
                    note(
                        &mut self.diagnostics,
                        "A :has() check ran past its budget; its hiding rule masks what it could match.".into(),
                    );
                    hides = true;
                }
                continue;
            }
            if !rule.containers.is_empty() {
                match self.container_truth(rule) {
                    Truth::True => {}
                    Truth::False => continue,
                    // Undecided, like a dropped `@container`: its hiding rules mask what they match.
                    Truth::Unknown => {
                        hides |= rule.hiding;
                        continue;
                    }
                }
            }
            hides |= rule.hides;
            // `:host` and `::slotted()` rules come from an inner tree context, which loses to
            // the outer context's normal declarations and wins with `!important`.
            let anchored = self.groups.get(index) == Some(&crate::document::shadow::ANY_GROUP);
            for (position, declaration) in rule.declarations.iter().enumerate() {
                let priority = (
                    declaration.important,
                    !anchored || declaration.important,
                    false,
                    layer_key(&self.parsed.layer_ranks, rule.layer, declaration.important),
                    rule.specificity,
                    source_order(rule.order, position),
                );
                choose(
                    &mut chosen,
                    &declaration.name,
                    declaration.value.clone(),
                    priority,
                );
            }
        }
        if let Some(inline) = attributes.get("style").filter(|_| pseudo.is_none()) {
            let block = declaration::block(inline, &mut self.diagnostics);
            hides |= block.hides;
            for (position, declaration) in block.declarations.into_iter().enumerate() {
                let order = source_order(self.parsed.rules.len(), position);
                let priority = (declaration.important, true, true, 0, 0, order);
                choose(&mut chosen, &declaration.name, declaration.value, priority);
            }
        }
        Ok((chosen, hides))
    }
}
impl Cascade<'_> {
    /** Whether every `@container` query around a matched rule holds for the current element. */
    fn container_truth(&self, rule: &Rule) -> Truth {
        let custom =
            |scope: usize, name: &str| self.scopes.substitute(scope, &format!("var({name})"));
        rule.containers.iter().fold(Truth::True, |truth, query| {
            let sizes = self.sizes.as_ref();
            truth.and(query.truth(&self.ancestry, self.parent_scope, sizes, &custom))
        })
    }
}
/** Size containers among an element's ancestors, nearest first, from their computed styles. */
fn ancestry(
    candidate: &Document,
    positions: &BTreeMap<usize, usize>,
    parent: Option<usize>,
) -> Vec<super::container::Ancestor> {
    let mut found = Vec::new();
    let mut current = parent;
    while let Some(index) = current {
        let node = &candidate.nodes[index];
        let scope = index + FIRST_NODE;
        found.extend(super::container::ancestor(
            node.local_id,
            scope,
            &node.style,
        ));
        current = node.parent.and_then(|id| positions.get(&id).copied());
    }
    found
}
/**
 * A hiding rule whose `:has()` ran out of budget is undecided, so it acts like a dropped hiding
 * rule: its subject without `:has()` is a mask, and a match keeps the element from agents.
 */
fn masked<'a>(
    tree: &Tree<'a>,
    rule: &super::sheet::Rule,
    matcher: &dyn Fn(&Target, &Tree<'a>) -> EngineResult<bool>,
) -> EngineResult<bool> {
    let Target::Selector(selector) = &rule.target else {
        return Ok(false);
    };
    let subject = super::selector_parse::has_free_subject(selector);
    matcher(&Target::Mask(subject), tree)
}
/** Fail closed: no node, wrapper included, is agent-actionable; people still see the page. */
fn untrusted(document: &mut Document) {
    document.formatting.agent_untrusted = true;
    document.formatting.html.style.interactive = false;
    document.formatting.body.style.interactive = false;
    for node in &mut document.nodes {
        node.style.interactive = false;
    }
}
/** Positioned or shifted far off-screen, or transformed to nothing: hidden from agents. */
fn unreachable(style: &ComputedStyle) -> bool {
    let positioned = matches!(style.position, Position::Absolute | Position::Fixed);
    let far = |value: f64| value <= -OFFSCREEN;
    let offset = positioned && (style.inset[0].is_some_and(far) || style.inset[3].is_some_and(far));
    // A negative percentage margin of a whole containing block or more moves the box out of it.
    let pushed = |side: usize| style.margin_percent[side] <= -super::constants::PERCENT;
    let shifted = far(style.margin[0]) || far(style.margin[3]) || pushed(0) || pushed(3);
    let degenerate = style
        .transform
        .is_some_and(|[a, b, c, d, e, f]| (a * d - b * c).abs() < f64::EPSILON || far(e) || far(f));
    offset || shifted || degenerate
}
/** An explicit `dir` sets the inherited base direction and `lang` the inherited content
language; elements without them keep the parent's. */
fn direction(attributes: &BTreeMap<String, String>, style: &mut ComputedStyle) {
    // ponytail: `dir=auto` inherits instead of running first-strong detection; add it with bidi.
    match attributes
        .get("dir")
        .map(|dir| dir.to_ascii_lowercase())
        .as_deref()
    {
        Some("rtl") => style.rtl = true,
        Some("ltr") => style.rtl = false,
        _ => {}
    }
    if let Some(lang) = attributes.get("lang") {
        style.lang = lang.to_ascii_lowercase();
    }
}
/** How many times one element's cascade reruns for `revert`; each rerun drops one layer or
origin, so real sheets settle in a few and a hostile one cannot loop. */
const REVERT_ROUNDS: usize = 32;
/** What a `revert` winner rolls back: one origin (the author's when true, else the user
agent's) or, for `revert-layer`, one layer of one origin and importance. */
#[derive(Clone, Copy)]
enum Revert {
    Origin(bool),
    Layer(bool, bool, bool, usize),
}
/** The rolled-back origins and layers of each property. */
type Rolled = BTreeMap<String, Vec<Revert>>;
/** The rollback a winning value asks for, if it is `revert` or `revert-layer`. */
fn revert_of(priority: &Priority, value: &str) -> Option<Revert> {
    let value = value.trim();
    let (important, author, inline, layer, _, _) = *priority;
    if value.eq_ignore_ascii_case("revert") {
        return Some(Revert::Origin(author));
    }
    if !value.eq_ignore_ascii_case("revert-layer") {
        return None;
    }
    // In the user agent origin `revert-layer` has no layer below it and acts as `revert`.
    Some(if author {
        Revert::Layer(important, author, inline, layer)
    } else {
        Revert::Origin(false)
    })
}
/** Whether a declaration of `key` at `priority` was rolled back. Reverting the author origin
leaves only the user agent's; reverting that leaves nothing, so the property is unset. */
fn rolled_back(rolled: &Rolled, key: &str, priority: &Priority) -> bool {
    let (important, author, inline, layer, _, _) = *priority;
    rolled.get(key).is_some_and(|reverts| {
        reverts.iter().any(|revert| match *revert {
            Revert::Origin(true) => author,
            Revert::Origin(false) => true,
            Revert::Layer(i, a, n, l) => (i, a, n, l) == (important, author, inline, layer),
        })
    })
}
/** A rule's layer in the cascade: its rank, inverted for `!important` (Cascade 5 section 6.4). */
fn layer_key(ranks: &[usize], layer: usize, important: bool) -> usize {
    let rank = ranks.get(layer).copied().unwrap_or(usize::MAX - 1);
    if important {
        usize::MAX - 1 - rank
    } else {
        rank
    }
}
/// Declarations one rule orders among themselves; later ones share the last position.
const RULE_DECLARATIONS: usize = 1 << 16;
/**
 * Source order of a declaration: its rule's, then its place in the rule, so a logical and a
 * physical spelling of one side in the same rule resolve by which came last.
 */
fn source_order(rule: usize, position: usize) -> usize {
    rule.saturating_mul(RULE_DECLARATIONS)
        .saturating_add(position.min(RULE_DECLARATIONS - 1))
}
/** A stronger or equal later declaration wins without depending on hash iteration. */
pub(super) fn choose(chosen: &mut Chosen, key: &str, value: Rc<str>, priority: Priority) {
    if chosen
        .get(key)
        .is_none_or(|(previous, _)| priority >= *previous)
    {
        chosen.insert(key.into(), (priority, value));
    }
}

/** Whether the page set a control's font, and its padding or border widths. The UA sheet sets
 * neither on controls, so any winner is the page's (`font: inherit` and `padding: 0` included,
 * which Tailwind's preflight gives every button). */
fn authored_control(chosen: &Chosen) -> (bool, bool) {
    let font = chosen
        .keys()
        .any(|key| ["font-size", "font-family"].contains(&key.as_str()));
    let edges = chosen.keys().any(|key| {
        key.starts_with("padding")
            || (key.starts_with("border") && (key.ends_with("width") || key.ends_with("style")))
    });
    (font, edges)
}

/** The quirks-mode margin quirk: Chromium drops the UA top margin of the first box in the body
 * or a table cell (CERN's first heading sat 21px low). True when the node's parent is the body
 * or a cell and no visible sibling or text comes before it. */
fn opens_quirk_container(document: &Document, index: usize, parent: Option<usize>) -> bool {
    if let Some(parent) = parent {
        let cell = &document.nodes[parent];
        if !["td", "th"].contains(&cell.tag.as_str()) || !cell.text.trim().is_empty() {
            return false;
        }
    }
    let first = parent.map_or(0, |parent| parent + 1);
    let node = &document.nodes[index];
    // Backwards, so only the nearest earlier sibling's subtree is walked.
    !(first..index).rev().any(|before| {
        let sibling = &document.nodes[before];
        sibling.parent == node.parent && !blank(document, before)
    })
}
/** A box margins collapse through: hidden, or with no text, replaced content, height or edges
 * anywhere in its subtree (CERN's `<HEADER>` holding only its title). */
fn blank(document: &Document, index: usize) -> bool {
    let mut subtree = vec![document.nodes[index].local_id];
    let mut hidden: Vec<usize> = Vec::new();
    for node in &document.nodes[index..] {
        if node.local_id != subtree[0] && !node.parent.is_some_and(|p| subtree.contains(&p)) {
            break;
        }
        subtree.push(node.local_id);
        let style = &node.style;
        if style.hidden
            || node.tag == "#comment"
            || node.parent.is_some_and(|p| hidden.contains(&p))
        {
            hidden.push(node.local_id);
            continue;
        }
        let boxed = style.height.is_some()
            || style
                .padding
                .iter()
                .chain(style.border_widths.iter())
                .any(|v| *v > 0.0);
        let replaced = [
            "img", "svg", "input", "button", "select", "textarea", "video", "iframe",
        ]
        .contains(&node.tag.as_str());
        if !node.text.trim().is_empty() || boxed || replaced {
            return false;
        }
    }
    true
}
