/** Run the standalone V9 library with automatic separators and Unicode string spread. */
use mars_v9::{load, protocol::EngineResult};

/** Explicit input data and returned values require no browser or sibling engine process. */
fn main() -> EngineResult<()> {
    let execution = load(
        "const names = ['Mars', 'V9']\nconst message = [...names, ...'A😀B'].join(' | ')",
        &[],
    )?;
    if let Some(value) = &execution.state.globals()["message"].value {
        println!("{}", value.to_dom()?);
    }
    Ok(())
}
