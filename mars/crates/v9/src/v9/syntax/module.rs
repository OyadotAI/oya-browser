/** Module grammar: import and export declarations at a module's top level. */
use super::stmt::infer_name;
use super::*;
use std::sync::Arc;

impl Parser {
    /** Whether the cursor starts an import or export declaration: a module's top level only,
    and never `import(` or `import.meta`, which are expressions. */
    pub(super) fn module_item(&self) -> bool {
        if self.module.is_none() || self.levels.statements != 1 || self.nesting != 0 {
            return false;
        }
        match &self.peek().kind {
            Kind::Name(name) if name == "export" => true,
            Kind::Name(name) if name == "import" => {
                !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '.'))
            }
            _ => false,
        }
    }
    /** One import or export declaration; what it declares stands in the body as a plain one. */
    pub(super) fn module_declaration(&mut self) -> EngineResult<StmtKind> {
        if self.eat_name("import") {
            self.import_declaration()?;
            return Ok(StmtKind::Empty);
        }
        self.expect_name("export")?;
        self.export_declaration()
    }
    /** `import 'm'`, `import d, { a as b } from 'm'`, `import * as ns from 'm'`. */
    fn import_declaration(&mut self) -> EngineResult<()> {
        if matches!(self.peek().kind, Kind::String(_)) {
            self.module_request()?;
            return self.module_end();
        }
        let mut entries = Vec::new();
        if self.at_identifier() {
            let pos = self.pos();
            let local = self.identifier()?;
            entries.push((local, Some(Name::from("default")), pos));
            if !self.eat(",") {
                return self.import_from(entries);
            }
        }
        if self.eat("*") {
            self.expect_name("as")?;
            let pos = self.pos();
            let local = self.identifier()?;
            entries.push((local, None, pos));
        } else {
            self.expect("{")?;
            while !self.eat("}") {
                let pos = self.pos();
                let imported = self.module_export_name()?;
                let local = if self.eat_name("as") {
                    self.identifier()?
                } else {
                    self.binding_name(&imported, pos)?
                };
                entries.push((local, Some(imported), pos));
                if !self.at("}") {
                    self.expect(",")?;
                }
            }
        }
        self.import_from(entries)
    }
    /** `from 'm'` after an import clause, recording each binding against the request. */
    fn import_from(&mut self, entries: Vec<(Name, Option<Name>, Pos)>) -> EngineResult<()> {
        self.expect_name("from")?;
        let request = self.module_request()?;
        for (local, imported, pos) in entries {
            if &*local == "eval" || &*local == "arguments" {
                return Err(self.error_at(pos, "Unexpected eval or arguments in strict mode"));
            }
            let declared = self
                .module
                .as_ref()
                .is_some_and(|module| module.imports.iter().any(|entry| entry.local == local));
            if declared {
                return Err(self.error_at(pos, "Identifier has already been declared"));
            }
            let module = self.module.as_mut().expect("module parser");
            module.imports.push(ImportEntry {
                local,
                request,
                imported,
            });
        }
        self.module_end()
    }
    /** An imported name used as its own local binding must be a plain identifier. */
    fn binding_name(&self, name: &Name, pos: Pos) -> EngineResult<Name> {
        let usable = name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '$' || c == '_')
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '$' || c == '_')
            && !reserved(name, true)
            && &**name != "await";
        if usable {
            Ok(name.clone())
        } else {
            Err(self.error_at(pos, "Unexpected reserved word"))
        }
    }
    /** Everything after `export`. */
    fn export_declaration(&mut self) -> EngineResult<StmtKind> {
        let pos = self.pos();
        if self.eat_name("default") {
            return self.export_default(pos);
        }
        if self.eat("*") {
            let export = if self.eat_name("as") {
                Some(self.module_export_name()?)
            } else {
                None
            };
            self.expect_name("from")?;
            let request = self.module_request()?;
            let entry = match export {
                Some(export) => ExportEntry::Indirect {
                    export,
                    request,
                    imported: None,
                },
                None => ExportEntry::Star(request),
            };
            self.export(entry);
            self.module_end()?;
            return Ok(StmtKind::Empty);
        }
        if self.at("{") {
            self.export_list()?;
            return Ok(StmtKind::Empty);
        }
        let statement = self.statement(true)?;
        let mut names = Vec::new();
        match &statement.kind {
            StmtKind::Var(kind, declarators) if !kind.resource() => {
                for declarator in declarators {
                    pattern_names(&declarator.target, &mut names);
                }
            }
            StmtKind::Function(func) => names.extend(func.name.clone()),
            StmtKind::Class(class) => names.extend(class.name.clone()),
            _ => return Err(self.error_at(pos, "Unexpected token 'export'")),
        }
        for name in names {
            self.export(ExportEntry::Local {
                export: name.clone(),
                local: name,
                pos,
            });
        }
        Ok(statement.kind)
    }
    /** `export default` of a function, a class or an expression. */
    fn export_default(&mut self, pos: Pos) -> EngineResult<StmtKind> {
        let at = self.pos();
        let is_async = self.at_name("async")
            && matches!(&self.peek_at(1).kind, Kind::Name(name) if name == "function")
            && !self.peek_at(1).line_terminator_before;
        if is_async || self.at_name("function") {
            if is_async {
                self.next();
            }
            self.next();
            let mut func = self.function_rest(at, is_async, false)?;
            if func.name.is_none() {
                *func.display.borrow_mut() = Name::from("default");
                Arc::get_mut(&mut func)
                    .ok_or_else(|| self.error_at(at, "Unexpected token 'export'"))?
                    .name = Some(Name::from(DEFAULT_BINDING));
            }
            let local = func.name.clone().expect("named above");
            self.export_default_local(local, pos);
            return Ok(StmtKind::Function(func));
        }
        if self.at_name("class") {
            let class = self.class(false)?;
            if let Some(local) = class.name.clone() {
                self.export_default_local(local, pos);
                return Ok(StmtKind::Class(class));
            }
            let mut value = self.node(ExprKind::Class(class), at)?;
            infer_name(&mut value, "default");
            return Ok(self.default_binding(value, pos));
        }
        let mut value = self.assignment(false)?;
        infer_name(&mut value, "default");
        self.semicolon()?;
        Ok(self.default_binding(value, pos))
    }
    /** `export default expression` initializes the hidden default binding. */
    fn default_binding(&mut self, value: Expr, pos: Pos) -> StmtKind {
        let local = Name::from(DEFAULT_BINDING);
        self.export_default_local(local.clone(), pos);
        StmtKind::Var(
            VarKind::Const,
            vec![Declarator {
                target: Pattern::Ident(local, pos),
                init: Some(value),
            }],
        )
    }
    /** Record `default` as an export of a local binding. */
    fn export_default_local(&mut self, local: Name, pos: Pos) {
        self.export(ExportEntry::Local {
            export: Name::from("default"),
            local,
            pos,
        });
    }
    /** `export { a, b as c }`, or with `from 'm'` a re-export of another module's names. */
    fn export_list(&mut self) -> EngineResult<()> {
        self.expect("{")?;
        let mut list = Vec::new();
        while !self.eat("}") {
            let pos = self.pos();
            let string = matches!(self.peek().kind, Kind::String(_));
            let local = self.module_export_name()?;
            let export = if self.eat_name("as") {
                self.module_export_name()?
            } else {
                local.clone()
            };
            list.push((local, export, pos, string));
            if !self.at("}") {
                self.expect(",")?;
            }
        }
        if self.eat_name("from") {
            let request = self.module_request()?;
            for (imported, export, _, _) in list {
                self.export(ExportEntry::Indirect {
                    export,
                    request,
                    imported: Some(imported),
                });
            }
        } else {
            for (local, export, pos, string) in list {
                if string {
                    return Err(self.error_at(pos, "A string export name needs 'from'"));
                }
                self.binding_name(&local, pos)?;
                self.export(ExportEntry::Local { export, local, pos });
            }
        }
        self.module_end()
    }
    /** An identifier name or, since ES2022, a string literal naming an export. */
    fn module_export_name(&mut self) -> EngineResult<Name> {
        if let Kind::String(value) = &self.peek().kind {
            let location = self.peek().location;
            // IsStringWellFormedUnicode: an export name with a lone surrogate is an early error.
            let text = value.to_dom().map_err(|_| {
                errors::syntax(location, "An export name must be well-formed Unicode")
            })?;
            self.next();
            return Ok(Name::from(text.as_str()));
        }
        self.property_name()
    }
    /** A specifier string, interned as a request index. */
    fn module_request(&mut self) -> EngineResult<usize> {
        let token = self.next();
        let Kind::String(value) = token.kind else {
            self.position -= 1;
            return Err(self.unexpected());
        };
        let text = value
            .to_dom()
            .map_err(|error| errors::located(error, token.location, "parse"))?;
        let module = self.module.as_mut().expect("module parser");
        Ok(
            match module.requests.iter().position(|request| *request == text) {
                Some(index) => index,
                None => {
                    module.requests.push(text);
                    module.requests.len() - 1
                }
            },
        )
    }
    /** Import attributes (`with { type: 'json' }`), checked and then ignored, then the
    semicolon. Each entry is `key: 'value'` with an identifier-name or string key, and a key
    given twice is an early error (spec 16.2.2.1). */
    fn module_end(&mut self) -> EngineResult<()> {
        if (self.at_name("with") || self.at_name("assert")) && !self.peek().line_terminator_before {
            self.next();
            self.expect("{")?;
            let mut keys = Vec::new();
            while !self.eat("}") {
                let token = self.next();
                let key = match token.kind {
                    Kind::Name(name) => name.as_str().to_string(),
                    Kind::String(value) => value
                        .to_dom()
                        .map_err(|error| errors::located(error, token.location, "parse"))?,
                    _ => {
                        self.position -= 1;
                        return Err(self.unexpected());
                    }
                };
                if keys.contains(&key) {
                    return Err(errors::syntax(
                        token.location,
                        "Import attribute has duplicate key",
                    ));
                }
                keys.push(key);
                self.expect(":")?;
                if !matches!(self.next().kind, Kind::String(_)) {
                    self.position -= 1;
                    return Err(self.unexpected());
                }
                if !self.eat(",") {
                    self.expect("}")?;
                    break;
                }
            }
        }
        self.semicolon()
    }
    /** Append an export entry. */
    fn export(&mut self, entry: ExportEntry) {
        self.module
            .as_mut()
            .expect("module parser")
            .exports
            .push(entry);
    }
}

