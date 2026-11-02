/** Secure profile records stay bounded and reject malformed future data without values. */
use mars::{
    constants::STORAGE_AREA_BYTES,
    profile::{ProfileError, SessionProfile, WebStorageProfile},
};
use std::collections::BTreeMap;

/** Origin maps round-trip as strict private secure-store payloads. */
#[test]
fn web_storage_profile_round_trips_and_rejects_unknown_fields() {
    let profile = WebStorageProfile {
        version: 1,
        origins: BTreeMap::from([(
            "https://example.com".into(),
            BTreeMap::from([("key".into(), "value".into())]),
        )]),
    };
    let bytes = profile.encode().unwrap();
    assert_eq!(
        WebStorageProfile::decode(&bytes).unwrap().origins,
        profile.origins
    );
    assert_eq!(
        WebStorageProfile::decode(br#"{"version":1,"origins":{},"extra":"secret"}"#),
        Err(ProfileError::Corrupt)
    );
}

/** Session payloads contain only bounded credential-free HTTP(S) destinations. */
#[test]
fn session_profile_validates_destinations_count_and_selection() {
    let profile = SessionProfile {
        version: 1,
        urls: vec!["https://example.com/path?private=query".into()],
        active_index: 0,
    };
    let bytes = profile.encode().unwrap();
    assert_eq!(SessionProfile::decode(&bytes).unwrap(), profile);
    for url in ["file:///tmp/page", "https://user:password@example.com/"] {
        let invalid = SessionProfile {
            urls: vec![url.into()],
            ..profile.clone()
        };
        assert_eq!(invalid.encode(), Err(ProfileError::Corrupt));
    }
    let too_many = SessionProfile {
        urls: vec!["https://example.com/".into(); mars::constants::TABS + 1],
        active_index: 0,
        ..profile.clone()
    };
    assert_eq!(too_many.encode(), Err(ProfileError::Corrupt));
    let bad_index = SessionProfile {
        active_index: 1,
        ..profile
    };
    assert_eq!(bad_index.encode(), Err(ProfileError::Corrupt));
}

/** Invalid versions and oversized origin areas fail before secure persistence. */
#[test]
fn web_storage_profile_rejects_future_version_and_quota_overflow() {
    let future = br#"{"version":99,"origins":{}}"#;
    assert_eq!(
        WebStorageProfile::decode(future),
        Err(ProfileError::Corrupt)
    );
    let large = "x".repeat(STORAGE_AREA_BYTES + 1);
    let profile = WebStorageProfile {
        version: 1,
        origins: BTreeMap::from([(
            "https://example.com".into(),
            BTreeMap::from([("key".into(), large)]),
        )]),
    };
    assert_eq!(profile.encode(), Err(ProfileError::Corrupt));
}
