/** The CSS Grid track sizing algorithm (css-grid-2 section 12), following Blink's
 * GridTrackSizingAlgorithm: initialize base sizes and growth limits, resolve intrinsic sizes by
 * span groups, maximize, expand flexible tracks by finding the size of an fr, and stretch `auto`
 * tracks. One function sizes either axis; the caller supplies item contributions. */
/** A track's minimum sizing function. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MinSizing {
    /// A definite length.
    Fixed(f64),
    /// `auto`: the items' minimum contributions.
    Auto,
    /// `min-content`: the items' min-content contributions.
    MinContent,
    /// `max-content`: the items' max-content contributions.
    MaxContent,
}
/** A track's maximum sizing function. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MaxSizing {
    /// A definite length.
    Fixed(f64),
    /// `auto`: grows to the max-content contributions, and stretches into free space.
    Auto,
    /// `min-content`: grows to the min-content contributions.
    MinContent,
    /// `max-content`: grows to the max-content contributions.
    MaxContent,
    /// `fit-content(limit)`: max-content, but never past `limit` (unless the base size is).
    FitContent(f64),
    /// `<flex>`: a share of the leftover space.
    Fr(f64),
}
/** One track's sizing functions. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TrackSizing {
    /// Minimum sizing function.
    pub min: MinSizing,
    /// Maximum sizing function.
    pub max: MaxSizing,
}
/** What one grid item asks of the tracks it spans, all as outer sizes (margins included). */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Contribution {
    /// First spanned track.
    pub start: usize,
    /// Number of spanned tracks.
    pub span: usize,
    /// The minimum contribution: the outer size its `min-*` (or automatic minimum) gives.
    pub minimum: f64,
    /// The min-content contribution.
    pub min_content: f64,
    /// The max-content contribution.
    pub max_content: f64,
}
/** The kinds of contribution the intrinsic step distributes, in Blink's order
 * (`GridItemContributionType`). */
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Minimum contributions into the base sizes of intrinsic-minimum tracks.
    IntrinsicMinimums,
    /// Min-content contributions into the base sizes of min- and max-content minimum tracks.
    ContentBasedMinimums,
    /// Max-content contributions into the base sizes of max-content minimum tracks.
    MaxContentMinimums,
    /// Min-content contributions into the growth limits of intrinsic-maximum tracks.
    IntrinsicMaximums,
    /// Max-content contributions into the growth limits of `auto`/max-content maximum tracks.
    MaxContentMaximums,
}
/** One track's state while sizing. `limit` is None while infinite. */
#[derive(Clone, Copy, Debug)]
struct Track {
    sizing: TrackSizing,
    base: f64,
    limit: Option<f64>,
    infinitely_growable: bool,
    planned: Option<f64>,
}
impl Track {
    /** A flexible maximum. */
    fn flexible(&self) -> bool {
        matches!(self.sizing.max, MaxSizing::Fr(_))
    }
    /** The flex factor, zero for non-flexible tracks. */
    fn factor(&self) -> f64 {
        match self.sizing.max {
            MaxSizing::Fr(factor) => factor,
            _ => 0.0,
        }
    }
    /** An intrinsic maximum: `auto`, `min-content`, `max-content` or `fit-content()`. */
    fn intrinsic_max(&self) -> bool {
        !matches!(self.sizing.max, MaxSizing::Fixed(_) | MaxSizing::Fr(_))
    }
    /** A maximum that grows to max-content contributions: `auto`, `max-content`, `fit-content()`. */
    fn max_content_or_auto_max(&self) -> bool {
        matches!(
            self.sizing.max,
            MaxSizing::Auto | MaxSizing::MaxContent | MaxSizing::FitContent(_)
        )
    }
    /** A plain `auto` maximum, the only kind stretched into leftover space. */
    fn auto_max(&self) -> bool {
        self.sizing.max == MaxSizing::Auto
    }
    /** The `fit-content()` argument, when the maximum is one. */
    fn fit_limit(&self) -> Option<f64> {
        match self.sizing.max {
            MaxSizing::FitContent(limit) => Some(limit),
            _ => None,
        }
    }
    /** An intrinsic minimum. */
    fn intrinsic_min(&self) -> bool {
        !matches!(self.sizing.min, MinSizing::Fixed(_))
    }
    /** The growth limit, a still-infinite one standing in as the base size. */
    fn definite_limit(&self) -> f64 {
        self.limit.unwrap_or(self.base)
    }
}

