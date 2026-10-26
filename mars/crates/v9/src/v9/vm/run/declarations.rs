/** Global declaration checks precede all binding creation; function initialization defines data without invoking setters. */
use super::*;

impl Vm<'_> {
    /** Global function declarations can replace configurable properties or writable enumerable data only. */
    pub(super) fn check_global_function(&mut self, name: &JsString, eval: bool) -> Res<()> {
        self.no_global_lexical(name)?;
        let allowed = if matches!(name.lossy().as_str(), "undefined" | "NaN" | "Infinity") {
            false
        } else {
            match self.global_declaration_property(&Key::Str(name.clone()))? {
                Some(prop) => {
                    prop.attrs & CONFIGURABLE != 0
                        || matches!(prop.slot, Slot::Data(_))
                            && prop.attrs & WRITABLE != 0
                            && prop.attrs & ENUMERABLE != 0
                }
                None => {
                    let global = self.global_object();
                    crate::v9::builtins::object_is_extensible(self, global)?
                }
            }
        };
        if !allowed {
            return Err(self.global_declaration_error(name, eval));
        }
        Ok(())
    }

    /** A var may reuse any existing own binding; an absent one requires an extensible global. */
    pub(super) fn check_global_var(&mut self, name: &JsString, eval: bool) -> Res<()> {
        self.no_global_lexical(name)?;
        let global = self.global_object();
        if self
            .global_declaration_property(&Key::Str(name.clone()))?
            .is_none()
            && !crate::v9::builtins::object_is_extensible(self, global)?
        {
            return Err(self.global_declaration_error(name, eval));
        }
        Ok(())
    }

    /** Declaration never initializes an existing property; the later initializer retains ordinary environment lookup. */
    pub(super) fn declare_global_var(&mut self, name: JsString, deletable: bool) -> Res<()> {
        self.check_global_var(&name, deletable)?;
        self.create_global_var(name, deletable, true)
    }

    /** Script Annex B declarations retain their prior optional creation behavior, separate from required eval declarations. */
    pub(super) fn declare_optional_global_var(&mut self, name: JsString) -> Res<()> {
        self.no_global_lexical(&name)?;
        self.create_global_var(name, false, false)
    }

    /** Failed required definitions throw; a compatibility-only script hoist may leave the global untouched. */
    fn create_global_var(&mut self, name: JsString, deletable: bool, required: bool) -> Res<()> {
        let key = Key::Str(name.clone());
        if self.global_declaration_property(&key)?.is_some() {
            return Ok(());
        }
        if !self.define(
            self.global_object(),
            key,
            Descriptor {
                value: Some(JsValue::Undefined),
                writable: Some(true),
                enumerable: Some(true),
                configurable: Some(deletable),
                ..Descriptor::default()
            },
        )? && required
        {
            return Err(self.global_declaration_error(&name, deletable));
        }
        Ok(())
    }

    /** Nonconfigurable writable globals retain their attributes; configurable accessors become ordinary data without calls. */
    pub(super) fn declare_global_function(
        &mut self,
        name: JsString,
        value: JsValue,
        deletable: bool,
    ) -> Res<()> {
        self.check_global_function(&name, deletable)?;
        let key = Key::Str(name.clone());
        let existing = self.own(self.global_object(), &key)?;
        let platform = if existing.is_none() {
            self.global_declaration_property(&key)?
        } else {
            None
        };
        let descriptor = if existing.is_some_and(|prop| prop.attrs & CONFIGURABLE == 0) {
            Descriptor {
                value: Some(value),
                ..Descriptor::default()
            }
        } else if let Some(prop) = platform.filter(|prop| prop.attrs & CONFIGURABLE == 0) {
            Descriptor {
                value: Some(value),
                writable: Some(prop.attrs & WRITABLE != 0),
                enumerable: Some(prop.attrs & ENUMERABLE != 0),
                configurable: Some(false),
                ..Descriptor::default()
            }
        } else {
            Descriptor {
                value: Some(value),
                writable: Some(true),
                enumerable: Some(true),
                configurable: Some(deletable),
                ..Descriptor::default()
            }
        };
        if !self.define(self.global_object(), key, descriptor)? {
            return Err(self.global_declaration_error(&name, deletable));
        }
        Ok(())
    }

    /** Browser Window intrinsics are represented on its immediate prototype; ordinary inherited properties are not bindings. */
    fn global_declaration_property(&mut self, key: &Key) -> Res<Option<Prop>> {
        let existing = self.own(self.global_object(), key)?;
        if existing.is_some() {
            return Ok(existing);
        }
        let window = realm().ids.window_prototype;
        if self.proto_of(self.global_object()) == Some(window)
            && realm()
                .object(window)
                .is_some_and(|definition| definition.get(key).is_some())
        {
            return self.own(window, key);
        }
        Ok(None)
    }

    /** Script instantiation and sloppy global eval distinguish SyntaxError from TypeError, without author constructors. */
    fn global_declaration_error(&mut self, name: &JsString, eval: bool) -> Thrown {
        let message = format!("Identifier '{}' has already been declared", name.lossy());
        if eval {
            self.type_error(&message)
        } else {
            self.syntax_error(&message)
        }
    }
}
