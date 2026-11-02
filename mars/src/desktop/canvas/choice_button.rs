/** Native radio keyboard traversal follows the owned form/name group rather than Cocoa siblings. */
use super::*;
use objc2_app_kit::{NSButtonType, NSEvent};
/** Browser-owned identity and semantics cannot be changed by page source after construction. */
#[derive(Default)]
pub(super) struct Choice {
    id: std::cell::Cell<usize>,
    radio: std::cell::Cell<bool>,
}
define_class!(
    #[unsafe(super=NSButton)]
    #[thread_kind=MainThreadOnly]
    #[ivars=Choice]
    pub(super) struct ChoiceButton;
    unsafe impl NSObjectProtocol for ChoiceButton {}
    impl ChoiceButton {
        /** Arrow keys activate a neighbor in the same named form group; Space stays native. */
        #[unsafe(method(keyDown:))]
        fn key(&self,event:&NSEvent) {
            if self.ivars().radio.get() && [123,124,125,126].contains(&event.keyCode()) {
                if let Some(target)=self.target().and_then(|target|target.downcast::<Canvas>().ok()) {target.radio_neighbor(self.ivars().id.get(),if [123,126].contains(&event.keyCode()) {-1}else{1});}return;
            }
            unsafe {let _:()=msg_send![super(self),keyDown:event];}
        }
    }
);
impl ChoiceButton {
    /** Cocoa supplies checkbox/radio painting, focus and Space activation. */
    pub fn make(item: &DisplayItem, radio: bool, target: &Canvas) -> Retained<NSButton> {
        let this = Self::alloc(target.mtm()).set_ivars(Choice {
            id: std::cell::Cell::new(item.local_id),
            radio: std::cell::Cell::new(radio),
        });
        let this: Retained<Self> =
            unsafe { msg_send![super(this),initWithFrame:rect(0.0,0.0,item.width,item.height)] };
        this.setTitle(&NSString::from_str(""));
        this.setTag(item.local_id as isize + NODE_TAG);
        this.setButtonType(if radio {
            NSButtonType::Radio
        } else {
            NSButtonType::Switch
        });
        unsafe {
            this.setTarget(Some(target));
            this.setAction(Some(sel!(activate:)));
        }
        this.into_super()
    }
}
impl Canvas {
    /** Disabled and hidden radios never receive focus or native activation. */
    fn radio_neighbor(&self, id: usize, direction: isize) {
        let snapshot = self.ivars().snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return;
        };
        let Some(document) = &snapshot.document else {
            return;
        };
        let Some(node) = document.nodes.iter().find(|n| n.local_id == id) else {
            return;
        };
        let group = document
            .nodes
            .iter()
            .filter(|n| {
                n.tag == "input"
                    && n.input_type() == "radio"
                    && !n.disabled
                    && !n.hidden()
                    && n.style.interactive
                    && n.attributes.get("name") == node.attributes.get("name")
                    && crate::document::form_owner(document, n)
                        == crate::document::form_owner(document, node)
            })
            .collect::<Vec<_>>();
        let Some(index) = group.iter().position(|n| n.local_id == id) else {
            return;
        };
        let next =
            group[(index as isize + direction).rem_euclid(group.len() as isize) as usize].local_id;
        if let Some(widget) = self.ivars().widgets.borrow().get(&next)
            && let Some(window) = self.window()
        {
            window.makeFirstResponder(Some(widget.view()));
        }
        if let Some(page) = self.page_id() {
            self.push(Action::Click(next, page));
        }
    }
}