/** The space the tracks of one axis are sized into (Blink's `SizingConstraint` plus the
 * available size): a definite length, or the grid's own min- or max-content size being found. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Space {
    /// Laying out into a definite size.
    Definite(f64),
    /// Indefinite free space: a max-content constraint, also an auto-height grid's rows. `auto`
    /// tracks still stretch up to the container's definite minimum size, this length.
    MaxContent(f64),
    /// A min-content constraint: flexible tracks keep their base sizes (css-grid-2 12.7.1).
    MinContent,
}
impl Space {
    /** The definite size, if any. */
    fn definite(self) -> Option<f64> {
        match self {
            Self::Definite(size) => Some(size),
            _ => None,
        }
    }
}

/** Sizes tracks into `space` with `gap` gutters. `stretch` is whether the container's content
 * distribution in this axis is `normal` or `stretch`, which lets `auto` tracks take the leftover.
 * Returns each track's used size. */
pub(super) fn size_tracks(
    sizing: &[TrackSizing],
    items: &[Contribution],
    space: Space,
    gap: f64,
    stretch: bool,
) -> Vec<f64> {
    let available = space.definite();
    let mut tracks = sizing
        .iter()
        .map(|sizing| initialize(*sizing))
        .collect::<Vec<_>>();
    resolve_intrinsic(&mut tracks, items, gap);
    for track in &mut tracks {
        if track.limit.is_none() {
            track.limit = Some(track.base);
        }
    }
    maximize(&mut tracks, available, gap);
    if space != Space::MinContent {
        expand_flexible(&mut tracks, items, available, gap);
    }
    if stretch {
        let target = match space {
            Space::Definite(size) | Space::MaxContent(size) => Some(size),
            Space::MinContent => None,
        };
        stretch_auto(&mut tracks, target, gap);
    }
    tracks.iter().map(|track| track.base).collect()
}

/** Step 1: a fixed minimum is the base size, a fixed maximum the growth limit, an intrinsic one
 * infinite; a flexible maximum's limit is the base size, and no limit falls below its base.
 * Authored lengths are never negative; only a subgrid's inherited track can be, when its gap is
 * wider than its parent's, and it keeps that size so later lines stay on the parent's. */
fn initialize(sizing: TrackSizing) -> Track {
    let base = match sizing.min {
        MinSizing::Fixed(length) => length,
        MinSizing::Auto | MinSizing::MinContent | MinSizing::MaxContent => 0.0,
    };
    let limit = match sizing.max {
        MaxSizing::Fixed(length) => Some(length.max(base)),
        MaxSizing::Auto
        | MaxSizing::MinContent
        | MaxSizing::MaxContent
        | MaxSizing::FitContent(_) => None,
        MaxSizing::Fr(_) => Some(base),
    };
    Track {
        sizing,
        base,
        limit,
        infinitely_growable: false,
        planned: None,
    }
}

/** Whether an item spans a track with a flexible maximum. */
fn spans_flexible(tracks: &[Track], item: &Contribution) -> bool {
    tracks[item.start..item.start + item.span]
        .iter()
        .any(Track::flexible)
}

/** Step 2: items not spanning a flexible track in groups of increasing span, then every item
 * that does, together, distributing only into the flexible tracks by flex factor. */