/** Every name a declaration pattern binds. */
fn pattern_names(pattern: &Pattern, names: &mut Vec<Name>) {
    match pattern {
        Pattern::Ident(name, _) => names.push(name.clone()),
        Pattern::Expr(_) => {}
        Pattern::Object(props, rest, _) => {
            for prop in props {
                pattern_names(&prop.value, names);
            }
            if let Some(rest) = rest {
                pattern_names(rest, names);
            }
        }
        Pattern::Array(items, rest, _) => {
            for item in items.iter().flatten() {
                pattern_names(item, names);
            }
            if let Some(rest) = rest {
                pattern_names(rest, names);
            }
        }
        Pattern::Default(target, _) => pattern_names(target, names),
    }
}

/** No top-level declaration may reuse an import's local name. */
pub(super) fn check_imports(module: &ModuleSyntax, body: &[Stmt]) -> EngineResult<()> {
    let mut names = Vec::new();
    for stmt in body {
        match &stmt.kind {
            StmtKind::Var(_, list) => list
                .iter()
                .for_each(|d| super::func::bound_names(&d.target, &mut names)),
            StmtKind::Class(class) => names.extend(class.name.clone()),
            StmtKind::Function(func) => names.extend(func.name.clone()),
            _ => {}
        }
        if let Some(entry) = module.imports.iter().find(|e| names.contains(&e.local)) {
            return Err(errors::syntax(
                stmt.pos.location(),
                &format!("Identifier '{}' has already been declared", entry.local),
            ));
        }
    }
    Ok(())
}

/** A module may export each name once. */
pub(super) fn check_exports(module: &ModuleSyntax, pos: Pos) -> EngineResult<()> {
    let mut seen = std::collections::HashSet::new();
    for entry in &module.exports {
        let export = match entry {
            ExportEntry::Local { export, .. } | ExportEntry::Indirect { export, .. } => export,
            ExportEntry::Star(_) => continue,
        };
        if !seen.insert(export.clone()) {
            return Err(errors::syntax(
                pos.location(),
                &format!("Duplicate export of '{export}'"),
            ));
        }
    }
    Ok(())
}
