/** Regressions from the 50-site visual parity run against Chromium: each test is one shared root
cause, reduced from the real pages it moved (named on the test). */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at the harness's 1100 px viewport. */
fn layout(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_owned()]).unwrap();
    let display = render(&document, 1100.0).unwrap();
    (document, display)
}
/** The first item an element with this id painted. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap_or_else(|| panic!("#{id} painted nothing"))
}

/** `display: table` with `table-cell` children lays the cells out side by side in one row,
sized like table columns and centered by `vertical-align: middle` (gnu.org's banner and its
supporter form stacked vertically before). */
#[test]
fn css_table_cells_sit_side_by_side() {
    let sheet = "#t{display:table;width:100%}#a,#b{display:table-cell;vertical-align:middle}";
    let markup = "<div id=t><p id=a style='height:60px;margin:0'>left</p>\
        <div id=b>right</div></div>";
    let (document, display) = layout(markup, sheet);
    let (t, a, b) = (
        find(&document, &display, "t"),
        find(&document, &display, "a"),
        find(&document, &display, "b"),
    );
    assert_eq!(t.width, 1084.0);
    assert_eq!((a.y, b.y, b.height), (a.y, a.y, 60.0), "b fills the row");
    let middle = b.lines[0].y - b.y;
    assert!((middle - (60.0 - 18.4) / 2.0).abs() < 1.0, "{middle}");
    assert!(b.x > a.x + a.width - 1.0, "b sits right of a");
    assert!(
        (a.width + b.width - t.width).abs() < 1.0,
        "the cells fill the table"
    );
}

/** CSS `table-row` boxes stack as grid rows with shared column widths. */
#[test]
fn css_table_rows_share_columns() {
    let sheet = ".t{display:table}.r{display:table-row}.c{display:table-cell}";
    let markup = "<div class=t><div class=r><span id=a class=c>a</span><span class=c>b</span></div>\
        <div class=r><span id=c class=c>wider text</span><span id=d class=c>d</span></div></div>";
    let (document, display) = layout(markup, sheet);
    let (a, c, d) = (
        find(&document, &display, "a"),
        find(&document, &display, "c"),
        find(&document, &display, "d"),
    );
    assert_eq!(a.width, c.width, "one column width");
    assert!(d.y > a.y, "second row below the first");
    assert_eq!(d.x, c.x + c.width);
}

/** A `display: table` box without row or cell children is a block (the clearfix pattern). */
#[test]
fn css_table_without_cells_stays_a_block() {
    let (document, display) = layout("<div id=t style='display:table'><p id=p>text</p></div>", "");
    let p = find(&document, &display, "p");
    assert_eq!(p.width, 1084.0);
    assert_eq!(find(&document, &display, "t").width, 1084.0);
}

/** A `display: table` figure is as wide as its image, and its `table-caption` wraps below it at
that width instead of widening the float to the caption's one-line width (every Wikipedia
thumbnail was a third wider than Chromium's). */
#[test]
fn table_caption_wraps_to_the_table_width() {
    let sheet = "figure{display:table;float:right;margin:0}figure>a{display:block}\
        figcaption{display:table-caption;caption-side:bottom}";
    let markup = "<figure id=f><a id=a><img width=250 height=100></a>\
        <figcaption id=c>A caption that is far longer than the image is wide, so it wraps</figcaption>\
        </figure>";
    let (document, display) = layout(markup, sheet);
    let (f, a, c) = (
        find(&document, &display, "f"),
        find(&document, &display, "a"),
        find(&document, &display, "c"),
    );
    assert_eq!(f.width, 250.0);
    assert_eq!(c.width, 250.0);
    assert!(c.lines.len() > 1, "the caption wraps");
    assert!(c.y >= a.y + a.height, "the caption sits below the image");
    assert!(f.height >= a.height + c.height);
}

