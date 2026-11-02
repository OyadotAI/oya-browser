/** Static table validation derives bounded row/cell relationships from source order. */
use super::{Document, Node, TableCell};
use crate::{
    constants::{DIAGNOSTICS, TABLE_CELLS, TABLE_COLUMNS, TABLE_ROWS},
    protocol::EngineResult,
};
use std::collections::BTreeMap;

/** Direct child positions by parent position, built once so every table check stays linear. */
type Children = Vec<Vec<usize>>;

/** Table relationships are annotated best effort: structure outside the static subset is laid
out as the tree builder repaired it and noted once, never refused. Only the hard row, column and
cell bounds still fail. */
pub(super) fn annotate(document: &mut Document) -> EngineResult<()> {
    let kids = index_children(document);
    for message in irregularities(document, &kids) {
        if document.diagnostics.len() < DIAGNOSTICS
            && !document.diagnostics.iter().any(|d| d == message)
        {
            document.diagnostics.push(message.into());
        }
    }
    let mut rows = BTreeMap::<usize, Vec<usize>>::new();
    for row in ids(document, "tr") {
        if let Some(table) = closest_table(document, row) {
            rows.entry(table).or_default().push(row);
        }
    }
    for table in ids(document, "table") {
        let rows = rows.remove(&table).unwrap_or_default();
        annotate_table(document, &kids, table, &rows)?;
    }
    Ok(())
}

/** One walk records each node under its parent, in source order. */
fn index_children(document: &Document) -> Children {
    let mut kids = vec![Vec::new(); document.nodes.len()];
    for (index, node) in document.nodes.iter().enumerate() {
        if let Some(list) = node.parent.and_then(|parent| kids.get_mut(parent)) {
            list.push(index);
        }
    }
    kids
}

/** The value-free notes for structure the grid lays out approximately. */
fn irregularities(document: &Document, kids: &Children) -> Vec<&'static str> {
    let mut found = Vec::new();
    for node in &document.nodes {
        if node.tag == "table" && ancestor(document, node, "table") {
            found.push("Nested tables are laid out inside their cells.");
        }
        if table_tag(&node.tag) && !valid_parent(document, node) {
            found.push("Table parts outside their usual parents are laid out best effort.");
        }
        if spanning(node) {
            found.push("Table cell spans are laid out as single cells.");
        }
    }
    let unexpected = |parent: usize, allowed: &[&str]| {
        kids[parent]
            .iter()
            .any(|child| !allowed.contains(&document.nodes[*child].tag.as_str()))
    };
    let tables = ids(document, "table");
    let groups = ["thead", "tbody", "tfoot"]
        .iter()
        .flat_map(|tag| ids(document, tag));
    if tables.iter().any(|table| {
        unexpected(
            *table,
            &[
                "caption", "colgroup", "col", "thead", "tbody", "tfoot", "tr", "form",
            ],
        )
    }) || groups
        .into_iter()
        .any(|group| unexpected(group, &["tr", "form"]))
        || ids(document, "tr")
            .into_iter()
            .any(|row| unexpected(row, &["th", "td", "form"]))
    {
        found.push("Content inside table structure that is not a row or cell is not laid out.");
    }
    if tables
        .iter()
        .any(|table| duplicate_groups(document, kids, *table))
    {
        found.push("Repeated table captions, headers or footers are laid out in source order.");
    }
    found
}

/** Table children normally use the direct structural parents HTML gives them. */
fn valid_parent(document: &Document, node: &Node) -> bool {
    let parent = node
        .parent
        .and_then(|id| document.nodes.get(id))
        .map(|parent| parent.tag.as_str());
    match node.tag.as_str() {
        "caption" | "thead" | "tbody" | "tfoot" => parent == Some("table"),
        "tr" => [Some("table"), Some("thead"), Some("tbody"), Some("tfoot")].contains(&parent),
        "th" | "td" => parent == Some("tr"),
        _ => true,
    }
}

/** Spans and unknown header scopes are laid out as one ordinary cell. */
fn spanning(node: &Node) -> bool {
    ["th", "td"].contains(&node.tag.as_str())
        && (["rowspan", "colspan"].iter().any(|name| {
            node.attributes
                .get(*name)
                .is_some_and(|value| value.trim() != "1")
        }) || node
            .attributes
            .get("scope")
            .is_some_and(|scope| !["row", "col"].contains(&scope.as_str())))
}

/** Each table's cells learn their row, column and headers, within independent bounds. A table
past them (a long Hacker News thread, a data dump) still parses and lays out like any other; it
only goes without header associations, as a browser never refuses a page for a large table. */
fn annotate_table(
    document: &mut Document,
    kids: &Children,
    table: usize,
    rows: &[usize],
) -> EngineResult<()> {
    let cells = rows
        .iter()
        .map(|row| children(document, kids, *row))
        .collect::<Vec<_>>();
    let oversized = rows.len() > TABLE_ROWS
        || cells.iter().map(Vec::len).sum::<usize>() > TABLE_CELLS
        || cells.iter().any(|row| row.len() > TABLE_COLUMNS);
    if oversized {
        let message = "A large table's cells were not associated with their headers.";
        if document.diagnostics.len() < DIAGNOSTICS
            && !document.diagnostics.iter().any(|d| d == message)
        {
            document.diagnostics.push(message.into());
        }
        return Ok(());
    }
    annotate_cells(document, table, rows, &cells)
}

