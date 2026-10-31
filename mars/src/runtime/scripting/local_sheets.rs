/**
 * The `<style>` texts V9 alone holds (head styles scripts create or change, which the document
 * engine never sees as nodes) kept as one block inside the page's stylesheet list. CSS-in-JS
 * libraries insert their rules this way; without the block those rules never applied.
 */

/** Replace the block at `block` (start, length) in `sheets` with `texts`; an empty block is
placed at the end. Returns the new block. Sheets before and after it keep their order. */
pub(super) fn replace(
    sheets: &mut Vec<String>,
    block: (usize, usize),
    texts: &[String],
) -> (usize, usize) {
    let (start, length) = block;
    let start = if length == 0 {
        sheets.len()
    } else {
        start.min(sheets.len())
    };
    let end = (start + length).min(sheets.len());
    sheets.splice(start..end, texts.iter().cloned());
    (start, texts.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /** The block lands at the end first, then is replaced in place as it changes. */
    #[test]
    fn block_is_replaced_in_place() {
        let mut sheets = vec!["a".to_string()];
        let block = replace(&mut sheets, (0, 0), &["x".into(), "y".into()]);
        sheets.push("later".into());
        let block = replace(&mut sheets, block, &["z".into()]);
        assert_eq!(sheets, ["a", "z", "later"]);
        assert_eq!(block, (1, 1));
    }
}
