/**
 * Applies an Oya persona to a Mars runtime as one unit. What the runtime can already take
 * (navigator.languages) is applied here; the rest of the identity (User-Agent, client hints,
 * Accept-Language, proxy for Relay; platform, hardware, screen, timezone, WebGL and noise for V9)
 * travels as `mars_link::Identity` until those engines accept it. A runtime gets its persona once,
 * before its first navigation: switching persona means a new runtime, never an edited one.
 */
use crate::runtime::Runtime;
use mars_link::{Identity, Persona, identity::IdentityError};

/// The Chrome version Mars presents personas as. Mars has no Chrome engine, so this is a claim
/// the identity must make consistently: it is Relay's own version, so a persona's User-Agent and
/// client hints always match the headers Relay sends.
pub const PRESENTED_ENGINE: &str = crate::network::CHROME_ENGINE;

/// A runtime presenting `persona`, and the identity the engines must apply for it.
pub fn with_persona(
    runtime: Runtime,
    persona: &Persona,
) -> Result<(Runtime, Identity), IdentityError> {
    let identity = persona.identity(PRESENTED_ENGINE)?;
    let runtime = runtime.with_languages(identity.policy.languages.clone());
    Ok((runtime, identity))
}
