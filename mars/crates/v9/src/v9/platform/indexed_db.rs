/**
 * IndexedDB: a JavaScript bootstrap (the `indexed_db/js` sections) plus the native seam it is
 * handed. The database engine lives in the page's own heap, because IndexedDB is specified as
 * tasks, events and promises over ordered key/value records, all of which V9 already runs; the
 * engine's state then survives between lifecycle steps like any other page state, and nothing
 * crosses the process boundary except database images when a database is opened and the records
 * a transaction changed when it commits. The coordinator owns persistence per storage partition
 * (`wire.rs` defines what crosses); the seam here dispatches trusted events along the
 * request -> transaction -> database path, reads Blob bytes, runs microtask checkpoints and
 * records host requests and commits.
 */
mod natives;
mod wire;

pub(in crate::v9) use natives::{end_of_task, install};
pub use wire::{
    INDEXED_DB_DATABASE_BYTES, INDEXED_DB_EFFECT_BYTES, INDEXED_DB_KIND, INDEXED_DB_LIST,
    INDEXED_DB_OPEN, INDEXED_DB_ORIGIN_BYTES, INDEXED_DB_ORIGIN_RECORD_BYTES,
    INDEXED_DB_RESPONSE_BYTES, IndexedDbCommit, IndexedDbDatabase, IndexedDbOperation,
    IndexedDbOrigin, IndexedDbStore,
};

use crate::protocol::EngineResult;
use crate::v9::compile::Code;
use crate::v9::value::JsValue;
use crate::v9::vm::{GLOBAL, Vm};
use std::sync::Arc;

/** The bootstrap: one function expression, `(function installIndexedDB(global, host) { ... })`, cut into sections by concern. */
const SOURCE: &str = concat!(
    include_str!("indexed_db/js/prelude.js"),
    include_str!("indexed_db/js/interfaces.js"),
    include_str!("indexed_db/js/keys.js"),
    include_str!("indexed_db/js/keypaths.js"),
    include_str!("indexed_db/js/values.js"),
    include_str!("indexed_db/js/revive.js"),
    include_str!("indexed_db/js/ranges.js"),
    include_str!("indexed_db/js/storage.js"),
    include_str!("indexed_db/js/requests.js"),
    include_str!("indexed_db/js/transactions.js"),
    include_str!("indexed_db/js/scheduler.js"),
    include_str!("indexed_db/js/mutations.js"),
    include_str!("indexed_db/js/commit.js"),
    include_str!("indexed_db/js/queries.js"),
    include_str!("indexed_db/js/objectstore.js"),
    include_str!("indexed_db/js/indexes.js"),
    include_str!("indexed_db/js/schema.js"),
    include_str!("indexed_db/js/cursors.js"),
    include_str!("indexed_db/js/cursor_methods.js"),
    include_str!("indexed_db/js/database.js"),
    include_str!("indexed_db/js/factory.js"),
    include_str!("indexed_db/js/install.js"),
);
/** Where the bootstrap compiles: one mebibyte past the Observable bootstrap's offset, which leaves that source room to grow, and far below dynamic code (DYNAMIC_OFFSET), so their sites never collide. */
const BOOTSTRAP_OFFSET: usize = super::observable::BOOTSTRAP_OFFSET + BOOTSTRAP_GAP;
/** The room left for the Observable bootstrap before this one starts. */
const BOOTSTRAP_GAP: usize = 1024 * 1024;
const _: () = assert!(BOOTSTRAP_OFFSET + SOURCE.len() < crate::v9::DYNAMIC_OFFSET);
/* The platform bootstrap before this one stays inside its gap, so their code sites never collide. */
const _: () = assert!(super::observable::SOURCE_LEN < BOOTSTRAP_GAP);

/** Compile the bootstrap into this session's registry; every step does, because the heap's closures refer to its code by site. */
pub(in crate::v9) fn compile(vm: &mut Vm) -> EngineResult<Arc<Code>> {
    crate::v9::compile_dynamic(&mut vm.registry, SOURCE, BOOTSTRAP_OFFSET, None, false)
}

/** Whether a code site belongs to the bootstrap, whose functions show as native code like the interfaces they implement. */
pub(in crate::v9) fn is_bootstrap_site(site: u32) -> bool {
    let offset = (site / crate::v9::syntax::SITE_TAGS) as usize;
    (BOOTSTRAP_OFFSET..BOOTSTRAP_OFFSET + SOURCE.len()).contains(&offset)
}

/** Run the bootstrap once for a fresh page or worker: evaluate the installer, then call it with the global and the native seam. A failure breaks every page, so it is an engine error. */
pub(in crate::v9) fn run(vm: &mut Vm, code: Arc<Code>) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    let result = vm.run_script(code).and_then(|installer| {
        vm.pin(installer.clone());
        let host = natives::host_object(vm)?;
        vm.call(
            installer,
            JsValue::Undefined,
            vec![JsValue::Object(GLOBAL), host],
        )
    });
    match result {
        Ok(_) => Ok(()),
        Err(thrown) => {
            let location = vm.page.throw_location.unwrap_or_default();
            Err(vm.uncaught(thrown, location))
        }
    }
}

#[cfg(test)]
mod tests {
    /** The sections must join into one function expression that installs the factory. */
    #[test]
    fn bootstrap_is_one_installer_that_defines_the_factory() {
        let text = super::SOURCE.trim_end();
        assert!(text.contains("(function installIndexedDB(global, host) {"));
        assert!(text.ends_with("})"));
        assert!(text.contains("defineProperty(attributeHolder(), 'indexedDB'"));
    }

    /** Sites inside the bootstrap's range report as native; page sites do not. */
    #[test]
    fn only_bootstrap_sites_are_native() {
        let tags = crate::v9::syntax::SITE_TAGS;
        assert!(super::is_bootstrap_site(
            super::BOOTSTRAP_OFFSET as u32 * tags
        ));
        assert!(!super::is_bootstrap_site(0));
    }
}