/** A `<video poster>` paints its poster frame, contained in its own box, in place of the
"not supported" label box (gnu.org's and Wikipedia's videos were empty frames). */
#[test]
fn video_poster_paints_as_the_video_box() {
    let markup =
        "<p><video id=v width=250 height=159 poster='/thumb/demo.jpg' controls></video></p>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let video = document.nodes.iter().find(|n| n.html_id == "v").unwrap();
    let urls = mars_eye::background_urls(&document, "https://example.org/wiki/Page");
    assert_eq!(
        urls,
        [(
            video.local_id,
            "https://example.org/thumb/demo.jpg".to_owned()
        )]
    );
    let display = render(&document, 1100.0).unwrap();
    let frame = find(&document, &display, "v");
    assert_eq!((frame.width, frame.height), (250.0, 159.0));
    assert!(
        frame.lines.is_empty(),
        "no placeholder label over the poster"
    );
    assert!(
        document
            .nodes
            .iter()
            .all(|n| n.parent != Some(video.local_id)),
        "no stand-in box"
    );
}

/** `font-size: 0` hides an element's text and gives it no room, while children that reset the
size still show (rustdoc's copy-path button painted its label as an 8px column of letters). */
#[test]
fn zero_font_size_hides_text() {
    let markup = "<div id=row style='font-size:0'>hidden <span id=s style='font-size:16px'>shown</span></div>\
        <button id=b style='font-size:0;width:33px;height:34px'>Copy item path</button>";
    let (document, display) = layout(markup, "");
    let row = find(&document, &display, "row");
    assert!(row.lines.is_empty(), "{:?}", row.lines);
    assert_eq!(find(&document, &display, "s").lines[0].text, "shown");
    assert!(find(&document, &display, "b").lines.is_empty());
    assert_eq!(document.nodes[0].style.font_size, 0.0);
}

/** A shrink-to-fit box is as wide as the words and spaces line layout places, so its own text
never wraps (Google's "AI Mode" chip broke onto two lines when kerning made the shaped run
narrower than its words). */
#[test]
fn shrink_to_fit_text_stays_on_one_line() {
    for text in ["AI Mode", "To Ya. Te Vo", "Wa, Yo; AV TA", "f. T. V. W. Y."] {
        let markup =
            format!("<span id=s style='display:inline-block;font-family:Helvetica'>{text}</span>");
        let (document, display) = layout(&markup, "");
        let lines = &find(&document, &display, "s").lines;
        assert_eq!(lines.len(), 1, "{text}: {lines:?}");
    }
}

/** Percentage widths resolve to Chromium's 1/64 px LayoutUnit, so floats whose percentages
add up to 100% share one row (python.org's six 16.6666667% tabs wrapped the sixth). */
#[test]
fn percentage_floats_summing_to_full_width_share_a_row() {
    let sheet = "*{box-sizing:border-box}li{float:left;width:16.6666667%;border-left:1px solid}\
        ul{margin:0;padding:0;list-style:none}body{margin:0}";
    let markup = "<ul><li>a<li>b<li>c<li>d<li>e<li id=last>f</ul>";
    let (document, display) = layout(markup, sheet);
    let last = find(&document, &display, "last");
    assert_eq!(last.y, 0.0, "the sixth tab stays on the first row");
}

/** `background-clip: text` gradient headings paint their text in the gradient's colour and no
background box (nodejs.org's "Run JavaScript Everywhere" was a dark slab with invisible text). */
#[test]
fn background_clip_text_colours_the_text() {
    let sheet = "h1{background:linear-gradient(90deg,#ff0000,#00ff00,#0000ff);\
        -webkit-background-clip:text;background-clip:text;color:transparent}";
    let (document, display) = layout("<h1 id=h>Gradient</h1>", sheet);
    let h = find(&document, &display, "h");
    assert_eq!(h.color.as_deref(), Some("#00ff00"));
    assert!(h.background.is_none());
    assert!(document.nodes[0].style.backgrounds.is_empty());
}

