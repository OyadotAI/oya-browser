/** Worker module graphs share V9's loader while retaining worker-specific policy and credentials. */
use super::*;
use crate::constants::{MODULE_BYTES, MODULES, SCRIPT_BYTES};
use crate::v9::{EntryKind, ModuleEntry, ModuleInput, ModuleRequest, ModuleSource};

/** Every source is bounded and belongs only to this worker, including dynamic import results. */
#[derive(Clone)]
pub(super) struct Graph {
    /// The same inert module input used by page V9 execution, without any document capability.
    pub input: ModuleInput,
    /// Missing graph URLs are acquired before queued worker messages can run.
    pub pending: Vec<ModuleRequest>,
    /// Constructor credentials mode is immutable across static and dynamic imports.
    credentials: WorkerCredentials,
    /// Source and URL bytes include the initially acquired worker source.
    bytes: usize,
}

impl Graph {
    /** Classic workers may use dynamic import without treating their initial source as a module. */
    pub fn new(kind: WorkerType, credentials: WorkerCredentials, url: &str, source: &str) -> Self {
        let entries = if kind == WorkerType::Module {
            vec![ModuleEntry {
                index: 0,
                kind: EntryKind::Module,
                url: Some(url.into()),
            }]
        } else {
            Vec::new()
        };
        let sources = if kind == WorkerType::Module {
            vec![ModuleSource {
                url: url.into(),
                source: Some(source.into()),
                classic: false,
            }]
        } else {
            Vec::new()
        };
        Self {
            input: ModuleInput { entries, sources },
            pending: Vec::new(),
            credentials: if kind == WorkerType::Classic {
                WorkerCredentials::SameOrigin
            } else {
                credentials
            },
            bytes: source.len().saturating_add(url.len()),
        }
    }

    /** Retained graph growth fails explicitly rather than losing failed-source records and retrying. */
    fn deliver(&mut self, url: String, source: Option<String>) -> EngineResult<()> {
        let bytes = self
            .bytes
            .saturating_add(url.len())
            .saturating_add(source.as_ref().map_or(0, String::len));
        if self.input.sources.len() >= MODULES
            || bytes > MODULE_BYTES
            || source
                .as_ref()
                .is_some_and(|source| source.len() > SCRIPT_BYTES)
        {
            return Err(EngineError::limit("Worker module graph", MODULE_BYTES));
        }
        self.bytes = bytes;
        self.input.sources.push(ModuleSource {
            url,
            source,
            classic: false,
        });
        Ok(())
    }
}

impl Runtime {
    /** One graph round has one acquisition deadline, independent of how many URLs it discovers. */
    pub(super) fn worker_modules(
        &mut self,
        worker: &mut Context,
        until: Instant,
    ) -> EngineResult<()> {
        let deadline = until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
        for request in std::mem::take(&mut worker.modules.pending) {
            if worker
                .modules
                .input
                .sources
                .iter()
                .any(|known| known.url == request.url)
            {
                continue;
            }
            let source = self.worker_module_source(
                &worker.policy,
                &request.url,
                false,
                worker.modules.credentials,
                deadline,
            )?;
            let source = source.map(|source| source.text).filter(|source| {
                request.integrity.as_deref().is_none_or(|integrity| {
                    super::super::integrity::matches(integrity, source.as_bytes())
                })
            });
            worker.modules.deliver(request.url, source)?;
        }
        Ok(())
    }
}
