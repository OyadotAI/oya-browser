/** Catch binding records reject duplicates and body redeclarations before any catch code executes. */
use super::*;
use crate::v9::syntax::Pattern;

/** Only a simple catch identifier admits Annex B var redeclarations in its body. */
pub(super) fn validate(param: &Pattern, names: &[Name], body: &[Stmt]) -> EngineResult<()> {
    let mut seen = crate::v9::vm::heap::FastSet::default();
    let duplicate = names.iter().any(|name| !seen.insert(name));
    let lexical = scan::lexical_names(body, true)
        .iter()
        .any(|(name, _)| names.contains(name));
    let mut vars = Vec::new();
    if !matches!(param, Pattern::Ident(..)) {
        scan::var_names(body, true, true, &mut vars);
    }
    if duplicate || lexical || vars.iter().any(|name| names.contains(name)) {
        return Err(errors::syntax(
            position(param).location(),
            "SyntaxError: Identifier has already been declared",
        ));
    }
    Ok(())
}

/** Binding positions keep duplicate errors on the offending catch parameter. */
fn position(pattern: &Pattern) -> Pos {
    match pattern {
        Pattern::Ident(_, pos) | Pattern::Object(_, _, pos) | Pattern::Array(_, _, pos) => *pos,
        Pattern::Expr(value) => value.pos,
        Pattern::Default(target, _) => position(target),
    }
}
