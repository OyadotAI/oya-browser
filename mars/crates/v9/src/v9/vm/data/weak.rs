/** Weak registrations retain holdings but never confer ownership of targets or tokens. */
use crate::v9::value::{JsValue, ObjectId};
use serde::{Deserialize, Serialize};

/** One registration; a cleared target means its holding is ready for a cleanup task. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct FinalizationCell {
    /** An object or unregistered symbol, cleared before its heap record is swept. */
    pub target: Option<JsValue>,
    /** The only user value this registration retains strongly. */
    pub holdings: JsValue,
    /** An optional weak identity used to remove all registrations sharing that token. */
    pub token: Option<JsValue>,
}

/** Registry-owned state survives requests; scheduled tasks carry only its weak numeric identity. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct FinalizationData {
    /** Callable supplied to the constructor, retained as long as the registry is reachable. */
    pub callback: ObjectId,
    /** Both live registrations and cleared registrations waiting to deliver their holdings. */
    pub cells: Vec<FinalizationCell>,
    /** Suppresses duplicate cleanup tasks until the current task finishes or throws. */
    pub scheduled: bool,
}
