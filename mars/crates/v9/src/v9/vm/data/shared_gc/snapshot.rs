/** Shared GC snapshots decode bounded inert bytes; native attachment requires a separate granted root. */
use super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    identity: u64,
    revision: u64,
    node: u64,
    payload: Blob,
}
struct Blob(Vec<u8>);
impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        super::super::Bytes(self.0.clone()).serialize(s)
    }
}
impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        if text.len() > MAX_BYTES * 2 || text.len() % 2 != 0 || !text.is_ascii() {
            return Err(serde::de::Error::custom("invalid shared GC snapshot size"));
        }
        let bytes = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?;
        Ok(Self(bytes))
    }
}
impl Serialize for SharedGc {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let state = self.root.arena.lock();
        Wire {
            identity: self.arena().identity(),
            revision: state.revision,
            node: self.node(),
            payload: Blob(encode(&state.subgraph(self.node()))),
        }
        .serialize(s)
    }
}
impl<'de> Deserialize<'de> for SharedGc {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(d)?;
        if wire.identity == 0 || wire.revision > u64::MAX / 2 {
            return Err(serde::de::Error::custom("invalid shared GC identity"));
        }
        let mut state = decode(&wire.payload.0).map_err(|e| serde::de::Error::custom(e.message))?;
        state.revision = wire.revision;
        if state.records.values().any(|r| r.version > state.revision) {
            return Err(serde::de::Error::custom("invalid shared GC revision"));
        }
        let arena = SharedGcArena {
            store: Arc::new(Store {
                id: wire.identity,
                state: Mutex::new(state),
            }),
        };
        let result = arena
            .root_locked(&mut arena.lock(), wire.node, true)
            .map_err(|e| serde::de::Error::custom(e.message))?;
        Ok(result)
    }
}
/** Length-prefixed fields make every allocation bounded by the already checked payload remainder. */
fn encode(state: &State) -> Vec<u8> {
    let mut bytes = Vec::new();
    put(&mut bytes, state.next);
    count(&mut bytes, state.modules.len());
    for module in &state.modules {
        count(&mut bytes, module.len());
        bytes.extend(&**module);
    }
    count(&mut bytes, state.records.len());
    for (node, record) in &state.records {
        put(&mut bytes, *node);
        put(&mut bytes, record.version);
        bytes.extend(record.module.to_le_bytes());
        bytes.extend(record.ty.to_le_bytes());
        bytes.push(u8::from(record.array));
        count(&mut bytes, record.values.len());
        for value in &record.values {
            match value {
                SharedGcValue::Undefined => bytes.push(0),
                SharedGcValue::Null => bytes.push(1),
                SharedGcValue::Boolean(v) => bytes.push(if *v { 3 } else { 2 }),
                SharedGcValue::Number(v) => {
                    bytes.push(4);
                    put(&mut bytes, *v);
                }
                SharedGcValue::BigInt(v) => {
                    bytes.push(5);
                    bytes.extend(v.to_le_bytes());
                }
                SharedGcValue::Vector(v) => {
                    bytes.push(6);
                    bytes.extend(v);
                }
                SharedGcValue::String(v) => {
                    bytes.push(7);
                    count(&mut bytes, v.len());
                    for unit in &**v {
                        bytes.extend(unit.to_le_bytes());
                    }
                }
                SharedGcValue::BigIntWide(value) => {
                    bytes.push(9);
                    bytes.push(u8::from(value.negative()));
                    count(&mut bytes, value.words().len());
                    for word in value.words() {
                        bytes.extend(word.to_le_bytes());
                    }
                }
                SharedGcValue::Reference(v) => {
                    bytes.push(8);
                    put(&mut bytes, *v);
                }
            }
        }
    }
    bytes
}
fn put(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
fn count(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend((value as u32).to_le_bytes());
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> EngineResult<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or_else(invalid)?;
        let value = self.bytes.get(self.at..end).ok_or_else(invalid)?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> EngineResult<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> EngineResult<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> EngineResult<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn count(&mut self) -> EngineResult<usize> {
        let n = self.u32()? as usize;
        if n > self.bytes.len() - self.at {
            return Err(invalid());
        }
        Ok(n)
    }
    fn value(&mut self, remaining: usize) -> EngineResult<SharedGcValue> {
        Ok(match self.byte()? {
            0 => SharedGcValue::Undefined,
            1 => SharedGcValue::Null,
            2 => SharedGcValue::Boolean(false),
            3 => SharedGcValue::Boolean(true),
            4 => SharedGcValue::Number(self.u64()?),
            5 => SharedGcValue::BigInt(i128::from_le_bytes(self.take(16)?.try_into().unwrap())),
            6 => SharedGcValue::Vector(self.take(16)?.try_into().unwrap()),
            7 => {
                let n = self.count()?;
                if n > crate::constants::JS_STRING_UNITS || n > remaining / 2 {
                    return Err(invalid());
                }
                let units = self
                    .take(n.checked_mul(2).ok_or_else(invalid)?)?
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>();
                SharedGcValue::String(units.into())
            }
            8 => SharedGcValue::Reference(self.u64()?),
            9 => {
                let negative = self.byte()?;
                let n = self.count()?;
                if negative > 1
                    || n > crate::v9::value::BigInteger::word_limit()
                    || n > remaining / 4
                {
                    return Err(invalid());
                }
                let words = self
                    .take(n.checked_mul(4).ok_or_else(invalid)?)?
                    .chunks_exact(4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .collect::<Vec<_>>();
                if words.last() == Some(&0) || negative == 1 && words.is_empty() {
                    return Err(invalid());
                }
                let value = crate::v9::value::BigInteger::from_words(negative == 1, words)
                    .ok_or_else(invalid)?;
                if let Some(value) = value.to_i128() {
                    SharedGcValue::BigInt(value)
                } else {
                    SharedGcValue::BigIntWide(value)
                }
            }
            _ => return Err(invalid()),
        })
    }
}
fn decode(bytes: &[u8]) -> EngineResult<State> {
    let mut reader = Reader { bytes, at: 0 };
    let next = reader.u64()?;
    if next == 0 || next >= u64::MAX / 2 {
        return Err(invalid());
    }
    let module_count = reader.count()?;
    if module_count > MAX_BYTES / std::mem::size_of::<Arc<[u8]>>() {
        return Err(limit());
    }
    let mut decoded_bytes = module_count * std::mem::size_of::<Arc<[u8]>>();
    let mut modules = Vec::with_capacity(module_count);
    for _ in 0..module_count {
        let n = reader.count()?;
        decoded_bytes = decoded_bytes
            .checked_add(n)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        modules.push(Arc::from(reader.take(n)?));
    }
    let count = reader.count()?;
    if count > crate::constants::VM_OBJECTS || count > MAX_BYTES / NODE_BYTES {
        return Err(invalid());
    }
    let mut records = BTreeMap::new();
    for _ in 0..count {
        let node = reader.u64()?;
        let version = reader.u64()?;
        let module = reader.u32()?;
        let ty = reader.u32()?;
        let flag = reader.byte()?;
        if node == 0
            || node >= next
            || module as usize >= modules.len()
            || flag > 1
            || records.contains_key(&node)
        {
            return Err(invalid());
        }
        let n = reader.count()?;
        if n > crate::constants::VM_ARRAY_LENGTH {
            return Err(invalid());
        }
        let extra = NODE_BYTES
            + n.checked_mul(std::mem::size_of::<SharedGcValue>())
                .ok_or_else(limit)?;
        decoded_bytes = decoded_bytes
            .checked_add(extra)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            let value = reader.value(MAX_BYTES - decoded_bytes)?;
            decoded_bytes = decoded_bytes
                .checked_add(value.dynamic_bytes())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or_else(limit)?;
            values.push(value);
        }
        records.insert(
            node,
            SharedGcRecord {
                module,
                version,
                ty,
                array: flag == 1,
                values,
            },
        );
    }
    let mut state = State {
        next,
        modules,
        records,
        ..State::default()
    };
    state.bytes = state.computed_bytes();
    if reader.at != bytes.len()
        || state.allocated() > MAX_BYTES
        || !state
            .records
            .values()
            .all(|r| r.values.iter().all(|v| state.valid_value(v)))
    {
        return Err(invalid());
    }
    Ok(state)
}

impl State {
    /** A snapshot carries only nodes reachable from its granted root, never unrelated agents' records. */
    fn subgraph(&self, node: u64) -> Self {
        let mut output = State {
            next: self.next,
            revision: self.revision,
            ..State::default()
        };
        let mut pending = vec![node];
        let mut modules = BTreeMap::new();
        while let Some(node) = pending.pop() {
            if output.records.contains_key(&node) {
                continue;
            }
            if let Some(record) = self.records.get(&node) {
                let mut record = record.clone();
                let module = *modules.entry(record.module).or_insert_with(|| {
                    let index = output.modules.len() as u32;
                    output
                        .modules
                        .push(self.modules[record.module as usize].clone());
                    index
                });
                record.module = module;
                for value in &record.values {
                    if let SharedGcValue::Reference(id) = value {
                        pending.push(*id);
                    }
                }
                output.records.insert(node, record);
            }
        }
        output.bytes = output.computed_bytes();
        output
    }
}
impl SharedGcArena {
    /** Inert aliases merge by per-record revisions; this never attaches them to a native granted arena. */
    pub(in crate::v9) fn merge_snapshot(&self, other: &SharedGc) -> EngineResult<SharedGc> {
        if self.same_store(&other.arena()) {
            return Ok(other.clone());
        }
        if !other.restored() || self.identity() != other.arena().identity() {
            return Err(invalid());
        }
        let source = other.root.arena.lock().clone();
        let mut target = self.lock();
        let mut merged = target.clone();
        for (id, record) in source.records {
            let module = &source.modules[record.module as usize];
            let index = merged
                .modules
                .iter()
                .position(|m| m == module)
                .unwrap_or_else(|| {
                    merged.modules.push(module.clone());
                    merged.modules.len() - 1
                }) as u32;
            let mut record = record;
            record.module = index;
            if let Some(old) = merged.records.get(&id) {
                if old.ty != record.ty
                    || old.array != record.array
                    || old.module != record.module
                    || old.version == record.version && *old != record
                {
                    return Err(invalid());
                }
                if old.version > record.version {
                    continue;
                }
            }
            merged.records.insert(id, record);
        }
        merged.next = merged.next.max(source.next);
        merged.revision = merged.revision.max(source.revision);
        merged.bytes = merged.computed_bytes();
        if merged.allocated() > MAX_BYTES {
            return Err(limit());
        }
        *target = merged;
        self.root_locked(&mut target, other.node(), true)
    }
}
