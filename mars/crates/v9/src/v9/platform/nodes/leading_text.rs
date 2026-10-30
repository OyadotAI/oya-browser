/** Materialize the projection's implicit leading text before the owned fragment parser assigns real DOM identities. */
use crate::protocol::EngineResult;
use crate::v9::platform::Page;

/** Raw and embedded content keep their specialized parser contracts. */
const RETAIN_TEXT: &[&str] = &[
    "script", "style", "textarea", "title", "img", "input", "iframe", "object", "embed", "canvas",
    "video", "audio",
];

impl Page<'_> {
    /** Leading text on an ordinary detached element becomes a real first child, matching DomFragments normalization. */
    pub(super) fn materialize_leading_text(&mut self, root: usize) -> EngineResult<()> {
        let mut nodes = vec![root];
        nodes.extend(self.all_descendants(root));
        for node in nodes {
            let text = self
                .state
                .detached
                .get(&node)
                .filter(|record| {
                    !record.tag.starts_with('#') && !RETAIN_TEXT.contains(&record.tag.as_str())
                })
                .and_then(|record| record.text.clone())
                .filter(|text| !text.is_empty());
            if let Some(text) = text {
                let child = self.create("#text", Some(text))?;
                let record = self
                    .state
                    .detached
                    .get_mut(&node)
                    .expect("detached leading-text element");
                record.text = None;
                record.children.insert(0, child);
                self.state
                    .detached
                    .get_mut(&child)
                    .expect("new leading-text child")
                    .parent = Some(node);
            }
        }
        Ok(())
    }
}