/** Adjacent inline elements touch unless source whitespace separates them, and `<pre>` keeps
whitespace-only text between highlighted tokens, newlines included (nodejs.org's code sample
read `res . writeHead` on one run-together line). */
#[test]
fn inline_elements_are_spaced_only_by_source_whitespace() {
    let markup = "<p><b id=a>aaa</b><i id=b>bbb</i> <i id=c>ccc</i></p>\
        <pre><span id=k>const</span> <span id=v>server</span>\n<span id=n>next</span>\n</pre>";
    let (document, display) = layout(markup, "");
    let (a, b, c) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
        find(&document, &display, "c"),
    );
    assert_eq!(b.x, a.x + a.width, "no space between adjacent elements");
    assert!(c.x > b.x + b.width + 2.0, "source whitespace spaces them");
    let (k, v, n) = (
        find(&document, &display, "k"),
        find(&document, &display, "v"),
        find(&document, &display, "n"),
    );
    assert!(
        v.x > k.x + k.width + 2.0,
        "the space between tokens is kept"
    );
    assert!(n.y > k.y, "the newline between lines is kept");
    assert_eq!(n.x, k.x);
}

/** A trailing newline in `<pre>` adds no blank line. */
#[test]
fn pre_trailing_newline_adds_no_line() {
    let (document, display) = layout("<pre id=one>a\n</pre><pre id=two>a\nb</pre>", "");
    let (one, two) = (
        find(&document, &display, "one"),
        find(&document, &display, "two"),
    );
    assert_eq!(two.height, 2.0 * one.height);
}

/** Whitespace-only text between grid items in a `<pre>` makes no grid item. */
#[test]
fn grid_skips_whitespace_between_items_in_pre() {
    let markup = "<pre><code style='display:grid'><span id=a>one</span>\n<span id=b>two</span>\n</code></pre>";
    let (document, display) = layout(markup, "");
    let (a, b) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
    );
    assert_eq!(
        b.y,
        a.y + a.height,
        "consecutive rows, no blank row between"
    );
}

/** A button whose page set its font and edges is those edges around its title in that font
(Tailwind's preflight `font: inherit; padding: 0; border: 0` buttons, such as nodejs.org's code
tokens, were native-sized: wider and 21px tall). */
#[test]
fn styled_button_takes_its_authored_font_and_edges() {
    let sheet = "button{font:inherit;padding:0;border:0;background:transparent}\
        p{font:16px/20px monospace}#padded{padding:10px 20px;border:1px solid}";
    let markup = "<p><span id=t>server</span> <button id=b>server</button> \
        <button id=padded>server</button></p>";
    let (document, display) = layout(markup, sheet);
    let (t, b, padded) = (
        find(&document, &display, "t"),
        find(&document, &display, "b"),
        find(&document, &display, "padded"),
    );
    assert!(
        (b.width - t.width).abs() < 0.01,
        "{} vs {}",
        b.width,
        t.width
    );
    assert_eq!(b.height, 20.0);
    assert!(
        b.control_height.is_none(),
        "drawn as a CSS box, not a native title"
    );
    assert_eq!(b.lines[0].text, "server");
    assert!((padded.width - (t.width + 42.0)).abs() < 0.01);
    assert_eq!(padded.height, 42.0);
}

/** A text field whose page set its padding is one line plus that padding tall. */
#[test]
fn padded_text_field_grows_with_its_padding() {
    let sheet = "input{padding:10px;border:1px solid;font:16px/20px sans-serif}";
    let (document, display) = layout("<input id=i>", sheet);
    assert_eq!(find(&document, &display, "i").height, 42.0);
}