/** Duplicate caption and header/footer groups make source associations approximate. */
fn duplicate_groups(document: &Document, kids: &Children, table: usize) -> bool {
    ["caption", "thead", "tfoot"].iter().any(|tag| {
        kids[table]
            .iter()
            .filter(|child| document.nodes[**child].tag == *tag)
            .count()
            > 1
    })
}

/** Header associations use explicit row/column scope or the first header row. */
fn annotate_cells(
    document: &mut Document,
    table: usize,
    rows: &[usize],
    cells: &[Vec<usize>],
) -> EngineResult<()> {
    let mut column_headers = BTreeMap::<usize, Vec<usize>>::new();
    let first_header_row = rows.iter().zip(cells).position(|(row, cells)| {
        !cells.is_empty()
            && (closest_tag(document, *row, "thead", table)
                || cells.iter().all(|id| document.nodes[*id].tag == "th"))
    });
    for (row_index, row_cells) in cells.iter().enumerate() {
        for (column_index, id) in row_cells.iter().enumerate() {
            if is_column_header(document, *id, row_index, table, first_header_row) {
                column_headers.entry(column_index).or_default().push(*id);
            }
        }
    }
    for (row_index, (row, row_cells)) in rows.iter().zip(cells).enumerate() {
        let row_headers = row_cells
            .iter()
            .copied()
            .filter(|id| {
                document.nodes[*id].tag == "th"
                    && document.nodes[*id]
                        .attributes
                        .get("scope")
                        .is_some_and(|scope| scope == "row")
            })
            .collect::<Vec<_>>();
        for (column_index, id) in row_cells.iter().enumerate() {
            let column = column_headers
                .get(&column_index)
                .map_or(&[][..], Vec::as_slice);
            let headers = headers_for(document, *id, column, &row_headers);
            document.nodes[*id].table_cell = Some(TableCell {
                table,
                row: *row,
                row_index: row_index + 1,
                column_index: column_index + 1,
                headers,
            });
        }
    }
    Ok(())
}

/** Column headers apply down their column; explicit row headers apply across the row. */
fn headers_for(document: &Document, cell: usize, column: &[usize], rows: &[usize]) -> Vec<usize> {
    let mut result = column
        .iter()
        .copied()
        .filter(|id| *id != cell)
        .collect::<Vec<_>>();
    result.extend(rows.iter().copied().filter(|id| *id != cell));
    if document.nodes[cell].tag == "th" {
        result.retain(|id| *id != cell);
    }
    result
}

/** A header row is recognized by the enclosing thead or inferred from first-row th cells. */
fn is_column_header(
    document: &Document,
    id: usize,
    row: usize,
    table: usize,
    first: Option<usize>,
) -> bool {
    let node = &document.nodes[id];
    node.tag == "th"
        && match node.attributes.get("scope").map(String::as_str) {
            Some("col") => true,
            Some("row") => false,
            None => first == Some(row) || closest_tag(document, id, "thead", table),
            _ => false,
        }
}

/** Group membership is bounded to the nearest containing table. */
fn closest_tag(document: &Document, row: usize, tag: &str, table: usize) -> bool {
    let mut parent = document.nodes[row].parent;
    while let Some(id) = parent {
        if id == table {
            return false;
        }
        if document.nodes[id].tag == tag {
            return true;
        }
        parent = document.nodes[id].parent;
    }
    false
}

/** Table identity for a row follows parent links, never page-controlled HTML IDs. */
fn closest_table(document: &Document, node: usize) -> Option<usize> {
    let mut parent = document.nodes[node].parent;
    while let Some(id) = parent {
        if document.nodes[id].tag == "table" {
            return Some(id);
        }
        parent = document.nodes[id].parent;
    }
    None
}

/** Parent ancestry checks do not recurse through untrusted markup depth. */
fn ancestor(document: &Document, node: &Node, tag: &str) -> bool {
    let mut parent = node.parent;
    while let Some(id) = parent {
        if document.nodes[id].tag == tag {
            return true;
        }
        parent = document.nodes[id].parent;
    }
    false
}

/** Direct children preserve source order and stable local node IDs. */
fn children(document: &Document, kids: &Children, parent: usize) -> Vec<usize> {
    kids[parent]
        .iter()
        .map(|child| &document.nodes[*child])
        .filter(|node| ["td", "th"].contains(&node.tag.as_str()))
        .map(|node| node.local_id)
        .collect()
}

/** Source traversal retains a stable bounded identity vector. */
fn ids(document: &Document, tag: &str) -> Vec<usize> {
    document
        .nodes
        .iter()
        .filter(|node| node.tag == tag)
        .map(|node| node.local_id)
        .collect()
}

/** Structural tags are validated even when author markup placed them outside a table. */
fn table_tag(tag: &str) -> bool {
    [
        "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
    ]
    .contains(&tag)
}
