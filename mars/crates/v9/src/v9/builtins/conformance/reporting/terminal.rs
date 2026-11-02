/** Real terminal shell reporting retains failure provenance across actual native stop control. */
use super::*;

/** A requested stop still aborts the source list when an earlier uncaught failure forbids successful scoring. */
const FAILED_EXIT: &str = "conformance_exit_after_failure";

/** This private host outcome conveys failure provenance without inventing a different requested exit status. */
pub(in crate::v9) fn is_failed_exit(error: &EngineError) -> bool {
    error.code == FAILED_EXIT
}

/** Root throws remain failed scripts after reporting; native termination interrupts the whole host run. */
pub(in crate::v9) fn terminal(
    vm: &mut Vm,
    thrown: Thrown,
    location: SourceLocation,
) -> EngineError {
    match thrown {
        Thrown::Fatal(error) => stop_or_locate(vm, error, location),
        Thrown::Value(value) => {
            let mark = vm.pins.len();
            vm.pins.push(value.clone());
            let error = terminal_value(vm, value, location);
            vm.pins.truncate(mark);
            error
        }
    }
}

/** The original throw remains rooted until classification, callbacks and the final diagnostic all complete. */
fn terminal_value(vm: &mut Vm, value: JsValue, location: SourceLocation) -> EngineError {
    // A reporting callback may reach another missing capability, replacing the VM's last thrown marker.
    let native = vm
        .unsupported
        .as_ref()
        .filter(|(id, _)| value.object() == Some(*id))
        .map(|(_, error)| error.clone());
    let assertion = is_assertion(vm, &value);
    let formatted = match detail(vm, value.clone()) {
        Ok(formatted) => formatted,
        Err(Thrown::Fatal(error)) => return stop_or_locate(vm, error, location),
        Err(Thrown::Value(error)) => {
            note(vm, &error);
            None
        }
    };
    if let Some(error) = native.or_else(|| recorded_native(vm)) {
        return crate::v9::vm::error::locate(error, location);
    }
    let recorded_assertion = vm
        .page
        .effects
        .conformance_error
        .clone()
        .filter(|error| error.contains("MjsUnitAssertionError"));
    let description = match formatted {
        Some(value) => describe(vm, &JsValue::String(value)),
        None => recorded_assertion
            .clone()
            .unwrap_or_else(|| describe(vm, &value)),
    };
    let assertion = assertion || recorded_assertion.is_some();
    root_failure(description, assertion, location)
}

/** A semantic assertion or unsupported native throw cannot disappear inside an otherwise successful requested exit. */
fn stop_or_locate(vm: &Vm, error: EngineError, location: SourceLocation) -> EngineError {
    let known = vm
        .page
        .effects
        .conformance_native_error
        .as_ref()
        .map(|(code, message)| format!("{code}: {message}"))
        .or_else(|| vm.page.effects.conformance_error.clone());
    if error.code == super::super::EXIT_CODE
        && let Some(cause) = known
    {
        return crate::v9::vm::error::locate(
            EngineError::new(
                FAILED_EXIT,
                cause,
                &format!(
                    "The host stopped after a recorded uncaught failure. Requested exit: {}. {}",
                    error.message, error.hint
                ),
            ),
            location,
        );
    }
    crate::v9::vm::error::locate(error, location)
}

/** Native failure codes remain explicit even when the root throw originally carried another ordinary value. */
fn recorded_native(vm: &Vm) -> Option<EngineError> {
    vm.page
        .effects
        .conformance_native_error
        .as_ref()
        .map(|(code, message)| {
            EngineError::new(
                code,
                message.clone(),
                "An uncaught native capability failure occurred while reporting this exception.",
            )
        })
}

/** Diagnostics use the already computed string and never run an author's message/toString hooks a second time. */
fn root_failure(mut description: String, assertion: bool, location: SourceLocation) -> EngineError {
    if assertion && !description.starts_with("MjsUnitAssertionError") {
        description = format!("MjsUnitAssertionError: {description}");
    }
    let description = description.strip_suffix('.').unwrap_or(&description);
    crate::v9::vm::error::locate(
        EngineError::new(
            "script_execution_error",
            format!(
                "Script execution failed at line {}, column {}: Uncaught {description}.",
                location.line, location.column
            ),
            "Correct the script; what it did before the error stays, as in browsers.",
        ),
        location,
    )
}
