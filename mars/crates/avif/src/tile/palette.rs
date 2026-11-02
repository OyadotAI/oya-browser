/** Palette syntax (spec palette_mode_info, get_palette_cache, palette_tokens and get_palette_color_context): the colours of a palette block drawn from its neighbours' cache and new deltas, and the colour-index map that assigns each sample a colour. */
use super::constants::{
    COLOR_MAP_STRIDE, DC_PRED, MI_SIZE, PALETTE_BSIZE_CTX_OFFSET, PALETTE_CHROMA_MIN_DIM,
    PALETTE_CHROMA_PAD, PALETTE_COLORS, PALETTE_DELTA_MIN_BITS, PALETTE_DELTA_MIN_BITS_V,
    PALETTE_EXTRA_BITS, PALETTE_MIN_SIZE, PALETTE_NUM_NEIGHBORS,
};
use super::decoder::TileDecoder;
use super::lr::plane_subsampling;
use super::tables::{
    MI_HEIGHT_LOG2, MI_WIDTH_LOG2, PALETTE_COLOR_CONTEXT, PALETTE_COLOR_HASH_MULTIPLIERS,
};
use crate::error::{Error, Result};

/** Samples in a superblock row; a palette's above neighbour is read only inside one. */
const SUPERBLOCK_ROW_SAMPLES: usize = 64;
/** The colour-order score table has one entry per possible palette colour. */
const SCORES: usize = PALETTE_COLORS;

impl<'a> TileDecoder<'a> {
    /** palette_mode_info(): whether the luma and the chroma of an intra block carry a palette, and its colours. */
    pub(super) fn palette_mode_info(&mut self) -> Result<()> {
        let mi = self.blk.mi_size;
        let bsize_ctx =
            (MI_WIDTH_LOG2[mi] + MI_HEIGHT_LOG2[mi]) as usize - PALETTE_BSIZE_CTX_OFFSET;
        if self.blk.y_mode == DC_PRED {
            self.palette_luma(bsize_ctx)?;
        }
        if self.blk.has_chroma && self.blk.uv_mode == DC_PRED {
            self.palette_chroma(bsize_ctx)?;
        }
        Ok(())
    }

    /** has_palette_y and, when set, the luma palette's size and colours. */
    fn palette_luma(&mut self, bsize_ctx: usize) -> Result<()> {
        let ctx = self.palette_neighbours(0);
        if self
            .sym
            .read_symbol(&mut self.cdf.palette_y_mode[bsize_ctx][ctx])?
            == 0
        {
            return Ok(());
        }
        let size = self
            .sym
            .read_symbol(&mut self.cdf.palette_y_size[bsize_ctx])?
            + PALETTE_MIN_SIZE;
        self.blk.palette_size_y = size;
        self.blk.palette_colors[0] = self.read_sample_colors(0, size)?;
        Ok(())
    }

    /** has_palette_uv and, when set, the chroma palette: its U colours, then its V colours. */
    fn palette_chroma(&mut self, bsize_ctx: usize) -> Result<()> {
        let ctx = usize::from(self.blk.palette_size_y > 0);
        if self.sym.read_symbol(&mut self.cdf.palette_uv_mode[ctx])? == 0 {
            return Ok(());
        }
        let size = self
            .sym
            .read_symbol(&mut self.cdf.palette_uv_size[bsize_ctx])?
            + PALETTE_MIN_SIZE;
        self.blk.palette_size_uv = size;
        self.blk.palette_colors[1] = self.read_sample_colors(1, size)?;
        self.blk.palette_colors[2] = self.read_v_colors(size)?;
        Ok(())
    }

    /** How many of the above and left neighbours have a palette in this plane, as the has_palette context. */
    fn palette_neighbours(&self, plane: usize) -> usize {
        let (r, c) = (self.blk.mi_row, self.blk.mi_col);
        let above = self.blk.avail_u && self.fs.mi.at(r - 1, c).palette_size[plane] > 0;
        let left = self.blk.avail_l && self.fs.mi.at(r, c - 1).palette_size[plane] > 0;
        usize::from(above) + usize::from(left)
    }

    /** The colours of the luma or U palette: cached colours flagged one by one, then one literal, then ascending deltas. Luma deltas are at least one; U deltas may be zero. */
    fn read_sample_colors(&mut self, plane: usize, size: usize) -> Result<[u16; PALETTE_COLORS]> {
        let bd = u32::from(self.seq.color.bit_depth);
        let cache = self.palette_cache(plane);
        let mut colors = [0u16; PALETTE_COLORS];
        let mut idx = 0;
        for &cached in &cache {
            if idx >= size {
                break;
            }
            if self.sym.read_literal(1)? == 1 {
                colors[idx] = cached;
                idx += 1;
            }
        }
        if idx < size {
            colors[idx] = self.sym.read_literal(bd)? as u16;
            idx += 1;
        }
        let mut bits = 0;
        if idx < size {
            bits = bd - PALETTE_DELTA_MIN_BITS + self.sym.read_literal(PALETTE_EXTRA_BITS)?;
        }
        while idx < size {
            let mut delta = self.sym.read_literal(bits)? as i64;
            let range_bias = if plane == 0 {
                delta += 1;
                1
            } else {
                0
            };
            let value = (i64::from(colors[idx - 1]) + delta).clamp(0, max_sample(bd));
            colors[idx] = value as u16;
            let range = (1i64 << bd) - value - range_bias;
            bits = bits.min(ceil_log2(range));
            idx += 1;
        }
        colors[..size].sort_unstable();
        Ok(colors)
    }

