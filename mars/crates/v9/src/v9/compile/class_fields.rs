/** Computed field keys execute once during class definition and remain in its captured lexical environment. */
use super::*;
use crate::v9::syntax::{Class, ClassMember, PropKey, StmtKind};

/** A computed field key shares source ordering with method definitions. */
pub(super) enum Element<'a> {
    Method(&'a ClassMember),
    Field(&'a Expr),
}

/** Hidden names cannot collide with authored identifiers and are stable when the source is recompiled. */
pub(super) fn binding(key: &Expr) -> Name {
    Name::from(format!("%field-key{}", key.pos.offset).as_str())
}

/** A computed auto-accessor shares its captured public name with its private backing initializer. */
pub(super) fn auto_binding(backing: &str) -> Name {
    Name::from(format!("%accessor-key{backing}").as_str())
}

/** Direct field statements exclude static-block statements and nested class definitions. */
pub(super) fn keys(class: &Class) -> Vec<&Expr> {
    class
        .fields
        .iter()
        .chain(class.statics.iter())
        .filter_map(|function| block_body(function))
        .flatten()
        .filter_map(|statement| match &statement.kind {
            StmtKind::Field(PropKey::Computed(key), _) => Some(key.as_ref()),
            _ => None,
        })
        .collect()
}

/** Methods and computed fields retain their source order even though their initializers run at different times. */
pub(super) fn elements(class: &Class) -> Vec<Element<'_>> {
    let mut elements: Vec<_> = class
        .members
        .iter()
        .map(Element::Method)
        .chain(keys(class).into_iter().map(Element::Field))
        .collect();
    elements.sort_by_key(|element| match element {
        Element::Method(member) => member.pos.offset,
        Element::Field(key) => key.pos.offset,
    });
    elements
}
