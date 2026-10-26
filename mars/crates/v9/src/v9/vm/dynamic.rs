use super::data::FunctionKind;
/** Dynamic code is retained by live closures, suspended activations and lexical declarations, not by execution history. */
use super::*;
use crate::constants::{VM_DYNAMIC_BYTES, VM_DYNAMIC_SOURCES};
use crate::v9::compile::Op;
use crate::v9::syntax::SITE_TAGS;
use std::collections::HashSet;

/** Stable source offsets allow unreachable units to disappear without changing surviving function identities. */
pub(in crate::v9) fn source_base(sources: &[DynamicSource], index: usize) -> usize {
    sources[index]
        .base
        .unwrap_or_else(|| crate::v9::builtins::dynamic_base(&sources[..index]))
}

/** Saved ranges may have gaps after collection, but may never overlap or escape the compiler's site namespace. */
pub(in crate::v9) fn validate_sources(
    sources: &[DynamicSource],
) -> crate::protocol::EngineResult<()> {
    let mut end = crate::v9::DYNAMIC_OFFSET;
    let mut bytes = 0usize;
    for source in sources {
        let base = source.base.unwrap_or(end);
        let next = base
            .checked_add(source.source.len())
            .and_then(|end| end.checked_add(1));
        if base < end || next.is_none_or(|next| next > u32::MAX as usize / SITE_TAGS as usize) {
            return Err(EngineError::invalid("Invalid V9 dynamic source ranges."));
        }
        end = next.expect("checked range");
        bytes = bytes
            .checked_add(source.source.len())
            .ok_or_else(|| EngineError::invalid("Invalid V9 dynamic source size."))?;
    }
    if bytes > VM_DYNAMIC_BYTES || sources.len() > VM_DYNAMIC_SOURCES {
        return Err(EngineError::limit("V9 dynamic code", VM_DYNAMIC_BYTES));
    }
    Ok(())
}

impl Vm<'_> {
    /** A real major collection precedes source-pressure reclamation, preserving every surviving weak or strong code root. */
    pub(in crate::v9) fn prepare_dynamic(&mut self, bytes: usize) -> Res<()> {
        if self.dynamic_full(bytes) {
            self.collect_now()?;
            self.reclaim_dynamic();
        }
        if self.dynamic_full(bytes) {
            return Err(EngineError::limit("V9 dynamic code", VM_DYNAMIC_BYTES).into());
        }
        let base = crate::v9::builtins::dynamic_base(&self.dynamic);
        if base
            .checked_add(bytes)
            .and_then(|end| end.checked_add(1))
            .is_none_or(|end| end > u32::MAX as usize / SITE_TAGS as usize)
        {
            return Err(EngineError::limit(
                "V9 dynamic source site namespace",
                u32::MAX as usize / SITE_TAGS as usize,
            )
            .into());
        }
        Ok(())
    }
    /** Existing quotas apply to resident source bytes and live source units after reclamation. */
    fn dynamic_full(&self, bytes: usize) -> bool {
        self.dynamic.len() >= VM_DYNAMIC_SOURCES
            || self
                .dynamic
                .iter()
                .map(|source| source.source.len())
                .sum::<usize>()
                .saturating_add(bytes)
                > VM_DYNAMIC_BYTES
    }
    /** Unit granularity preserves all nested declarations a surviving function may create on its next invocation. */
    fn reclaim_dynamic(&mut self) {
        let mut next = crate::v9::DYNAMIC_OFFSET;
        for source in &mut self.dynamic {
            let base = source.base.unwrap_or(next);
            source.base = Some(base);
            next = base + source.source.len() + 1;
        }
        let mut live = self
            .dynamic_sites()
            .into_iter()
            .map(|site| site as usize / SITE_TAGS as usize)
            .collect::<Vec<_>>();
        live.sort_unstable();
        live.dedup();
        let lexical = self
            .registry
            .codes
            .values()
            .filter(|code| {
                code.ops
                    .iter()
                    .any(|op| matches!(op, Op::DeclareLexical(..)))
            })
            .filter_map(|code| code.source.as_ref().map(|(_, base)| *base))
            .collect::<HashSet<_>>();
        let mut removed = HashSet::new();
        let mut indices = Vec::with_capacity(self.dynamic.len());
        let mut retained = Vec::new();
        for (index, source) in self.dynamic.iter().enumerate() {
            let base = source_base(&self.dynamic, index);
            let last = base + source.source.len();
            let first = live.partition_point(|offset| *offset < base);
            if source.context.is_none()
                || lexical.contains(&base)
                || live.get(first).is_some_and(|offset| *offset <= last)
            {
                indices.push(Some(retained.len()));
                let mut source = source.clone();
                source.base = Some(base);
                retained.push(source);
            } else {
                indices.push(None);
                removed.insert(base);
            }
        }
        self.dynamic = retained;
        self.registry.codes.retain(|_, code| {
            code.source
                .as_ref()
                .is_none_or(|(_, base)| !removed.contains(base))
        });
        #[cfg(feature = "conformance")]
        self.realms
            .scripts
            .retain_mut(|(index, _)| match indices.get(*index).copied().flatten() {
                Some(next) => {
                    *index = next;
                    true
                }
                None => false,
            });
    }
    /** Every retained object is reachable after the pressure collection, including generator and mapped-argument code records. */
    fn dynamic_sites(&self) -> HashSet<u32> {
        let mut sites = self
            .frames
            .iter()
            .map(|frame| frame.code.site)
            .collect::<HashSet<_>>();
        for object in self.heap.objects.values() {
            if let Some(function) = object.function()
                && let FunctionKind::Closure { site, .. } = function.kind
            {
                sites.insert(site);
            }
            let frame = match &object.class {
                Class::Generator(data) => data.frame.as_ref(),
                Class::AsyncGenerator(data) => data.frame.as_ref(),
                Class::Async(data) => data.frame.as_ref(),
                _ => None,
            };
            if let Some(frame) = frame {
                sites.insert(frame.site);
            }
            if let Class::MappedArguments(data) = &object.class {
                sites.insert(data.site);
            }
        }
        for environment in self.heap.envs.values() {
            if let Some(site) = environment.class_site {
                sites.insert(site);
            }
        }
        sites
    }
}
