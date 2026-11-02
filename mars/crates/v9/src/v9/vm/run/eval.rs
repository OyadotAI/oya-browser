/**
 * Direct eval at run time (spec PerformEval, V8's Runtime_ResolvePossiblyDirectEval): a call
 * spelled `eval(...)` whose callee is the intrinsic eval compiles its string in the caller's
 * scopes and runs it on the caller's environment, `this`, callee and `new.target`. Any other
 * callee is an ordinary call. Also the eval-var object ops a sloppy eval's `var`s live in.
 */
use super::{Flow, Vm};
use crate::protocol::EngineError;
use crate::v9::builtins::dynamic_base;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::DynamicSource;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Exit, Frame, Res, Resume};
use std::sync::OnceLock;

/** The intrinsic %eval% function's identity. */
fn intrinsic_eval() -> Option<ObjectId> {
    static INDEX: OnceLock<Option<u32>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            realm()
                .natives
                .iter()
                .position(|native| native.name == "eval")
                .map(|index| index as u32)
        })
        .map(|index| realm().native_identity(index))
}

impl Vm<'_> {
    /** `eval(args)`: direct when the callee is %eval%, else an ordinary call. */
    pub(super) fn direct_eval(&mut self, count: u16, context: u32) -> Res<Flow> {
        let count = count as usize;
        let function = self.peek(count + 1);
        if intrinsic_eval().is_none_or(|id| function != JsValue::Object(id)) {
            let stack = &self.frame().stack;
            let args = stack[stack.len().saturating_sub(count)..].to_vec();
            let this = self.peek(count);
            return self.call_op(function, this, args, count + 2, None);
        }
        let argument = if count == 0 {
            JsValue::Undefined
        } else {
            self.peek(count - 1)
        };
        // A TrustedScript evaluates as its text, as the Trusted Types sink for eval says.
        let argument = match crate::v9::platform::trusted_script_text(self, &argument)? {
            Some(text) => text,
            None => argument,
        };
        let value = match argument {
            JsValue::String(source) => {
                let context = self.constant(context).lossy();
                self.run_direct_eval(source.lossy(), context)?
            }
            other => other,
        };
        self.drop_n(count + 2);
        self.push(value);
        Ok(Flow::Next)
    }
    /** Compile direct eval source in the caller context and run it on the caller's frame state. */
    fn run_direct_eval(&mut self, source: String, context: String) -> Res<JsValue> {
        self.check_string_codegen()?;
        self.check_eval_envs(&context)?;
        let origin = self.capture_eval_origin()?;
        self.prepare_dynamic(source.len().saturating_add(origin.bytes()))?;
        let native = self.native_dynamic();
        let base = dynamic_base(&self.dynamic);
        let code = match crate::v9::compile_dynamic(
            &mut self.registry,
            &source,
            base,
            Some(&context),
            native,
        ) {
            Ok(code) => code,
            Err(error) if error.code == "script_syntax_error" => {
                return Err(self.syntax_error(&crate::v9::errors::syntax_reason(&error)));
            }
            Err(error) if error.code == "unsupported_feature" => {
                return Err(self.unsupported_error("dynamic code with unsupported syntax"));
            }
            Err(error) => return Err(error.into()),
        };
        let origin = self.finish_eval_origin(&source, origin)?;
        self.dynamic.push(DynamicSource {
            source,
            context: Some(context),
            base: Some(base),
            origin: Some(origin),
        });
        let caller = self.frame();
        let frame = Frame {
            realm: caller.realm,
            regs: vec![JsValue::Undefined; code.registers as usize],
            code,
            pc: 0,
            stack: Vec::new(),
            env: caller.env,
            this: caller.this.clone(),
            callee: caller.callee.clone(),
            args: caller.args.clone(),
            new_target: caller.new_target.clone(),
            handlers: Vec::new(),
            envs: 0,
            boundary: true,
            caller_hidden: false,
            construct: false,
            resume: Resume::Next,
        };
        self.frames.push(frame);
        let previous_realm = self.active_realm;
        self.native_depth += 1;
        let result = self.run();
        self.native_depth -= 1;
        self.select_realm(previous_realm)?;
        match result? {
            Exit::Return(value) => Ok(value),
            _ => Err(EngineError::invalid("V9 eval code suspended.").into()),
        }
    }
    /** Refuse a context whose environment scopes the caller's real environment chain cannot hold:
    eval code indexes them by (hops, slot), so each must exist and be at least that large. */
    fn check_eval_envs(&mut self, context: &str) -> Res<()> {
        let sizes = crate::v9::compile::env_sizes(context)
            .ok_or_else(|| EngineError::invalid("Malformed V9 eval context."))?;
        let mut env = self.frame().env;
        for needed in sizes {
            let record = env.and_then(|id| self.heap.env(id));
            match record {
                Some(record) if record.slots.len() >= needed as usize => env = record.parent,
                _ => return Err(EngineError::invalid("V9 eval context exceeds its caller.").into()),
            }
        }
        let sites = crate::v9::compile::eval_private_sites(context)
            .ok_or_else(|| EngineError::invalid("Malformed V9 private eval context."))?;
        let mut env = self.frame().env;
        for site in sites.iter().rev() {
            let mut found = false;
            while let Some(id) = env {
                let record = self
                    .heap
                    .env(id)
                    .ok_or_else(|| EngineError::invalid("Missing V9 private eval environment."))?;
                env = record.parent;
                if record.class_site == Some(*site) {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(
                    EngineError::invalid("V9 private eval context exceeds its caller.").into(),
                );
            }
        }
        Ok(())
    }
    /** The eval-var object in env slot (hops, slot), if one was made. */
    fn ext_object(&mut self, hops: u16, slot: u16) -> Res<Option<ObjectId>> {
        let env = self.env_at(hops)?;
        Ok(match self.env_slot(env, slot)? {
            JsValue::Object(id) => Some(id),
            _ => None,
        })
    }
    /** Push the eval var's value and true, or just false. */
    pub(super) fn get_ext(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let key = Key::Str(self.constant(name));
        if let Some(id) = self.ext_object(hops, slot)?
            && self.own(id, &key)?.is_some()
        {
            let value = self.get(&JsValue::Object(id), &key)?;
            self.push(value);
            self.push(JsValue::Boolean(true));
            return Ok(());
        }
        self.push(JsValue::Boolean(false));
        Ok(())
    }
    /** Store the top of stack into the eval var when it exists; push whether it did. */
    pub(super) fn set_ext(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let key = Key::Str(self.constant(name));
        if let Some(id) = self.ext_object(hops, slot)?
            && self.own(id, &key)?.is_some()
        {
            let value = self.peek(0);
            self.write_own(id, key, value)?;
            self.push(JsValue::Boolean(true));
            return Ok(());
        }
        self.push(JsValue::Boolean(false));
        Ok(())
    }
    /** Dynamic eval bindings are deletable; absence continues to the outer binding. */
    pub(super) fn delete_ext(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let key = Key::Str(self.constant(name));
        if let Some(id) = self.ext_object(hops, slot)?
            && self.own(id, &key)?.is_some()
        {
            let deleted = self.delete(id, &key)?;
            self.push(JsValue::Boolean(deleted));
            self.push(JsValue::Boolean(true));
        } else {
            self.push(JsValue::Boolean(false));
        }
        Ok(())
    }
    /** Create an eval var (undefined), making the function's eval-var object on first use. */
    pub(super) fn declare_ext(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let key = Key::Str(self.constant(name));
        let id = match self.ext_object(hops, slot)? {
            Some(id) => id,
            None => {
                let id = self.alloc(Object::new(Class::EvalVars, None))?;
                let env = self.env_at(hops)?;
                self.heap.set_slot(env, slot as usize, JsValue::Object(id));
                id
            }
        };
        if self.own(id, &key)?.is_none() {
            self.write_own(id, key, JsValue::Undefined)?;
        }
        Ok(())
    }
}
