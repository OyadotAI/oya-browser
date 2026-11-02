/** Parse and render actual HTML using Eye alone, without browser or service initialization. */
use mars_eye::{compute_styles, parse, protocol::EngineResult, render};

/** Display geometry is explicit caller-owned data suitable for a custom host surface. */
fn main() -> EngineResult<()> {
    let document = parse("<h1>Hello Eye</h1><p>Rust-owned document layout.</p>")?;
    let document = compute_styles(&document, &[])?;
    let display = render(&document, 640.0)?;
    println!("{} display items in {}", display.items.len(), display.unit);
    Ok(())
}
