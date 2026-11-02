/** Transform tree walk of an inter block's luma (spec transform_tree and find_tx_size): a region of the block is visited as the transform its tree leaf names, and a region too large for its leaf is split until each part fits. */
use super::constants::{MI_SIZE, TX_SIZES_ALL};
use super::decoder::TileDecoder;
use super::tables::{TX_HEIGHT, TX_WIDTH};
use crate::error::{Error, Result};

/** One region of the walk: its top-left sample and its size in samples. */
type Region = (usize, usize, usize, usize);

impl<'a> TileDecoder<'a> {
    /** transform_tree(startX, startY, w, h): visit the region's transform blocks in the order the tree gives them. A region past the frame edge is skipped. */
    pub(super) fn transform_tree(
        &mut self,
        start_x: usize,
        start_y: usize,
        w: usize,
        h: usize,
    ) -> Result<()> {
        if start_x >= self.mi_cols() * MI_SIZE || start_y >= self.mi_rows() * MI_SIZE {
            return Ok(());
        }
        let leaf = self
            .fs
            .mi
            .at(start_y / MI_SIZE, start_x / MI_SIZE)
            .inter_tx_size as usize;
        if w <= TX_WIDTH[leaf] as usize && h <= TX_HEIGHT[leaf] as usize {
            let tx = find_tx_size(w, h)?;
            return self.transform_block(0, start_x, start_y, tx, 0, 0);
        }
        for (x, y, cw, ch) in split_region(start_x, start_y, w, h) {
            self.transform_tree(x, y, cw, ch)?;
        }
        Ok(())
    }
}

/** The halves a region splits into: left and right when it is wide, top and bottom when it is tall, and four quarters when it is square. */
pub(super) fn split_region(x: usize, y: usize, w: usize, h: usize) -> Vec<Region> {
    let (hw, hh) = (w / 2, h / 2);
    if w > h {
        vec![(x, y, hw, h), (x + hw, y, hw, h)]
    } else if w < h {
        vec![(x, y, w, hh), (x, y + hh, w, hh)]
    } else {
        vec![
            (x, y, hw, hh),
            (x + hw, y, hw, hh),
            (x, y + hh, hw, hh),
            (x + hw, y + hh, hw, hh),
        ]
    }
}

/** The transform size with the given width and height in samples (spec find_tx_size). A pair no transform has is malformed, since the region came from a tree the block built. */
pub(super) fn find_tx_size(w: usize, h: usize) -> Result<usize> {
    (0..TX_SIZES_ALL)
        .find(|&tx| TX_WIDTH[tx] as usize == w && TX_HEIGHT[tx] as usize == h)
        .ok_or(Error::Malformed("transform size"))
}
