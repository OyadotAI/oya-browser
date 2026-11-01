/**
 * Declarative refresh (`<meta http-equiv=refresh>`), followed the way Chrome follows it. The
 * document's first refresh becomes a pending navigation due after its delay on the page's virtual
 * clock. When the clock reaches it, the tick queues an ordinary script-style navigation of kind
 * `refresh`, which replaces the current history entry (the spec's replace navigation) and runs
 * through the same loader, authority and sensitivity checks as a click. Bot interstitials depend
 * on this: their first page is a timed refresh to the verified address, and without it the page
 * stays on the interstitial's blank frame. A refresh chain is bounded so a page that refreshes
 * itself forever stops after META_REFRESHES hops; a person's or agent's own navigation starts a
 * new chain.
 */
use super::scripting::Scripting;
use crate::document::Document;
use crate::v9::NavigationRequest;
use url::Url;

/** The navigation kind a due refresh queues; follow_script_navigation recognises it. */
pub(super) const REFRESH_KIND: &str = "refresh";
/** Refresh navigations one chain may make in a row before the bound refuses the next. */
pub(super) const META_REFRESHES: u8 = 5;

/** A refresh waiting for its delay: the virtual time it is due and the absolute address it goes to. */
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingRefresh {
    /// The virtual millisecond at which the navigation starts.
    pub due_ms: f64,
    /// The absolute http or https address to navigate to.
    pub url: String,
}

/** The pending refresh a parsed document declares, resolved against its address; None when there is none or it names a non-web address, which Chrome does not follow. */
pub(super) fn pending(document: &Document, base: &str) -> Option<PendingRefresh> {
    let refresh = document.refresh.as_ref()?;
    let base = Url::parse(base).ok()?;
    let target = match &refresh.url {
        Some(address) => base.join(address).ok()?,
        None => base,
    };
    if !["http", "https"].contains(&target.scheme()) {
        return None;
    }
    Some(PendingRefresh {
        due_ms: refresh.seconds.max(0.0) * super::scripting::MS_PER_SECOND,
        url: target.into(),
    })
}

/** Queues the refresh's navigation once the virtual clock has reached its due time. The refresh is taken, so it fires once; a navigation a script already asked for keeps its place. */
pub(super) fn fire_due(scripting: &mut Scripting, now_ms: f64) {
    let Some(refresh) = scripting.refresh.as_ref() else {
        return;
    };
    if now_ms < refresh.due_ms || scripting.navigation.is_some() {
        return;
    }
    if let Some(refresh) = scripting.refresh.take() {
        scripting.navigation = Some(NavigationRequest {
            kind: REFRESH_KIND.into(),
            url: refresh.url,
            delta: 0,
        });
    }
}
