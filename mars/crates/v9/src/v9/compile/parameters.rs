/** Parameter expressions capture their own environment before body var bindings are created. */
use super::*;
use crate::v9::syntax::{Pattern, PropKey};

/** Parameter defaults and computed binding keys can create closures or direct-eval declarations. */
pub(super) fn expressions(func: &Func) -> bool {
    func.params
        .iter()
        .chain(func.rest.iter())
        .any(pattern_expression)
}
/** Destructuring alone does not require the separate environment unless it evaluates a key or default. */
fn pattern_expression(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Default(..) => true,
        Pattern::Array(items, rest, _) => {
            items.iter().flatten().any(pattern_expression)
                || rest.as_deref().is_some_and(pattern_expression)
        }
        Pattern::Object(properties, rest, _) => {
            properties.iter().any(|property| {
                matches!(property.key, PropKey::Computed(_)) || pattern_expression(&property.value)
            }) || rest.as_deref().is_some_and(pattern_expression)
        }
        _ => false,
    }
}

impl Compiler {
    /** Body vars copy actual parameter values into distinct cells; parameter-created closures retain the original cells. */
    pub(super) fn parameter_body(
        &mut self,
        func: &Func,
        params: &[Name],
        vars: &[Name],
    ) -> EngineResult<Option<(Slot, bool, Option<usize>)>> {
        if !expressions(func) {
            return Ok(None);
        }
        let mark = self.f().registers;
        let env = self.scopes.last().is_some_and(|scope| scope.env);
        self.push_scope(env, true);
        let push = env.then(|| {
            self.f().envs += 1;
            self.emit(Op::PushEnv(0))
        });
        let uses = scan::function_uses(func);
        if !func.strict && uses.eval {
            self.declare(&Name::from(super::eval::EXT), Declared::Hidden)?;
        }
        let functions = block_body(func)
            .map(scan::function_declarations)
            .unwrap_or_default();
        for name in vars {
            let parameter_binding = params.contains(name)
                || (&**name == "arguments" && !func.flags.arrow && uses.arguments);
            if parameter_binding
                && !functions
                    .iter()
                    .any(|function| function.name.as_ref() == Some(name))
            {
                self.load_name(name, func.pos)?;
            } else {
                self.emit(Op::Undefined);
            }
            let place = self.declare(name, Declared::Var)?;
            self.set_place(place, 0);
            self.emit(Op::Pop);
        }
        Ok(Some((mark, env, push)))
    }
    /** Compiler scope closure matches the body environment without changing captured parameter records. */
    pub(super) fn leave_parameter_body(&mut self, scope: Option<(Slot, bool, Option<usize>)>) {
        let Some((mark, env, push)) = scope else {
            return;
        };
        let scope = self.scopes.pop().expect("parameter body scope");
        if env {
            if let Some(push) = push {
                self.f().ops[push] = Op::PushEnv(scope.slots);
            }
            self.emit(Op::PopEnv);
            self.f().envs -= 1;
        }
        self.release(mark);
    }
}