    /** The V colours: either delta-coded around the previous colour with wrap-around, or each a literal. */
    fn read_v_colors(&mut self, size: usize) -> Result<[u16; PALETTE_COLORS]> {
        let bd = u32::from(self.seq.color.bit_depth);
        let mut colors = [0u16; PALETTE_COLORS];
        if self.sym.read_literal(1)? == 0 {
            for slot in colors.iter_mut().take(size) {
                *slot = self.sym.read_literal(bd)? as u16;
            }
            return Ok(colors);
        }
        let bits = bd - PALETTE_DELTA_MIN_BITS_V + self.sym.read_literal(PALETTE_EXTRA_BITS)?;
        let max_val = 1i64 << bd;
        colors[0] = self.sym.read_literal(bd)? as u16;
        for idx in 1..size {
            let mut delta = i64::from(self.sym.read_literal(bits)?);
            if delta != 0 && self.sym.read_literal(1)? == 1 {
                delta = -delta;
            }
            let mut val = i64::from(colors[idx - 1]) + delta;
            if val < 0 {
                val += max_val;
            }
            if val >= max_val {
                val -= max_val;
            }
            colors[idx] = val.clamp(0, max_sample(bd)) as u16;
        }
        Ok(colors)
    }

    /** get_palette_cache(plane): the sorted, de-duplicated colours of the above palette (when it is in the same superblock row) and the left palette. */
    fn palette_cache(&self, plane: usize) -> Vec<u16> {
        let (r, c) = (self.blk.mi_row, self.blk.mi_col);
        let above: &[u16] = if !(r * MI_SIZE).is_multiple_of(SUPERBLOCK_ROW_SAMPLES) {
            self.neighbour_colors(r - 1, c, plane)
        } else {
            &[]
        };
        let left: &[u16] = if self.blk.avail_l {
            self.neighbour_colors(r, c - 1, plane)
        } else {
            &[]
        };
        merge_cache(above, left)
    }

    /** The colours of the palette at a neighbouring position in one plane, or none when it has no palette there. */
    fn neighbour_colors(&self, r: usize, c: usize, plane: usize) -> &[u16] {
        let index = self.fs.mi.at(r, c).palette_index as usize;
        match index.checked_sub(1).and_then(|i| self.fs.palettes.get(i)) {
            Some(info) => &info.colors[plane][..info.size[plane]],
            None => &[],
        }
    }

    /** palette_tokens(): the colour-index maps of the luma and chroma palettes, with the parts of the block past the frame edge copied from their last on-screen column and row. */
    pub(super) fn palette_tokens(&mut self) -> Result<()> {
        let (bw, bh) = (self.blk.width(), self.blk.height());
        let on_h = bh.min((self.mi_rows() - self.blk.mi_row) * MI_SIZE);
        let on_w = bw.min((self.mi_cols() - self.blk.mi_col) * MI_SIZE);
        if self.blk.palette_size_y > 0 {
            let size = self.blk.palette_size_y;
            self.blk.color_map_y = self.read_color_map(0, size, (bw, bh), (on_w, on_h))?;
        }
        if self.blk.palette_size_uv > 0 {
            let size = self.blk.palette_size_uv;
            let (sub_x, sub_y) = plane_subsampling(self.seq, 1);
            let (mut w, mut h) = (bw >> sub_x, bh >> sub_y);
            let (mut ow, mut oh) = (on_w >> sub_x, on_h >> sub_y);
            if w < PALETTE_CHROMA_MIN_DIM {
                w += PALETTE_CHROMA_PAD;
                ow += PALETTE_CHROMA_PAD;
            }
            if h < PALETTE_CHROMA_MIN_DIM {
                h += PALETTE_CHROMA_PAD;
                oh += PALETTE_CHROMA_PAD;
            }
            self.blk.color_map_uv = self.read_color_map(1, size, (w, h), (ow, oh))?;
        }
        Ok(())
    }

