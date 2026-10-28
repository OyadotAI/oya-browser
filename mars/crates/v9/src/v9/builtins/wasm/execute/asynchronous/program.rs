/** Structured Wasm control becomes immutable owned steps; source paths retain validation and trap coordinates. */
use super::*;
/** A source path selects the original nested instruction, with body/else selectors on its ancestors. */
pub(super) type Path = Vec<(usize, usize)>;
#[derive(Clone)]
pub(super) enum Code {
    Plain(Instruction),
    Enter(usize),
    Else(usize),
    End(usize),
}
#[derive(Clone)]
pub(super) struct Op {
    pub code: Code,
    pub scopes: Vec<usize>,
    pub path: Path,
    pub position: usize,
}
/** Scope boundaries are derived only from validated AST structure and never accepted as serialized code. */
pub(super) struct Scope {
    pub kind: u8,
    pub signature: Signature,
    pub body: usize,
    pub otherwise: usize,
    pub end: usize,
    pub handlers: Vec<types::Catch>,
    pub legacy: Vec<(Option<u32>, usize)>,
    pub delegate: Option<u32>,
}
pub(super) struct Program {
    pub ops: Vec<Op>,
    pub scopes: Vec<Scope>,
}
impl Program {
    /** Each function has one compiled control layout, independent of its dynamic local values. */
    pub fn compile(code: &[Instruction]) -> crate::protocol::EngineResult<Self> {
        if footprint(code, 0) > BUFFER_BYTES {
            return Err(crate::protocol::EngineError::limit(
                "V9 resumable Wasm control bytes",
                BUFFER_BYTES,
            ));
        }
        let mut program = Self {
            ops: vec![],
            scopes: vec![],
        };
        program.sequence(code, &[], &[]);
        Ok(program)
    }
    fn sequence(&mut self, code: &[Instruction], scopes: &[usize], path: &[(usize, usize)]) {
        for (index, instruction) in code.iter().enumerate() {
            let mut inner = instruction;
            let mut position = 0;
            while let Instruction::Located(at, next) = inner {
                position = *at;
                inner = next;
            }
            let mut source = path.to_vec();
            source.push((index, 2));
            if let Instruction::Try(signature, body, handlers) = inner {
                self.legacy_try(signature, body, handlers, scopes, &source, position);
                continue;
            }
            let scoped = match inner {
                Instruction::Block(kind, signature, body, otherwise) => Some((
                    *kind,
                    signature,
                    body.as_slice(),
                    otherwise.as_slice(),
                    Vec::new(),
                    None,
                )),
                Instruction::TryTable(signature, handlers, body) => Some((
                    2,
                    signature,
                    body.as_slice(),
                    &[][..],
                    handlers.clone(),
                    None,
                )),
                Instruction::Delegate(signature, body, depth) => Some((
                    7,
                    signature,
                    body.as_slice(),
                    &[][..],
                    Vec::new(),
                    Some(*depth),
                )),
                _ => None,
            };
            if let Some((kind, signature, body, otherwise, handlers, delegate)) = scoped {
                let scope = self.scopes.len();
                self.scopes.push(Scope {
                    kind,
                    signature: signature.clone(),
                    handlers,
                    legacy: Vec::new(),
                    delegate,
                    body: 0,
                    otherwise: 0,
                    end: 0,
                });
                self.ops.push(Op {
                    code: Code::Enter(scope),
                    scopes: scopes.to_vec(),
                    path: source.clone(),
                    position,
                });
                self.scopes[scope].body = self.ops.len();
                let mut nested = scopes.to_vec();
                nested.push(scope);
                source.last_mut().unwrap().1 = 0;
                self.sequence(body, &nested, &source);
                if kind == 4 {
                    self.ops.push(Op {
                        code: Code::Else(scope),
                        scopes: nested.clone(),
                        path: source.clone(),
                        position,
                    });
                }
                self.scopes[scope].otherwise = self.ops.len();
                source.last_mut().unwrap().1 = 1;
                self.sequence(otherwise, &nested, &source);
                self.scopes[scope].end = self.ops.len();
                self.ops.push(Op {
                    code: Code::End(scope),
                    scopes: nested,
                    path: source,
                    position,
                });
            } else {
                self.ops.push(Op {
                    code: Code::Plain(instruction.clone()),
                    scopes: scopes.to_vec(),
                    path: source,
                    position,
                });
            }
        }
    }
    /** Flatten legacy try bodies and handlers into the same resumable control graph. */
    fn legacy_try(
        &mut self,
        signature: &Signature,
        body: &[Instruction],
        handlers: &[(Option<u32>, Vec<Instruction>)],
        scopes: &[usize],
        path: &Path,
        position: usize,
    ) {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            kind: 6,
            signature: signature.clone(),
            body: 0,
            otherwise: 0,
            end: 0,
            handlers: Vec::new(),
            legacy: Vec::new(),
            delegate: None,
        });
        self.ops.push(Op {
            code: Code::Enter(id),
            scopes: scopes.to_vec(),
            path: path.clone(),
            position,
        });
        self.scopes[id].body = self.ops.len();
        let mut nested = scopes.to_vec();
        nested.push(id);
        let mut source = path.clone();
        source.last_mut().unwrap().1 = 0;
        self.sequence(body, &nested, &source);
        self.ops.push(Op {
            code: Code::Else(id),
            scopes: nested.clone(),
            path: source.clone(),
            position,
        });
        for (number, (tag, code)) in handlers.iter().enumerate() {
            self.scopes[id].legacy.push((*tag, self.ops.len()));
            source.last_mut().unwrap().1 = number + 1;
            self.sequence(code, &nested, &source);
            self.ops.push(Op {
                code: Code::Else(id),
                scopes: nested.clone(),
                path: source.clone(),
                position,
            });
        }
        self.scopes[id].end = self.ops.len();
        self.ops.push(Op {
            code: Code::End(id),
            scopes: nested,
            path: source,
            position,
        });
    }
    /** The following step determines exactly which control scopes remain active at a suspension point. */
    pub fn active(&self, pc: usize) -> Option<&[usize]> {
        if pc == self.ops.len() {
            Some(&[])
        } else {
            self.ops.get(pc).map(|op| op.scopes.as_slice())
        }
    }
}