fn resolve_intrinsic(tracks: &mut [Track], items: &[Contribution], gap: f64) {
    let mut ordered = items
        .iter()
        .filter(|item| item.span > 0 && item.start + item.span <= tracks.len())
        .filter(|item| {
            tracks[item.start..item.start + item.span]
                .iter()
                .any(|track| track.intrinsic_min() || track.intrinsic_max())
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|item| (spans_flexible(tracks, item), item.span));
    let mut begin = 0;
    while begin < ordered.len() && !spans_flexible(tracks, ordered[begin]) {
        let span = ordered[begin].span;
        let mut end = begin;
        while end < ordered.len()
            && !spans_flexible(tracks, ordered[end])
            && ordered[end].span == span
        {
            end += 1;
        }
        for kind in [
            Kind::IntrinsicMinimums,
            Kind::ContentBasedMinimums,
            Kind::MaxContentMinimums,
            Kind::IntrinsicMaximums,
            Kind::MaxContentMaximums,
        ] {
            accommodate(tracks, &ordered[begin..end], kind, false, gap);
        }
        begin = end;
    }
    if begin < ordered.len() {
        for kind in [
            Kind::IntrinsicMinimums,
            Kind::ContentBasedMinimums,
            Kind::MaxContentMinimums,
        ] {
            accommodate(tracks, &ordered[begin..], kind, true, gap);
        }
    }
}

/** The size of a track a contribution kind grows: the base size, or the growth limit. */
fn affected(track: &Track, kind: Kind) -> f64 {
    if for_base(kind) {
        track.base
    } else {
        track.definite_limit()
    }
}
/** Whether a contribution kind grows base sizes rather than growth limits. */
fn for_base(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::IntrinsicMinimums | Kind::ContentBasedMinimums | Kind::MaxContentMinimums
    )
}
/** Whether a contribution kind applies to a track. */
fn applies(track: &Track, kind: Kind) -> bool {
    match kind {
        Kind::IntrinsicMinimums => track.intrinsic_min(),
        Kind::ContentBasedMinimums => {
            matches!(
                track.sizing.min,
                MinSizing::MinContent | MinSizing::MaxContent
            )
        }
        Kind::MaxContentMinimums => track.sizing.min == MinSizing::MaxContent,
        Kind::IntrinsicMaximums => track.intrinsic_max(),
        Kind::MaxContentMaximums => track.max_content_or_auto_max(),
    }
}
/** The item size a contribution kind distributes. */
fn contribution(item: &Contribution, kind: Kind) -> f64 {
    match kind {
        Kind::IntrinsicMinimums => item.minimum,
        Kind::ContentBasedMinimums | Kind::IntrinsicMaximums => item.min_content,
        Kind::MaxContentMinimums | Kind::MaxContentMaximums => item.max_content,
    }
}
/** How far a track may grow before reaching its limit; None is unbounded (Blink's
 * `GrowthPotentialForSet`). A finite growth limit not marked infinitely growable cannot grow
 * while `honor_growable`; a `fit-content()` limit grows only up to its argument. */
fn growth_potential(track: &Track, kind: Kind, honor_growable: bool) -> Option<f64> {
    if for_base(kind) {
        return track.limit.map(|limit| (limit - track.base).max(0.0));
    }
    if honor_growable && track.limit.is_some() && !track.infinitely_growable {
        return Some(0.0);
    }
    track
        .fit_limit()
        .map(|limit| (limit - track.definite_limit()).max(0.0))
}
/** Whether a track is among those a contribution kind grows past their limits once every
 * affected track is at its limit (Blink's `ShouldUsedSizeGrowBeyondLimit`); None when the
 * kind grows every affected track. */
fn beyond_limit(track: &Track, kind: Kind) -> Option<bool> {
    match kind {
        Kind::IntrinsicMinimums | Kind::ContentBasedMinimums => Some(track.intrinsic_max()),
        Kind::MaxContentMinimums => Some(track.max_content_or_auto_max()),
        Kind::IntrinsicMaximums | Kind::MaxContentMaximums => None,
    }
}

/** "Increase sizes to accommodate items": each item's extra space is distributed over its
 * affected tracks, planned increases take the largest per track, then apply at once. */
