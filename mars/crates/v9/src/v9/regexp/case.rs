/** Case equivalence follows Unicode simple folding for u/v and ECMAScript's restricted uppercase mapping otherwise. */
use super::case_data::EQUIVALENTS;

/** Unicode table lookup supplies every equivalent scalar without a per-match allocation. */
fn table(point: u32) -> &'static [char] {
    char::from_u32(point)
        .and_then(|point| {
            EQUIVALENTS
                .binary_search_by_key(&point, |entry| entry.0)
                .ok()
        })
        .map_or(&[], |index| EQUIVALENTS[index].1)
}

/** Legacy canonicalization never expands a character or folds a non-ASCII unit into ASCII. */
fn legacy(point: u32) -> u32 {
    let Some(character) = char::from_u32(point) else {
        return point;
    };
    let mut uppercase = character.to_uppercase();
    let Some(first) = uppercase.next() else {
        return point;
    };
    if uppercase.next().is_some() || point >= 128 && first.is_ascii() {
        point
    } else {
        first as u32
    }
}

/** Simple folding preserves single-scalar matching and includes special equivalents such as final sigma and Kelvin sign. */
pub(super) fn same(left: u32, right: u32, unicode: bool) -> bool {
    if unicode {
        left == right || table(left).iter().any(|point| *point as u32 == right)
    } else {
        legacy(left) == legacy(right)
    }
}

/** Character classes examine the full equivalence class, filtering legacy-incompatible folds before range lookup. */
pub(super) fn equivalents(point: u32, unicode: bool) -> impl Iterator<Item = u32> {
    std::iter::once(point)
        .chain(table(point).iter().map(|character| *character as u32))
        .filter(move |candidate| unicode || legacy(*candidate) == legacy(point))
}

/** Class escapes close their set before complementing, so [\W] cannot gain word characters through later folding. */
pub(super) fn closed_ranges(ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let contains = |point| {
        ranges
            .iter()
            .any(|(low, high)| (*low..=*high).contains(&point))
    };
    let mut closed = ranges.to_vec();
    for (point, members) in EQUIVALENTS {
        if contains(*point as u32) || members.iter().any(|member| contains(*member as u32)) {
            closed.push((*point as u32, *point as u32));
        }
    }
    closed.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (low, high) in closed {
        if let Some(last) = merged.last_mut()
            && low <= last.1.saturating_add(1)
        {
            last.1 = last.1.max(high);
        } else {
            merged.push((low, high));
        }
    }
    merged
}