/** The space a text run ends with separates it from a following inline-block or image
(Google's "It's Drug Take Back Day: " ran into its inline-block link). */
#[test]
fn trailing_space_separates_an_atomic_inline() {
    let markup = "<p><span id=t>Day: </span><a id=a style='display:inline-block'>Find</a>\
        <span id=u>x</span><img id=i width=10 height=10></p>";
    let (document, display) = layout(markup, "");
    let (t, a, u, i) = (
        find(&document, &display, "t"),
        find(&document, &display, "a"),
        find(&document, &display, "u"),
        find(&document, &display, "i"),
    );
    assert!(
        a.x > t.lines[0].x + t.lines[0].width + 2.0,
        "a space before the inline-block"
    );
    assert_eq!(i.x, u.x + u.width, "no space where the source has none");
}

/** An installed family the page names shapes with that font, in page order before the generic
(Hacker News's Verdana was drawn in Helvetica, narrower than Chromium's). */
#[cfg(target_os = "macos")]
#[test]
fn installed_family_shapes_with_the_named_font() {
    let markup = "<span id=v style='font-family:Verdana,Geneva,sans-serif'>Hacker News rows</span><br>\
        <span id=h style='font-family:sans-serif'>Hacker News rows</span><br>\
        <span id=g style='font-family:-apple-system,Verdana'>Hacker News rows</span>";
    let (document, display) = layout(markup, "");
    let width = |id| find(&document, &display, id).width;
    assert!(
        width("v") > width("h") + 5.0,
        "{} vs {}",
        width("v"),
        width("h")
    );
    assert!(
        (width("g") - width("v")).abs() > 1.0,
        "a generic first wins"
    );
}

/** A whitespace-only spacer element still separates the runs around it (LinkedIn's
"LinkedIn's<span> </span><a>User Agreement</a>" ran together). */
#[test]
fn whitespace_spacer_element_keeps_its_space() {
    let markup = "<p><span id=t>agree to</span><span> </span><a id=a href=#>User Agreement</a></p>";
    let (document, display) = layout(markup, "");
    let (t, a) = (
        find(&document, &display, "t"),
        find(&document, &display, "a"),
    );
    assert!(a.x > t.x + t.width + 2.0, "{} {}", a.x, t.x + t.width);
}

/** In quirks mode the first heading's UA top margin collapses away at the top of the body and
of a table cell, as in Chromium; a standards page keeps it (info.cern.ch sat 21px low). */
#[test]
fn quirks_mode_drops_the_first_ua_top_margin() {
    let quirky = "<header><nextid n=55></nextid></header><h1 id=h>World Wide Web</h1><p id=p>text</p>\
        <table><tr><td><p id=c>cell</p></td></tr></table>";
    let (document, display) = layout(quirky, "");
    assert!(document.formatting.quirks);
    assert_eq!(find(&document, &display, "h").y, 8.0);
    assert!(
        find(&document, &display, "p").y > 8.0 + 37.0,
        "later margins stay"
    );
    let cell = document
        .nodes
        .iter()
        .find(|node| node.html_id == "c")
        .unwrap();
    assert!(cell.style.quirk_top);
    assert_eq!(cell.style.margin[0], 16.0, "the computed margin stays");
    let (positioned, display) = layout("<p id=a style='position:absolute'>x</p>", "");
    assert_eq!(
        find(&positioned, &display, "a").y,
        24.0,
        "out of flow keeps its margin"
    );
    let (standard, display) = layout(&format!("<!DOCTYPE html>{quirky}"), "");
    assert!(!standard.formatting.quirks);
    assert!(find(&standard, &display, "h").y > 8.0);
}

/** Text exactly as wide as its box, give or take float rounding, stays on one line. */
#[test]
fn text_overflowing_by_rounding_does_not_wrap() {
    let (document, display) = layout("<span id=probe>Watch Live</span>", "");
    let width = find(&document, &display, "probe").width;
    let markup = format!(
        "<div id=d style='width:{}px'>Watch Live</div>",
        width - 0.001
    );
    let (document, display) = layout(&markup, "");
    assert_eq!(find(&document, &display, "d").lines.len(), 1);
}