fn accommodate(
    tracks: &mut [Track],
    group: &[&Contribution],
    kind: Kind,
    flexible: bool,
    gap: f64,
) {
    for track in tracks.iter_mut() {
        track.planned = None;
    }
    for item in group {
        let range = item.start..item.start + item.span;
        let spanned = tracks[range.clone()]
            .iter()
            .map(|track| affected(track, kind))
            .sum::<f64>()
            + gap * item.span.saturating_sub(1) as f64;
        let grow = range
            .clone()
            .filter(|index| {
                let track = &tracks[*index];
                (!flexible || track.flexible()) && applies(track, kind)
            })
            .collect::<Vec<_>>();
        if grow.is_empty() {
            continue;
        }
        for index in &grow {
            tracks[*index].planned.get_or_insert(0.0);
        }
        let extra = (contribution(item, kind) - spanned).max(0.0);
        if extra <= 0.0 {
            continue;
        }
        let factors = grow
            .iter()
            .map(|index| tracks[*index].factor())
            .sum::<f64>();
        let increases = if flexible && factors > 0.0 {
            grow.iter()
                .map(|index| extra * tracks[*index].factor() / factors)
                .collect::<Vec<_>>()
        } else {
            distribute_equally(tracks, &grow, extra, kind)
        };
        for (index, increase) in grow.iter().zip(increases) {
            let planned = tracks[*index].planned.get_or_insert(0.0);
            *planned = planned.max(increase);
        }
    }
    for track in tracks.iter_mut() {
        let planned = track.planned.take();
        grow_affected(track, kind, planned);
    }
}

/** Applies a planned increase. Every track loses its "infinitely growable" mark; a growth limit
 * that was infinite and grows for intrinsic maximums gains it (css-grid-2 12.5.1, Blink's
 * `GrowAffectedSizeByPlannedIncrease`). */
fn grow_affected(track: &mut Track, kind: Kind, increase: Option<f64>) {
    track.infinitely_growable = false;
    let Some(increase) = increase else {
        return;
    };
    match kind {
        Kind::IntrinsicMinimums | Kind::ContentBasedMinimums | Kind::MaxContentMinimums => {
            track.base += increase;
            if let Some(limit) = track.limit {
                track.limit = Some(limit.max(track.base));
            }
        }
        Kind::IntrinsicMaximums => {
            track.infinitely_growable = track.limit.is_none();
            track.limit = Some(track.definite_limit() + increase);
        }
        Kind::MaxContentMaximums => track.limit = Some(track.definite_limit() + increase),
    }
}

/** Equal distribution (css-grid-2 "distribute extra space"): first up to each track's limit,
 * least growth potential first, then beyond limits. Base sizes go beyond limits only into
 * tracks with an intrinsic maximum when there are any; growth limits into every affected one. */
