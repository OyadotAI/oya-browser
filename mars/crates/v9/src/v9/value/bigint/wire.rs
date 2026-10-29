/** Inert signed-limb snapshots reject oversized and noncanonical magnitudes before admitting values. */
use super::*;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, SeqAccess, Visitor},
};

impl Serialize for BigInt {
    /** Immutable limb snapshots contain no identities, executable code or external authority. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.negative, self.words.as_ref()).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for BigInt {
    /** Bounded limb admission runs while reading the sequence rather than after allocating an unbounded Vec. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (negative, Words(words)) = <(bool, Words)>::deserialize(deserializer)?;
        if words.last() == Some(&0) || negative && words.is_empty() {
            return Err(D::Error::custom("noncanonical BigInt magnitude"));
        }
        Self::new(negative, words)
            .ok_or_else(|| D::Error::custom("BigInt magnitude exceeds retained-value bound"))
    }
}
/** Only this bounded reader may allocate serialized magnitude limbs. */
struct Words(Vec<u32>);
impl<'de> Deserialize<'de> for Words {
    /** A visitor stops before accepting the first limb beyond the current engine memory bound. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bounded;
        impl<'de> Visitor<'de> for Bounded {
            type Value = Words;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("bounded canonical BigInt limbs")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Words, A::Error> {
                if sequence
                    .size_hint()
                    .is_some_and(|length| length > MAX_WORDS)
                {
                    return Err(A::Error::custom("BigInt limb bound"));
                }
                let mut words = Vec::new();
                while let Some(word) = sequence.next_element::<u32>()? {
                    if words.len() >= MAX_WORDS {
                        return Err(A::Error::custom("BigInt limb bound"));
                    }
                    words.push(word);
                }
                Ok(Words(words))
            }
        }
        deserializer.deserialize_seq(Bounded)
    }
}