/** A sticky box's `top` is a threshold: below it the box stays in flow, above it the box moves
down to it, never past its parent (BBC's `top: 80px` navigation sat 80px under its header). */
#[test]
fn sticky_top_is_a_threshold_not_an_offset() {
    let markup = "<body style='margin:0'><header style='height:80px'>h</header>\
        <nav id=n style='position:sticky;top:80px;height:40px'>nav</nav>\
        <div style='height:200px'><p id=p style='position:sticky;top:30px;margin:0'>pinned</p></div></body>";
    let (document, display) = layout(markup, "");
    assert_eq!(find(&document, &display, "n").y, 80.0);
    assert_eq!(
        find(&document, &display, "p").y,
        120.0,
        "its flow position is already below 30px"
    );
    let (document, display) = layout(
        "<body style='margin:0'><p id=q style='position:sticky;top:30px;margin:0'>pinned</p></body>",
        "",
    );
    let q = find(&document, &display, "q");
    assert_eq!(q.y, 0.0, "the body ends at its bottom, so it cannot move");
}

/** A shrink-to-fit table cell is as wide as its inline content on one line, own text and spaces
included (Hacker News titles with a " (site)" suffix wrapped where Chromium keeps one line). */
#[test]
fn table_cell_max_content_counts_inline_text_and_spaces() {
    let markup = "<table><tr><td id=c><span><a href=#>John Carmack on the Goose Language</a>\
        <span> (<a href=#><span>twitter.com/id_aa_carmack</span></a>)</span></span></td></tr></table>";
    let (document, display) = layout(markup, "");
    let cell = find(&document, &display, "c");
    assert!(
        cell.height < 25.0,
        "one line plus the cell padding: {}",
        cell.height
    );
}

/** `display: flow-root` contains its floats, so later blocks start below them (IANA's header
stayed 50px tall and its floated navigation squeezed the article beside it). */
#[test]
fn flow_root_contains_its_floats() {
    let markup = "<div id=h style='display:flow-root'><div style='float:right;height:60px;width:300px'></div></div>\
        <article id=a style='display:flex'>content</article>";
    let (document, display) = layout(markup, "");
    let (h, a) = (
        find(&document, &display, "h"),
        find(&document, &display, "a"),
    );
    assert_eq!(h.height, 60.0);
    assert_eq!((a.y, a.width), (h.y + 60.0, 1084.0));
}

/** A video's width and height attributes give it a ratio, so `width: 100%; height: auto` keeps
its shape (gnu.org's video collapsed to nothing). */
#[test]
fn video_attributes_keep_their_ratio_under_auto_height() {
    let markup = "<div style='width:512px'><video id=v width=512 height=288 poster=/p.jpg \
        style='width:100%;height:auto'></video></div>";
    let (document, display) = layout(markup, "");
    let video = find(&document, &display, "v");
    assert_eq!((video.width, video.height), (512.0, 288.0));
}

/** A shrink-to-fit grid counts its column gaps, so its last item fits (BBC's floated
"icon LIVE" label overflowed into the headline beside it). */
#[test]
fn shrink_to_fit_grid_counts_its_gaps() {
    let sheet = ".w{display:grid;grid-template-columns:auto auto;gap:10px;float:left}\
        .w>i{display:block;width:20px;height:20px}";
    let markup = "<div id=w class=w><i></i><span id=l>LIVE</span></div>";
    let (document, display) = layout(markup, sheet);
    let (w, l) = (
        find(&document, &display, "w"),
        find(&document, &display, "l"),
    );
    assert!(
        (w.x + w.width - (l.x + l.width)).abs() < 0.01,
        "the label ends at the grid's edge"
    );
    assert_eq!(l.x - w.x, 30.0);
}

