/** Sparse Wasm bytes retain a bounded resident working set independently of logical length. */
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

pub(super) const PAGE: usize = 1024;
/** Reserve space for each map entry and allocator overhead alongside its backing bytes. */
pub(super) const PAGE_COST: usize = PAGE + 128;
const WASM_PAGE: usize = 65536;
pub(in crate::v9) const MAX_LOGICAL: u64 = 262_144 * 65_536;
const MAX_RESIDENT: usize = 2 * crate::constants::JS_STRING_UNITS;

/** Only nonzero pages consume memory; the logical byte length can be much larger. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(in crate::v9) struct SparseBuffer {
    pub length: usize,
    pages: BTreeMap<usize, Vec<u8>>,
}

/** Snapshot decoding rejects oversized pages and page maps while they are being read. */
impl<'de> Deserialize<'de> for SparseBuffer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{Error, MapAccess, SeqAccess, Visitor};
        use std::fmt;
        struct Page(Vec<u8>);
        impl<'de> Deserialize<'de> for Page {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct PageVisitor;
                impl<'de> Visitor<'de> for PageVisitor {
                    type Value = Page;
                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("one bounded Wasm memory page")
                    }
                    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Page, S::Error> {
                        let mut bytes = Vec::with_capacity(PAGE);
                        while let Some(byte) = seq.next_element()? {
                            if bytes.len() == PAGE {
                                return Err(S::Error::custom("sparse page exceeds its bound"));
                            }
                            bytes.push(byte);
                        }
                        if bytes.len() != PAGE {
                            return Err(S::Error::custom("invalid sparse page length"));
                        }
                        Ok(Page(bytes))
                    }
                }
                deserializer.deserialize_seq(PageVisitor)
            }
        }
        struct Pages(BTreeMap<usize, Vec<u8>>);
        impl<'de> Deserialize<'de> for Pages {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct PagesVisitor;
                impl<'de> Visitor<'de> for PagesVisitor {
                    type Value = Pages;
                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("bounded sparse Wasm pages")
                    }
                    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Pages, M::Error> {
                        let mut pages = BTreeMap::new();
                        while let Some(key) = map.next_key()? {
                            if pages.len() == MAX_RESIDENT / PAGE_COST {
                                return Err(M::Error::custom("too many sparse pages"));
                            }
                            let page: Page = map.next_value()?;
                            if pages.insert(key, page.0).is_some() {
                                return Err(M::Error::custom("duplicate sparse page"));
                            }
                        }
                        Ok(Pages(pages))
                    }
                }
                deserializer.deserialize_map(PagesVisitor)
            }
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            length: usize,
            pages: Pages,
        }
        let wire = Wire::deserialize(deserializer)?;
        let value = Self {
            length: wire.length,
            pages: wire.pages.0,
        };
        if !value.valid() {
            return Err(D::Error::custom("invalid sparse Wasm buffer"));
        }
        Ok(value)
    }
}
impl SparseBuffer {
    /** Create a zero-filled logical buffer without materializing its address space. */
    pub fn new(length: usize) -> Self {
        Self {
            length,
            pages: BTreeMap::new(),
        }
    }
    /** Charge allocated pages and conservative map overhead, never logical zero bytes. */
    pub fn resident(&self) -> usize {
        self.pages.len() * PAGE_COST
    }
    /** Detachment clears both visible length and retained resident pages. */
    pub fn clear(&mut self) {
        self.length = 0;
        self.pages.clear();
    }
    /** Restored metadata is bounded before it can be used by views or native instructions. */
    pub fn valid(&self) -> bool {
        (self.length as u64) <= MAX_LOGICAL
            && self.length % WASM_PAGE == 0
            && self.resident() <= MAX_RESIDENT
            && self.pages.iter().all(|(i, p)| {
                p.len() == PAGE && i.checked_mul(PAGE).is_some_and(|at| at < self.length)
            })
    }
    /** Reads copy the requested range and synthesize missing pages as zeros. */
    pub fn read(&self, start: usize, count: usize) -> Option<Vec<u8>> {
        if count > MAX_RESIDENT {
            return None;
        }
        let end = start.checked_add(count).filter(|end| *end <= self.length)?;
        let mut output = vec![0; count];
        let mut at = start;
        while at < end {
            let page = at / PAGE;
            let offset = at % PAGE;
            let take = (PAGE - offset).min(end - at);
            if let Some(bytes) = self.pages.get(&page) {
                output[at - start..at - start + take]
                    .copy_from_slice(&bytes[offset..offset + take]);
            }
            at += take;
        }
        Some(output)
    }
    /** Writes preflight every new page so quota failure cannot partially mutate the buffer. */
    pub fn write(&mut self, start: usize, bytes: &[u8]) -> bool {
        let Some(end) = start
            .checked_add(bytes.len())
            .filter(|end| *end <= self.length)
        else {
            return false;
        };
        if bytes.is_empty() {
            return true;
        }
        let Some(additional) = self.additional(start, bytes) else {
            return false;
        };
        if self.resident().saturating_add(additional) > MAX_RESIDENT {
            return false;
        }
        let mut at = start;
        while at < end {
            let page = at / PAGE;
            let offset = at % PAGE;
            let take = (PAGE - offset).min(end - at);
            let incoming = &bytes[at - start..at - start + take];
            if self.pages.contains_key(&page) || incoming.iter().any(|b| *b != 0) {
                let target = self.pages.entry(page).or_insert_with(|| vec![0; PAGE]);
                target[offset..offset + take].copy_from_slice(incoming);
            }
            at += take;
        }
        true
    }
    /** Page growth can be charged before the write mutates any existing contents. */
    pub fn additional(&self, start: usize, bytes: &[u8]) -> Option<usize> {
        let end = start
            .checked_add(bytes.len())
            .filter(|end| *end <= self.length)?;
        let (mut at, mut pages) = (start, 0usize);
        while at < end {
            let page = at / PAGE;
            let offset = at % PAGE;
            let take = (PAGE - offset).min(end - at);
            if !self.pages.contains_key(&page)
                && bytes[at - start..at - start + take].iter().any(|b| *b != 0)
            {
                pages += 1;
            }
            at += take;
        }
        Some(pages.saturating_mul(PAGE_COST))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /** Distant writes preserve zeros and data across both page boundaries. */
    fn sparse_round_trip_keeps_logical_zeros() {
        let mut bytes = SparseBuffer::new(WASM_PAGE * 1024);
        assert!(bytes.write(PAGE - 1, &[1, 2]));
        assert!(bytes.write(bytes.length - 1, &[3]));
        assert_eq!(bytes.read(PAGE - 2, 4), Some(vec![0, 1, 2, 0]));
        assert_eq!(bytes.read(bytes.length - 2, 2), Some(vec![0, 3]));
        assert_eq!(bytes.resident(), PAGE_COST * 3);
        assert!(bytes.valid());
    }

    #[test]
    /** Allocation failure leaves all existing pages and bytes untouched. */
    fn quota_failure_is_atomic() {
        let mut bytes = SparseBuffer::new(WASM_PAGE * 1024);
        let pages = MAX_RESIDENT / PAGE_COST;
        let end = pages * PAGE;
        assert!(bytes.write(0, &vec![7; end]));
        assert!(!bytes.write(end - 1, &[9, 9]));
        assert_eq!(bytes.read(end - 1, 2), Some(vec![7, 0]));
        assert_eq!(bytes.resident(), pages * PAGE_COST);
    }

    #[test]
    /** Forged snapshots cannot allocate pages beyond the live resident budget. */
    fn snapshot_rejects_invalid_pages_and_lengths() {
        assert!(
            serde_json::from_str::<SparseBuffer>("{\"length\":65536,\"pages\":{\"0\":[1]}}")
                .is_err()
        );
        assert!(
            serde_json::from_str::<SparseBuffer>("{\"length\":17179869185,\"pages\":{}}").is_err()
        );
        let mut buffer = SparseBuffer::new(WASM_PAGE);
        assert!(buffer.write(3, &[9]));
        let saved = serde_json::to_string(&buffer).unwrap();
        assert_eq!(
            serde_json::from_str::<SparseBuffer>(&saved)
                .unwrap()
                .read(3, 1),
            Some(vec![9])
        );
    }
}