fn distribute_equally(tracks: &[Track], grow: &[usize], mut extra: f64, kind: Kind) -> Vec<f64> {
    let potential =
        |position: usize, honor: bool| growth_potential(&tracks[grow[position]], kind, honor);
    let mut order = (0..grow.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| match (potential(*a, false), potential(*b, false)) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let mut increases = vec![0.0; grow.len()];
    let growable = order
        .iter()
        .copied()
        .filter(|position| potential(*position, true) != Some(0.0))
        .collect::<Vec<_>>();
    share(&growable, &mut increases, &mut extra, |position| {
        potential(position, true)
    });
    if extra <= 0.0 {
        return increases;
    }
    let mut beyond = order.clone();
    let chosen = order
        .iter()
        .copied()
        .filter(|position| beyond_limit(&tracks[grow[*position]], kind) == Some(true))
        .collect::<Vec<_>>();
    if !chosen.is_empty() {
        beyond = chosen;
    }
    // A `fit-content()` growth limit never passes its argument.
    let room = |position: usize| {
        (!for_base(kind))
            .then(|| growth_potential(&tracks[grow[position]], kind, false))
            .flatten()
    };
    share(&beyond, &mut increases, &mut extra, room);
    increases
}
/** Shares `extra` over `positions` in order, each taking an equal part of what is left, capped
 * by its room (None is unbounded). */
fn share(
    positions: &[usize],
    increases: &mut [f64],
    extra: &mut f64,
    room: impl Fn(usize) -> Option<f64>,
) {
    let mut remaining = positions.len();
    for position in positions {
        if *extra <= 0.0 {
            break;
        }
        let mut part = *extra / remaining as f64;
        if let Some(room) = room(*position) {
            part = part.min(room);
        }
        increases[*position] += part;
        *extra -= part;
        remaining -= 1;
    }
}

/** The free space: the available size less base sizes and gutters, or None when indefinite. */
fn free_space(tracks: &[Track], available: Option<f64>, gap: f64) -> Option<f64> {
    available.map(|available| {
        (available
            - tracks.iter().map(|track| track.base).sum::<f64>()
            - gap * tracks.len().saturating_sub(1) as f64)
            .max(0.0)
    })
}

/** Step 3: positive free space grows every base size equally up to its growth limit; indefinite
 * free space fills every track to its limit. */
fn maximize(tracks: &mut [Track], available: Option<f64>, gap: f64) {
    let Some(mut free) = free_space(tracks, available, gap) else {
        for track in tracks.iter_mut() {
            track.base = track.base.max(track.definite_limit());
        }
        return;
    };
    let mut order = (0..tracks.len()).collect::<Vec<_>>();
    order.sort_by(|a, b| {
        let room = |index: usize| tracks[index].definite_limit() - tracks[index].base;
        room(*a).total_cmp(&room(*b))
    });
    let growable = order
        .iter()
        .filter(|index| tracks[**index].definite_limit() > tracks[**index].base)
        .copied()
        .collect::<Vec<_>>();
    let mut remaining = growable.len();
    for index in growable {
        if free <= 0.0 {
            break;
        }
        let room = tracks[index].definite_limit() - tracks[index].base;
        let part = (free / remaining as f64).min(room);
        tracks[index].base += part;
        free -= part;
        remaining -= 1;
    }
}

/** "Find the size of an fr" over `range` for `space`: flexible tracks whose base size exceeds
 * their share are treated as inflexible, largest base-to-factor ratio first. */
fn find_fr(tracks: &[Track], range: std::ops::Range<usize>, space: f64, gap: f64) -> f64 {
    let mut leftover = space - gap * range.len().saturating_sub(1) as f64;
    let mut factors = 0.0;
    let mut flexible = Vec::new();
    for track in &tracks[range] {
        if track.flexible() && track.factor() > 0.0 {
            factors += track.factor();
            flexible.push(*track);
        } else {
            leftover -= track.base;
        }
    }
    if leftover < 0.0 || flexible.is_empty() {
        return 0.0;
    }
    flexible.sort_by(|a, b| (b.base * a.factor()).total_cmp(&(a.base * b.factor())));
    let mut current = 0;
    while leftover > 0.0 && current < flexible.len() {
        let sum = factors.max(1.0);
        let mut next = current;
        while next < flexible.len()
            && flexible[next].factor() * leftover < flexible[next].base * sum
        {
            next += 1;
        }
        if next == current {
            return leftover / sum;
        }
        for track in &flexible[current..next] {
            factors -= track.factor();
            leftover -= track.base;
        }
        current = next;
    }
    0.0
}

/** Step 4: definite space sizes an fr over every track; indefinite space takes the largest fr
 * any flexible track's base size or any item crossing flexible tracks needs. */
fn expand_flexible(tracks: &mut [Track], items: &[Contribution], available: Option<f64>, gap: f64) {
    if !tracks.iter().any(Track::flexible) || free_space(tracks, available, gap) == Some(0.0) {
        return;
    }
    let fr = match available {
        Some(available) => find_fr(tracks, 0..tracks.len(), available, gap),
        None => {
            let mut fr = 0.0_f64;
            for item in items {
                if item.span > 0
                    && item.start + item.span <= tracks.len()
                    && spans_flexible(tracks, item)
                {
                    let range = item.start..item.start + item.span;
                    fr = fr.max(find_fr(tracks, range, item.max_content, gap));
                }
            }
            for track in tracks.iter().filter(|track| track.flexible()) {
                fr = fr.max(track.base / track.factor().max(1.0));
            }
            fr
        }
    };
    for track in tracks.iter_mut().filter(|track| track.flexible()) {
        track.base = track.base.max(fr * track.factor());
    }
}

/** Step 5: remaining definite free space is shared equally by `auto`-maximum tracks. */
fn stretch_auto(tracks: &mut [Track], available: Option<f64>, gap: f64) {
    let autos = tracks.iter().filter(|track| track.auto_max()).count();
    let Some(free) = free_space(tracks, available, gap) else {
        return;
    };
    if autos == 0 || free <= 0.0 {
        return;
    }
    for track in tracks.iter_mut().filter(|track| track.auto_max()) {
        track.base += free / autos as f64;
    }
}
