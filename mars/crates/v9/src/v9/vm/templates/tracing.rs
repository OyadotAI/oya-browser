/** A template cache belongs to a parsed Script: strong code roots keep its objects alive, discarded sources do not. */
use super::super::{
    Vm,
    data::{Class, FunctionKind},
    gc::Marker,
};
use std::collections::BTreeSet;

impl Vm<'_> {
    /** Page/module scripts and retained lexical dynamic units live with their reachable realm. */
    pub(in crate::v9::vm) fn trace_persistent_templates(&self, marker: &mut Marker, realm: u32) {
        if self.templates.is_empty() {
            return;
        }
        let sources = self
            .templates
            .0
            .keys()
            .filter(|key| key.0 == realm)
            .map(|key| key.1)
            .collect::<BTreeSet<_>>();
        for base in sources {
            let dynamic = self.dynamic.iter().enumerate().find(|(index, _)| {
                super::super::dynamic_source_base(&self.dynamic, *index) == base
            });
            let persistent = dynamic.is_none_or(|(_, source)| source.context.is_none())
                || self.registry.codes.values().any(|code| {
                    code.source
                        .as_ref()
                        .is_some_and(|(_, start)| *start == base)
                        && code
                            .ops
                            .iter()
                            .any(|op| matches!(op, crate::v9::compile::Op::DeclareLexical(..)))
                });
            if persistent {
                self.trace_template_source(marker, realm, base)
            }
        }
    }

    /** A live function or activation retains all literal slots belonging to its parsed source. */
    pub(in crate::v9::vm) fn trace_template_site(
        &self,
        marker: &mut Marker,
        realm: u32,
        site: u32,
    ) {
        if let Some(base) = self
            .registry
            .get(site)
            .and_then(|code| code.source.as_ref().map(|(_, base)| *base))
        {
            self.trace_template_source(marker, realm, base);
        }
    }

    /** Conditional roots are discovered during ordinary tracing, so weak-map ephemerons see them in the same fixed point. */
    fn trace_template_source(&self, marker: &mut Marker, realm: u32, base: usize) {
        if !marker.template_sources.insert((realm, base)) {
            return;
        }
        for (_, id) in self
            .templates
            .0
            .range((realm, base, 0, 0)..=(realm, base, u32::MAX, u32::MAX))
        {
            marker.object(*id)
        }
    }

    /** Suspended state and mapped arguments retain source ownership even without an ordinary visible closure. */
    pub(in crate::v9::vm) fn trace_template_owner(
        &self,
        marker: &mut Marker,
        realm: u32,
        class: &Class,
    ) {
        if self.templates.is_empty() {
            return;
        }
        let site = match class {
            Class::Function(function) => match function.kind {
                FunctionKind::Closure { site, .. } => Some(site),
                _ => None,
            },
            Class::Generator(data) => data.frame.as_ref().map(|frame| frame.site),
            Class::AsyncGenerator(data) => data.frame.as_ref().map(|frame| frame.site),
            Class::Async(data) => data.frame.as_ref().map(|frame| frame.site),
            Class::MappedArguments(data) => Some(data.site),
            _ => None,
        };
        if let Some(site) = site {
            self.trace_template_site(marker, realm, site)
        }
    }

    /** A dead source must not leave a cache entry that could alias a later source reusing its byte offsets. */
    pub(in crate::v9::vm) fn prune_template_sources(&mut self) {
        self.templates.0.retain(|&(_, base, site, _), _| {
            self.registry.get(site).is_some_and(|code| {
                code.source
                    .as_ref()
                    .is_some_and(|(_, start)| *start == base)
            })
        });
    }
}
