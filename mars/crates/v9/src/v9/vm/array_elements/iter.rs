/** Logical iterators preserve hole positions; tracing and accounting use resident iterators. */
use super::{ArrayElements, JsValue};
use std::ops::Range;

/** Borrowed traversal of every JavaScript index, including holes. */
pub(in crate::v9) struct Iter<'a> {
    elements: &'a ArrayElements,
    range: Range<usize>,
}
impl<'a> Iter<'a> {
    /** The range is cheap even for the maximum legal array length. */
    pub(super) fn new(elements: &'a ArrayElements) -> Self {
        Self {
            elements,
            range: 0..elements.len(),
        }
    }
}
impl<'a> Iterator for Iter<'a> {
    type Item = &'a JsValue;
    /** Each item preserves its original logical position. */
    fn next(&mut self) -> Option<Self::Item> {
        self.elements.get(self.range.next()?)
    }
    /** No physical storage scan is needed to know how many logical positions remain. */
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl DoubleEndedIterator for Iter<'_> {
    /** Reverse array algorithms retain the same logical holes. */
    fn next_back(&mut self) -> Option<Self::Item> {
        self.elements.get(self.range.next_back()?)
    }
}
impl ExactSizeIterator for Iter<'_> {}
impl<'a> IntoIterator for &'a ArrayElements {
    type Item = &'a JsValue;
    type IntoIter = Iter<'a>;
    /** Logical iteration is explicit in every language-facing call site. */
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/** Owned traversal still allocates nothing for absent positions. */
pub(in crate::v9) struct IntoIter {
    elements: ArrayElements,
    range: Range<usize>,
}
impl Iterator for IntoIter {
    type Item = JsValue;
    /** Consumers that materialize a Vec must enforce their own actual output allowance. */
    fn next(&mut self) -> Option<Self::Item> {
        self.elements.get(self.range.next()?).cloned()
    }
    /** A bounded consumer can preflight the output before allocating. */
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl IntoIterator for ArrayElements {
    type Item = JsValue;
    type IntoIter = IntoIter;
    /** Moving the backing store avoids an unnecessary dense copy before iteration starts. */
    fn into_iter(self) -> Self::IntoIter {
        let range = 0..self.len();
        IntoIter {
            elements: self,
            range,
        }
    }
}
