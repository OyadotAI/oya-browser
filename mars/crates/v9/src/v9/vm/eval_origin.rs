/** Eval provenance copies only bounded diagnostic text from the real caller, never retaining parent code or foreign objects. */
use super::*;
use crate::constants::VM_DYNAMIC_BYTES;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::syntax::SITE_TAGS;

/** One parsed eval owns its actual creation origin and optional scanner-recognized source URL. */
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(in crate::v9) struct EvalOrigin {
    /// Native compilation contexts partition dynamic Function caching; this never selects runtime authority.
    #[serde(default)]
    pub realm: u32,
    /// Rendered nesting records the actual source creation chain without rooting earlier programs.
    #[serde(deserialize_with = "bounded_text")]
    pub origin: String,
    /// A genuine magic comment affects sourceURL accessors, not the script's filename.
    #[serde(default, deserialize_with = "bounded_optional_text")]
    pub source_url: Option<String>,
}

impl EvalOrigin {
    /** Metadata shares the existing resident dynamic-source byte budget. */
    pub fn bytes(&self) -> usize {
        self.origin
            .len()
            .saturating_add(self.source_url.as_ref().map_or(0, String::len))
            .saturating_add(std::mem::size_of::<u32>())
    }
    /** A source URL names an eval origin directly, as in V8 FormatEvalOrigin. */
    pub fn display(&self) -> &str {
        self.source_url.as_deref().unwrap_or(&self.origin)
    }
}

/** Decoding cannot copy an oversized origin before ordinary dynamic-source validation runs. */
fn bounded_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    struct Text;
    impl<'de> serde::de::Visitor<'de> for Text {
        type Value = String;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bounded eval origin")
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<String, E> {
            if value.len() > VM_DYNAMIC_BYTES {
                return Err(E::custom("V9 eval origin exceeds the dynamic-code budget"));
            }
            Ok(value.to_owned())
        }
    }
    deserializer.deserialize_str(Text)
}

/** Optional sourceURL decoding has the same early bound as its companion creation origin. */
fn bounded_optional_text<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    struct Optional;
    impl<'de> serde::de::Visitor<'de> for Optional {
        type Value = Option<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an optional bounded source URL")
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: serde::Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            bounded_text(deserializer).map(Some)
        }
    }
    deserializer.deserialize_option(Optional)
}

impl Vm<'_> {
    /** Scan a successfully compiled source before installing it, preserving catchable parser error ordering. */
    pub(in crate::v9) fn finish_eval_origin(
        &mut self,
        source: &str,
        mut origin: EvalOrigin,
    ) -> Res<EvalOrigin> {
        origin.source_url = crate::v9::lexer::source_url(source)?;
        self.prepare_dynamic(source.len().saturating_add(origin.bytes()))?;
        Ok(origin)
    }
    /** Every function in an eval source uses that source's provenance, including calls made after the creator returned. */
    pub(in crate::v9) fn eval_origin(&self, site: u32) -> Option<&EvalOrigin> {
        let offset = (site / SITE_TAGS) as usize;
        self.dynamic.iter().enumerate().find_map(|(index, source)| {
            let base = dynamic_source_base(&self.dynamic, index);
            (offset >= base && offset <= base + source.source.len())
                .then_some(source.origin.as_ref())
                .flatten()
        })
    }

    /** Capture happens before compilation while the real caller PC and realm are still active. */
    pub(in crate::v9) fn capture_eval_origin(&self) -> EngineResult<EvalOrigin> {
        let source_url = None;
        let Some(caller) = self.frames.last() else {
            return Ok(EvalOrigin {
                realm: self.active_realm,
                origin: "eval at <anonymous>".to_owned(),
                source_url,
            });
        };
        let name = caller.code.name.lossy();
        let name = if name.is_empty() {
            "<anonymous>"
        } else {
            &name
        };
        let location = if let Some(parent) = self.eval_origin(caller.code.site) {
            parent.display().to_owned()
        } else {
            let pos = caller.code.position(caller.pc.saturating_sub(1));
            format!(
                "{}:{}:{}",
                self.modules.source_name(pos.offset as usize),
                pos.line,
                pos.column
            )
        };
        let size = "eval at  ()"
            .len()
            .saturating_add(name.len())
            .saturating_add(location.len());
        if size > VM_DYNAMIC_BYTES {
            return Err(EngineError::limit("V9 dynamic code", VM_DYNAMIC_BYTES));
        }
        Ok(EvalOrigin {
            realm: self.active_realm,
            origin: format!("eval at {name} ({location})"),
            source_url,
        })
    }
}
