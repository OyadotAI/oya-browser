/** Static operand contracts and mutability/storage checks for the supported Wasm GC instructions. */
use super::*;

/** The abstract root determines which reference hierarchy may be tested or cast. */
pub(super) fn root(m: &Module, heap: i64) -> Result<Type, Error> {
    if heap >= 0 {
        return Ok(match m.definitions.get(heap as usize).map(|d| &d.kind) {
            Some(Kind::Function(_)) => Type::Func,
            Some(_) => Type::Ref(-18, true),
            None => return Err(Error::invalid("unknown cast heap type")),
        });
    }
    Ok(match heap {
        -16 | -13 => Type::Func,
        -17 | -14 => Type::Extern,
        -25 | -22..=-18 | -15 => Type::Ref(-18, true),
        -23 | -12 => Type::Ref(-23, true),
        -37 | -36 => Type::Shared(-36, true),
        -32 | -31 | -26 => Type::Ref(heap, false),
        _ => return Err(Error::unsupported("WebAssembly cast reference hierarchy")),
    })
}
/** Every supported GC opcode has exact operand and result types checked before execution. */
pub(super) fn signature(
    m: &Module,
    op: u32,
    args: &[i64],
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    use Type::*;
    let index = args.first().copied().unwrap_or(0) as u32;
    let reference = Ref(i64::from(index), true);
    Ok(match op {
        0 | 1 => {
            let fields = gc_types::fields(m, index)?;
            if op == 1 && fields.iter().any(|f| !gc_types::defaultable(f.ty)) {
                return Err(Error::invalid(
                    "struct.new_default requires defaultable fields",
                ));
            }
            (
                if op == 0 {
                    fields.iter().map(|f| f.ty).collect()
                } else {
                    vec![]
                },
                vec![Ref(i64::from(index), false)],
            )
        }
        2..=5 => {
            let field = gc_types::fields(m, index)?
                .get(args[1] as usize)
                .ok_or_else(|| Error::invalid("unknown Wasm struct field"))?;
            if op == 5 {
                if !field.mutable {
                    return Err(Error::invalid("struct.set requires a mutable field"));
                }
                (vec![reference, field.ty], vec![])
            } else {
                if (op == 2) != field.packed.is_none() {
                    return Err(Error::invalid("struct.get packed storage mismatch"));
                }
                (vec![reference], vec![field.ty])
            }
        }
        6 | 7 => {
            let field = gc_types::element(m, index)?;
            if op == 7 && !gc_types::defaultable(field.ty) {
                return Err(Error::invalid(
                    "array.new_default requires defaultable element",
                ));
            }
            (
                if op == 6 {
                    vec![field.ty, I32]
                } else {
                    vec![I32]
                },
                vec![Ref(i64::from(index), false)],
            )
        }
        8 => {
            let field = gc_types::element(m, index)?;
            let count = args[1] as usize;
            Module::check_count(count)?;
            (vec![field.ty; count], vec![Ref(i64::from(index), false)])
        }
        9 | 10 => {
            let field = gc_types::element(m, index)?;
            let segment = args[1] as usize;
            if op == 9 {
                if field.ty.reference().is_some() || segment >= m.data.len() {
                    return Err(Error::invalid("invalid array.new_data segment or storage"));
                }
            } else if field.ty.reference().is_none() || segment >= m.elements.len() {
                return Err(Error::invalid("invalid array.new_elem segment or storage"));
            } else if !gc_types::subtype(m, m.elements[segment].ty, field.ty) {
                return Err(Error::invalid(
                    "segment type is not a subtype of array element type",
                ));
            }
            (vec![I32, I32], vec![Ref(i64::from(index), false)])
        }
        11..=14 => {
            let field = gc_types::element(m, index)?;
            if op == 14 {
                if !field.mutable {
                    return Err(Error::invalid(&format!("Array type #{index} is immutable")));
                }
                (vec![reference, I32, field.ty], vec![])
            } else {
                if (op == 11) != field.packed.is_none() {
                    return Err(Error::invalid("array.get packed storage mismatch"));
                }
                (vec![reference, I32], vec![field.ty])
            }
        }
        15 => (vec![Ref(-22, true)], vec![I32]),
        16 => {
            let field = gc_types::element(m, index)?;
            if !field.mutable {
                return Err(Error::invalid(&format!("Array type #{index} is immutable")));
            }
            (vec![reference, I32, field.ty, I32], vec![])
        }
        17 => {
            let target = gc_types::element(m, index)?;
            let source = gc_types::element(m, args[1] as u32)?;
            if !target.mutable {
                return Err(Error::invalid(&format!("Array type #{index} is immutable")));
            }
            if target.packed != source.packed || !gc_types::subtype(m, source.ty, target.ty) {
                return Err(Error::invalid(
                    "array.copy element types or mutability mismatch",
                ));
            }
            (vec![reference, I32, Ref(args[1], true), I32, I32], vec![])
        }
        18 | 19 => {
            let field = gc_types::element(m, index)?;
            if !field.mutable {
                let operation = if op == 18 {
                    "array.init_data"
                } else {
                    "array.init_elem"
                };
                return Err(Error::invalid(&format!(
                    "{operation} can only be used with mutable arrays"
                )));
            }
            if op == 18 {
                if field.ty.reference().is_some() || args[1] as usize >= m.data.len() {
                    return Err(Error::invalid(
                        "array.init_data storage or segment mismatch",
                    ));
                }
            } else if field.ty.reference().is_none() || args[1] as usize >= m.elements.len() {
                return Err(Error::invalid(
                    "array.init_elem storage or segment mismatch",
                ));
            } else if !gc_types::subtype(m, m.elements[args[1] as usize].ty, field.ty) {
                return Err(Error::invalid(
                    "segment type is not a subtype of array element type",
                ));
            }
            (vec![reference, I32, I32, I32], vec![])
        }
        20..=23 | 0x4c => {
            if matches!(args[0], -32 | -31 | -26) {
                return Err(Error::invalid(
                    "operand has to be in the same reference type hierarchy as a string view",
                ));
            }
            let target = if args.get(1) == Some(&1) {
                Type::Exact(args[0], op % 2 == 1)
            } else if args.get(2) == Some(&1) {
                Type::Shared(args[0], op % 2 == 1)
            } else {
                gc_types::reference(args[0], op % 2 == 1)
            };
            gc_types::check_type(m, target)?;
            let root = root(m, args[0])?;
            let root = if gc_types::is_shared(m, target) { let (heap, nullable) = root.reference().unwrap(); Type::Shared(heap, nullable) } else { root };
            (
                vec![root],
                vec![if op <= 21 { I32 } else { target }],
            )
        }
        26 => (vec![Extern], vec![Ref(-18, true)]),
        27 => (vec![Ref(-18, true)], vec![Extern]),
        28 => (vec![I32], vec![Ref(-20, false)]),
        31 => (vec![I32], vec![Type::Shared(-20, false)]),
        29 | 30 => (vec![Ref(-20, true)], vec![I32]),
        _ => return Err(Error::unsupported("WebAssembly GC instruction")),
    })
}
