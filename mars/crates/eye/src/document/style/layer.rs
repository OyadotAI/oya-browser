/**
 * CSS Cascade 5 layers, after Blink's CascadeLayer and CascadeLayerMap: every `@layer` name
 * becomes a node in one tree under the implicit outer layer, in order of first appearance, and
 * the cascade ranks layers by a post-order walk, so a layer's sublayers come before its own
 * rules and unlayered rules (the outer layer) come last.
 */
/// Layers one page may declare; past this new names fold into their parent layer.
const CSS_LAYERS: usize = 4096;

/** One named or anonymous layer. */
#[derive(Clone, Debug, Default)]
struct Layer {
    /// The last segment of a dotted name; None for an anonymous `@layer { }` block.
    name: Option<String>,
    /// Sublayers in order of first declaration.
    children: Vec<usize>,
}

/** Every layer the sheets declared; index 0 is the implicit outer layer (unlayered rules). */
#[derive(Clone, Debug)]
pub(super) struct Layers {
    /// The layer tree, by index.
    layers: Vec<Layer>,
}

impl Default for Layers {
    /** Only the implicit outer layer. */
    fn default() -> Self {
        Self {
            layers: vec![Layer::default()],
        }
    }
}

/** The implicit outer layer: rules outside any `@layer`. */
pub(super) const OUTER: usize = 0;

impl Layers {
    /**
     * The layer `name` (dotted, as in `a.b`) under `parent`, declared on first use. Past the
     * layer bound new names fold into `parent`, so a hostile sheet cannot grow the tree.
     */
    pub(super) fn declare(&mut self, parent: usize, name: &str) -> usize {
        name.split('.')
            .map(str::trim)
            .fold(parent, |parent, segment| self.child(parent, Some(segment)))
    }
    /** A fresh anonymous layer under `parent`: each `@layer { }` block is its own layer. */
    pub(super) fn anonymous(&mut self, parent: usize) -> usize {
        self.child(parent, None)
    }
    /** The named child of `parent`, created at the end of its children when new. */
    fn child(&mut self, parent: usize, name: Option<&str>) -> usize {
        let existing = self.layers[parent]
            .children
            .iter()
            .copied()
            .find(|child| name.is_some() && self.layers[*child].name.as_deref() == name);
        if let Some(found) = existing {
            return found;
        }
        if self.layers.len() >= CSS_LAYERS {
            return parent;
        }
        self.layers.push(Layer {
            name: name.map(str::to_owned),
            children: Vec::new(),
        });
        let index = self.layers.len() - 1;
        self.layers[parent].children.push(index);
        index
    }
    /**
     * Each layer's cascade rank: sublayers first, in declaration order, then the layer itself,
     * so the outer layer ranks last and its unlayered rules beat every layered one.
     */
    pub(super) fn ranks(&self) -> Vec<usize> {
        let mut ranks = vec![0; self.layers.len()];
        let mut next = 0;
        // Iterative post-order: (layer, children already pushed).
        let mut stack = vec![(OUTER, false)];
        while let Some((layer, expanded)) = stack.pop() {
            if expanded {
                ranks[layer] = next;
                next += 1;
                continue;
            }
            stack.push((layer, true));
            for child in self.layers[layer].children.iter().rev() {
                stack.push((*child, false));
            }
        }
        ranks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Sublayers rank before their parent's own rules, and the outer layer ranks last. */
    #[test]
    fn post_order_puts_sublayers_first_and_unlayered_last() {
        let mut layers = Layers::default();
        let a = layers.declare(OUTER, "a");
        let ab = layers.declare(OUTER, "a.b");
        let c = layers.declare(OUTER, "c");
        let ranks = layers.ranks();
        assert!(ranks[ab] < ranks[a]);
        assert!(ranks[a] < ranks[c]);
        assert!(ranks[c] < ranks[OUTER]);
        assert_eq!(layers.declare(a, "b"), ab);
    }
}
