/** Native unit tests exercise public engine surfaces without opening sockets. */
#[path = "unit/document/mod.rs"]
mod document;
#[path = "unit/mcp/mod.rs"]
mod mcp;
#[path = "unit/network/mod.rs"]
mod network;
#[path = "unit/profile.rs"]
mod profile;
#[path = "unit/protocol/codec.rs"]
mod protocol;
#[path = "unit/v9/mod.rs"]
mod v9;