/** An auto-width svg with `height: 100%` in a definite-height grid takes the row's height and
its viewBox ratio (BBC's NEWS logo, 26px tall, was drawn 300px wide and 78px tall). */
#[test]
fn grid_svg_percentage_height_resolves_against_the_row() {
    let markup = "<h1 style='display:grid;place-items:center;height:26px'>\
        <svg id=s style='width:auto;height:100%' viewBox='0 0 104 27'><path d='M0 0H104V27Z'/></svg></h1>";
    let (document, display) = layout(markup, "");
    let s = find(&document, &display, "s");
    assert_eq!(s.height, 26.0);
    assert!((s.width - 26.0 * 104.0 / 27.0).abs() < 0.01, "{}", s.width);
}

/** Items placed by named grid lines land between those lines (Notion's hero placed every region
by `content-start / content-end` and they all fell into one auto column). */
#[test]
fn named_grid_lines_place_items() {
    let sheet = ".g{display:grid;grid-template-columns:[full-start] 100px [content-start] 1fr \
        [content-end] 100px [full-end]}.c{grid-column:content-start/content-end}\
        .f{grid-column:full}.t{grid-column:content}";
    let markup =
        "<div class=g><p id=c class=c>c</p><p id=f class=f>f</p><p id=t class=t>t</p></div>";
    let (document, display) = layout(markup, sheet);
    let (c, f, t) = (
        find(&document, &display, "c"),
        find(&document, &display, "f"),
        find(&document, &display, "t"),
    );
    assert_eq!((c.x, c.width), (108.0, 884.0));
    assert_eq!((f.x, f.width), (8.0, 1084.0));
    assert_eq!((t.x, t.width), (108.0, 884.0));
}

/** Nesting output `:is(.a .b) h1` applies like `.a .b h1` (MDN's page title stayed bold because
the whole rule was dropped); `:where()` keeps zero specificity, so a later plain rule wins. */
#[test]
fn complex_is_arguments_apply() {
    let sheet = ":is(.layout .header) h1{font-weight:400}\
        :where(.layout .header) p{color:#ff0000}p{color:#0000ff}";
    let markup = "<div class=layout><div class=header><h1 id=h>Title</h1><p id=p>x</p></div></div>";
    let (document, _) = layout(markup, sheet);
    let style = |id: &str| {
        &document
            .nodes
            .iter()
            .find(|n| n.html_id == id)
            .unwrap()
            .style
    };
    assert!(!style("h").bold && style("h").font_weight == 400);
    assert_eq!(style("p").color.as_deref(), Some("#0000ff"));
}

/** A floated `display: table` whose content is more than one box still sizes to that content,
not its caption's one-line width (Wikipedia's video thumbnail has a magnify link beside it). */
#[test]
fn table_caption_does_not_widen_a_table_with_several_boxes() {
    let sheet = "figure{display:table;float:right;margin:0}figure>a{display:block}\
        figcaption{display:table-caption;caption-side:bottom}";
    let markup = "<figure id=f><span><img width=250 height=100></span><a href=#></a>\
        <figcaption>A caption that is far longer than the image is wide, so it wraps</figcaption>\
        </figure>";
    let (document, display) = layout(markup, sheet);
    assert!(find(&document, &display, "f").width < 300.0);
}

/** An empty `::before` inline-block opens a line box, so its element is one line tall. */
#[test]
fn empty_generated_inline_block_opens_a_line() {
    let sheet = ".l::before{content:\"\";display:inline-block}";
    let (document, display) = layout("<div class=l id=a></div><div id=b>x</div>", sheet);
    let (a, b) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
    );
    assert_eq!(a.height, b.height);
    assert_eq!(b.y, a.y + a.height);
}

/** A disc marker sits two thirds of an ascent plus 8px before the content, as Chromium places
it, not hugging the text (lite.cnn.com's bullets sat 8px from their links instead of 16px). */
#[test]
fn disc_marker_sits_clear_of_the_content() {
    let (document, display) = layout("<ul><li id=i>item</li></ul>", "");
    let item = find(&document, &display, "i");
    let marker = item
        .lines
        .iter()
        .find(|line| line.text == "\u{2022}")
        .unwrap();
    let gap = item.x - marker.x;
    assert!((14.0..19.0).contains(&gap), "{gap}");
}
