/**
 * Module lowering. A module body compiles as an async function whose parameters are the
 * namespaces of the modules it requests and its `import.meta` object. Its bindings all live in
 * its environment, so other modules read exports there through namespaces (live bindings), and
 * it suspends right after hoisting, so a whole graph is instantiated before any module runs.
 */
use super::*;
use crate::v9::syntax::{DEFAULT_BINDING, ExportEntry, FuncFlags, ModuleSyntax, Pattern, site_of};
use std::cell::RefCell;
use std::collections::BTreeMap;

/** The hidden parameter holding a module's `import.meta` object. */
pub(super) const META: &str = "%meta";

/** Where one exported name's value comes from. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::v9) enum Export {
    /** A binding of this module, at this slot of its environment. */
    Slot(Slot),
    /** `imported` of request `request`, or its namespace when None. */
    Indirect(usize, Option<Name>),
}

/** What linking needs of one compiled module. */
#[derive(Debug, Default)]
pub(in crate::v9) struct ModuleInfo {
    /** Distinct specifiers, in order; the body's parameters are their namespaces. */
    pub requests: Vec<String>,
    /** Named imports that must resolve when linking: request and imported name. */
    pub imports: Vec<(usize, Name)>,
    /** Exported names and their sources. */
    pub exports: BTreeMap<Name, Export>,
    /** `export * from` requests, in order. */
    pub stars: Vec<usize>,
}

/** The module being compiled. */
pub(super) struct ModuleScope {
    /** The body's code site. */
    site: u32,
    /** Its imports and exports. */
    syntax: Box<ModuleSyntax>,
    /** The body scope's bindings, kept once the body is compiled. */
    bindings: Vec<Binding>,
}

/** The hidden parameter naming request `index`'s namespace. */
fn dependency(index: usize) -> Name {
    Name::from(format!("%dep{index}").as_str())
}

impl Compiler {
    /** Compile a module body; returns its code site and link facts. */
    pub(super) fn module(&mut self, mut program: Program) -> EngineResult<(u32, ModuleInfo)> {
        let syntax = program.module.take().unwrap_or_default();
        let pos = program.pos;
        let site = site_of(pos);
        let mut params: Vec<Pattern> = (0..syntax.requests.len())
            .map(|index| Pattern::Ident(dependency(index), pos))
            .collect();
        params.push(Pattern::Ident(Name::from(META), pos));
        let body = Func {
            source_span: std::cell::RefCell::new(None),
            site,
            name: None,
            display: RefCell::new(Name::from("")),
            params,
            rest: None,
            body: Body::Block(std::mem::take(&mut program.body)),
            flags: FuncFlags {
                asynchronous: true,
                ..FuncFlags::default()
            },
            strict: true,
            pos,
            length: 0,
        };
        self.module = Some(ModuleScope {
            site,
            syntax,
            bindings: Vec::new(),
        });
        self.compile_function(&body, false)?;
        let scope = self.module.take().expect("module scope");
        Ok((site, link_facts(scope)?))
    }
    /** Whether `site` is the module body being compiled. */
    pub(super) fn module_body(&self, site: u32) -> bool {
        self.module
            .as_ref()
            .is_some_and(|module| module.site == site)
    }
    /** Keep the body scope's bindings for the export table. */
    pub(super) fn keep_module_bindings(&mut self, bindings: Vec<Binding>) {
        if let Some(module) = &mut self.module {
            module.bindings = bindings;
        }
    }
    /** Declare the imported names: a namespace import is a constant holding the namespace,
    a named import reads through it on every use. */
    pub(super) fn module_imports(&mut self, pos: Pos) -> EngineResult<()> {
        let imports: Vec<(Name, usize, bool)> = self
            .module
            .as_ref()
            .map(|module| {
                module
                    .syntax
                    .imports
                    .iter()
                    .map(|entry| (entry.local.clone(), entry.request, entry.imported.is_none()))
                    .collect()
            })
            .unwrap_or_default();
        for (local, request, namespace) in imports {
            if namespace {
                self.declare(&local, Declared::Const)?;
                self.load_name(&dependency(request), pos)?;
                self.initialize(&local, pos)?;
                self.emit(Op::Pop);
            } else {
                self.declare(&local, Declared::Import)?;
            }
        }
        Ok(())
    }
    /** Push an imported name's current value from its module's namespace. */
    pub(super) fn load_import(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        let entry = self.module.as_ref().and_then(|module| {
            module
                .syntax
                .imports
                .iter()
                .find(|entry| entry.local == *name)
                .map(|entry| (entry.request, entry.imported.clone()))
        });
        let Some((request, Some(imported))) = entry else {
            return Err(self.error(pos, "Invalid module import"));
        };
        self.load_name(&dependency(request), pos)?;
        let index = self.name(&imported)?;
        self.emit(Op::GetProp(index));
        Ok(())
    }
}

/** The export table and imports that linking checks, from the body's bindings. */
fn link_facts(scope: ModuleScope) -> EngineResult<ModuleInfo> {
    let ModuleScope {
        syntax, bindings, ..
    } = scope;
    let mut info = ModuleInfo {
        requests: syntax.requests.clone(),
        imports: syntax
            .imports
            .iter()
            .filter_map(|entry| Some((entry.request, entry.imported.clone()?)))
            .collect(),
        ..ModuleInfo::default()
    };
    for entry in &syntax.exports {
        match entry {
            ExportEntry::Local { export, local, pos } => {
                let imported = syntax.imports.iter().find(|import| import.local == *local);
                let source = match imported {
                    // A re-exported namespace import is the namespace itself (ES2024), so two
                    // star exports of it are not ambiguous.
                    Some(import) => Export::Indirect(import.request, import.imported.clone()),
                    _ => match bindings.iter().find(|binding| binding.name == *local) {
                        Some(Binding {
                            place: Place::Env(slot),
                            ..
                        }) => Export::Slot(*slot),
                        _ => {
                            let reason = if &**local == DEFAULT_BINDING {
                                "Invalid default export".to_string()
                            } else {
                                format!("Export '{local}' is not defined in module")
                            };
                            return Err(errors::syntax(pos.location(), &reason));
                        }
                    },
                };
                info.exports.insert(export.clone(), source);
            }
            ExportEntry::Indirect {
                export,
                request,
                imported,
            } => {
                info.exports
                    .insert(export.clone(), Export::Indirect(*request, imported.clone()));
            }
            ExportEntry::Star(request) => info.stars.push(*request),
        }
    }
    Ok(info)
}