    /** One colour-index map: the first index is coded with ns(n), then each later index in anti-diagonal order with a context from its coded neighbours. The map has a fixed stride of COLOR_MAP_STRIDE. */
    fn read_color_map(
        &mut self,
        plane: usize,
        n: usize,
        block: (usize, usize),
        onscreen: (usize, usize),
    ) -> Result<Vec<u8>> {
        let (w, h) = block;
        let (ow, oh) = onscreen;
        let stride = COLOR_MAP_STRIDE;
        let mut map = vec![0u8; stride * stride];
        map[0] = self.sym.read_ns(n as u32)? as u8;
        for i in 1..(oh + ow - 1) {
            let hi = i.min(ow - 1);
            let lo = (i + 1).saturating_sub(oh);
            for j in (lo..=hi).rev() {
                let (r, c) = (i - j, j);
                let (order, hash) = color_context(&map, stride, r, c, n);
                let slot = PALETTE_COLOR_CONTEXT.get(hash).copied().unwrap_or(-1);
                let ctx = usize::try_from(slot)
                    .map_err(|_| Error::Malformed("palette colour context"))?;
                let symbol = self.read_color_index(plane, n, ctx)?;
                map[r * stride + c] = order[symbol];
            }
        }
        extend_map(&mut map, stride, (w, h), (ow, oh));
        Ok(map)
    }

    /** One colour index, read with the CDF of its palette size and context. */
    fn read_color_index(&mut self, plane: usize, n: usize, ctx: usize) -> Result<usize> {
        let row = n - PALETTE_MIN_SIZE;
        if plane == 0 {
            self.sym
                .read_symbol(&mut self.cdf.palette_y_color[row][ctx][..=n])
        } else {
            self.sym
                .read_symbol(&mut self.cdf.palette_uv_color[row][ctx][..=n])
        }
    }
}

/** The largest sample value at a bit depth. */
fn max_sample(bd: u32) -> i64 {
    (1i64 << bd) - 1
}

/** CeilLog2(x): the number of bits needed to index x values, zero for x below two. */
pub(super) fn ceil_log2(x: i64) -> u32 {
    if x < 2 {
        return 0;
    }
    let mut i = 1;
    let mut p = 2i64;
    while p < x {
        i += 1;
        p <<= 1;
    }
    i
}

/** Merge two ascending colour lists into one ascending list without repeats, the palette cache. */
pub(super) fn merge_cache(above: &[u16], left: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(above.len() + left.len());
    let (mut ai, mut li) = (0, 0);
    while ai < above.len() && li < left.len() {
        let (a, l) = (above[ai], left[li]);
        if l < a {
            push_unique(&mut out, l);
            li += 1;
        } else {
            push_unique(&mut out, a);
            ai += 1;
            if l == a {
                li += 1;
            }
        }
    }
    for &a in &above[ai..] {
        push_unique(&mut out, a);
    }
    for &l in &left[li..] {
        push_unique(&mut out, l);
    }
    out
}

/** Append a value unless it repeats the last one, which is enough because both inputs are ascending. */
fn push_unique(out: &mut Vec<u16>, value: u16) {
    if out.last() != Some(&value) {
        out.push(value);
    }
}

/** get_palette_color_context(): the colour order for one map position, ranked by how many of its coded neighbours use each colour, and the hash of the top scores. */
fn color_context(
    map: &[u8],
    stride: usize,
    r: usize,
    c: usize,
    n: usize,
) -> ([u8; PALETTE_COLORS], usize) {
    let mut scores = [0u32; SCORES];
    let mut order = [0u8; PALETTE_COLORS];
    for (i, slot) in order.iter_mut().enumerate() {
        *slot = i as u8;
    }
    if c > 0 {
        scores[usize::from(map[r * stride + c - 1])] += 2;
    }
    if r > 0 && c > 0 {
        scores[usize::from(map[(r - 1) * stride + c - 1])] += 1;
    }
    if r > 0 {
        scores[usize::from(map[(r - 1) * stride + c])] += 2;
    }
    for i in 0..PALETTE_NUM_NEIGHBORS {
        rank_step(&mut scores, &mut order, i, n);
    }
    let hash = (0..PALETTE_NUM_NEIGHBORS)
        .map(|i| scores[i] as usize * usize::from(PALETTE_COLOR_HASH_MULTIPLIERS[i]))
        .sum();
    (order, hash)
}

/** One selection step of the colour ranking: move the best-scoring colour among positions i..n to position i, shifting the others down. */
fn rank_step(scores: &mut [u32; SCORES], order: &mut [u8; PALETTE_COLORS], i: usize, n: usize) {
    let mut max_score = scores[i];
    let mut max_idx = i;
    for (j, &score) in scores.iter().enumerate().take(n).skip(i + 1) {
        if score > max_score {
            max_score = score;
            max_idx = j;
        }
    }
    if max_idx == i {
        return;
    }
    let max_order = order[max_idx];
    for k in (i + 1..=max_idx).rev() {
        scores[k] = scores[k - 1];
        order[k] = order[k - 1];
    }
    scores[i] = max_score;
    order[i] = max_order;
}

/** Copy the on-screen columns right and the on-screen rows down, so the whole block has a colour everywhere. */
fn extend_map(map: &mut [u8], stride: usize, block: (usize, usize), onscreen: (usize, usize)) {
    let (w, h) = block;
    let (ow, oh) = onscreen;
    for i in 0..oh {
        let edge = map[i * stride + ow - 1];
        for j in ow..w {
            map[i * stride + j] = edge;
        }
    }
    for i in oh..h {
        for j in 0..w {
            map[i * stride + j] = map[(oh - 1) * stride + j];
        }
    }
}