/** Source paths and cloned delegated instructions are bounded before allocation, including deeply nested bodies. */
fn footprint(code: &[Instruction], depth: usize) -> usize {
    code.iter().fold(0usize, |total, instruction| {
        let mut inner = instruction;
        while let Instruction::Located(_, next) = inner {
            inner = next;
        }
        let base = 2 * std::mem::size_of::<Op>()
            + 4.max((depth + 1) * 2) * std::mem::size_of::<(usize, usize)>()
            + depth * std::mem::size_of::<usize>();
        let extra = if let Instruction::Block(kind, sig, body, otherwise) = inner {
            2 * std::mem::size_of::<Scope>()
                + (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + base * if *kind == 4 { 2 } else { 1 }
                + footprint(body, depth + 1)
                + footprint(otherwise, depth + 1)
        } else if let Instruction::Try(sig, body, handlers) = inner {
            2 * std::mem::size_of::<Scope>()
                + (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + handlers.len() * (std::mem::size_of::<(Option<u32>, usize)>() + base)
                + footprint(body, depth + 1)
                + handlers
                    .iter()
                    .map(|(_, code)| footprint(code, depth + 1))
                    .sum::<usize>()
        } else if let Instruction::Delegate(sig, body, _) = inner {
            2 * std::mem::size_of::<Scope>()
                + (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + footprint(body, depth + 1)
        } else if let Instruction::TryTable(sig, handlers, body) = inner {
            2 * std::mem::size_of::<Scope>()
                + (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + handlers.len() * std::mem::size_of::<types::Catch>()
                + footprint(body, depth + 1)
        } else {
            delegated_bytes(instruction)
        };
        total.saturating_add(base).saturating_add(extra)
    })
}
/** Cloning an unflattened EH node retains its complete nested AST, which is also real native memory. */
fn delegated_bytes(instruction: &Instruction) -> usize {
    let base = std::mem::size_of::<Instruction>();
    base.saturating_add(match instruction {
        Instruction::Located(_, inner) => delegated_bytes(inner),
        Instruction::Block(_, sig, body, otherwise) => {
            (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + body
                    .iter()
                    .chain(otherwise)
                    .map(delegated_bytes)
                    .sum::<usize>()
        }
        Instruction::Try(sig, body, handlers) => {
            (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + body.iter().map(delegated_bytes).sum::<usize>()
                + handlers
                    .iter()
                    .map(|(_, code)| code.iter().map(delegated_bytes).sum::<usize>())
                    .sum::<usize>()
        }
        Instruction::TryTable(sig, handlers, body) => {
            (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + handlers.len() * std::mem::size_of::<types::Catch>()
                + body.iter().map(delegated_bytes).sum::<usize>()
        }
        Instruction::Delegate(sig, body, _) => {
            (sig.params.len() + sig.results.len()) * std::mem::size_of::<Type>()
                + body.iter().map(delegated_bytes).sum::<usize>()
        }
        Instruction::BranchTable(labels, _) => labels.len() * std::mem::size_of::<u32>(),
        Instruction::Gc(_, args) => args.len() * std::mem::size_of::<i64>(),
        Instruction::Simd(_, args) | Instruction::Bulk(_, args) => {
            args.len() * std::mem::size_of::<u32>()
        }
        Instruction::Constant(types::Constant::Expression(code)) => {
            code.iter().map(delegated_bytes).sum::<usize>()
        }
        _ => 0,
    })
}
