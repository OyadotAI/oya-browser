/**
 * The Rust ports held to vectors produced by Oya's own TypeScript: governed-egress verdicts
 * (fixtures/egress/vectors.json, shared with the TypeScript copies) and persona devices and
 * identities (tests/vectors/persona.json). Same input, same verdict, same device, forever.
 */
use mars_link::identity::native_policy;
use mars_link::prng::seed_from_string;
use mars_link::profile::{Profile, SeedIdentity, generate};
use mars_link::{Persona, allowed_host};
use serde_json::{Value, json};

/// A JSON file relative to this crate.
fn load(path: &str) -> Value {
    let text =
        std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).expect(path);
    serde_json::from_str(&text).expect(path)
}

/// The persona vectors.
fn persona_vectors() -> Value {
    load("tests/vectors/persona.json")
}

#[test]
fn egress_matcher_gives_the_typescript_verdict_on_every_shared_vector() {
    let file = load("../../fixtures/egress/vectors.json");
    let vectors = file["vectors"].as_array().expect("vectors");
    assert!(
        vectors.len() > 1000,
        "the shared file covers the rule forms"
    );
    for v in vectors {
        let host = v["host"].as_str().unwrap();
        let rules: Vec<&str> = v["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        assert_eq!(
            allowed_host(host, &rules),
            v["allowed"].as_bool().unwrap(),
            "{host:?} against {rules:?}"
        );
    }
}

#[test]
fn seed_strings_hash_to_oyas_seeds() {
    for seed in persona_vectors()["seeds"].as_array().unwrap() {
        assert_eq!(
            u64::from(seed_from_string(seed["string"].as_str().unwrap())),
            seed["seed"].as_u64().unwrap()
        );
    }
}

/// A profile as JSON with its noise floats taken out, and those floats' bits.
fn split_noise(mut profile: Value) -> (Value, Vec<u64>) {
    let bits =
        ["canvas", "audio", "rects"].map(|k| profile[k]["noiseSeed"].as_f64().unwrap().to_bits());
    for k in ["canvas", "audio", "rects"] {
        profile[k]["noiseSeed"] = Value::Null;
    }
    (profile, bits.to_vec())
}

#[test]
fn every_seed_and_prefs_gives_oyas_device_pick_for_pick() {
    for v in persona_vectors()["profiles"].as_array().unwrap() {
        let identity: SeedIdentity = serde_json::from_value(v["identity"].clone()).unwrap();
        let generated = serde_json::to_value(generate(&identity)).unwrap();
        let (ours, our_bits) = split_noise(generated);
        let (theirs, _) = split_noise(v["profile"].clone());
        assert_eq!(ours, theirs, "device for {}", v["identity"]);
        let expected: Vec<u64> = ["canvas", "audio", "rects"]
            .iter()
            .map(|k| u64::from_str_radix(v["noise"][k].as_str().unwrap(), 16).unwrap())
            .collect();
        assert_eq!(our_bits, expected, "noise seeds for {}", v["identity"]);
    }
}

#[test]
fn the_same_persona_gets_the_same_device_every_time() {
    let identity = SeedIdentity {
        id: "p-1".into(),
        seed: seed_from_string("p-1"),
        ..Default::default()
    };
    assert_eq!(generate(&identity), generate(&identity));
}

#[test]
fn native_identity_matches_oya_for_every_platform_engine_gpu_and_language() {
    for v in persona_vectors()["identities"].as_array().unwrap() {
        let mut persona = v["persona"].clone();
        persona["id"] = "p".into();
        let profile: Profile = serde_json::from_value(persona).unwrap();
        let policy = native_policy(&profile, v["engine"].as_str().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&policy).unwrap(),
            v["policy"],
            "policy for {}",
            v["persona"]
        );
        let unit = Persona::from_welcome(&json!({"fingerprint": profile})).unwrap();
        let identity = unit.identity(v["engine"].as_str().unwrap()).unwrap();
        let hints: serde_json::Map<String, Value> = identity
            .client_hints
            .into_iter()
            .map(|(k, v)| (k, v.into()))
            .collect();
        assert_eq!(
            Value::Object(hints),
            v["hints"],
            "client hints for {}",
            v["persona"]
        );
        assert_eq!(
            identity
                .accept_language
                .map(Value::from)
                .unwrap_or(Value::Null),
            v["acceptLanguage"]
        );
    }
}
