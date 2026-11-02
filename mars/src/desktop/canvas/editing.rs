/** Native input reconciliation preserves valid newer drafts and bounds rejected pastes. */
use super::*;
use crate::constants::TEXT_BYTES;
use objc2_foundation::NSRange;

impl Canvas {
    /** Validation focuses the associated native field and exposes its reason to accessibility. */
    pub fn focus_invalid(&self, id: usize, message: &str) {
        if let Some(widget) = self.ivars().widgets.borrow().get(&id) {
            widget.view().setToolTip(Some(&NSString::from_str(message)));
            widget.view().scrollRectToVisible(widget.view().bounds());
            if let Some(window) = self.window() {
                window.makeFirstResponder(Some(widget.view()));
            }
        }
    }
    /** Oversized input never enters the engine queue or remains as a false visible value. */
    pub(super) fn edit(&self, field: &NSTextField, page: String) {
        if self.ivars().reconciling.get() {
            return;
        }
        let id = (field.tag() - NODE_TAG) as usize;
        let text = field.stringValue().to_string();
        if text.len() > TEXT_BYTES {
            let value = self
                .ivars()
                .drafts
                .borrow()
                .get(&id)
                .cloned()
                .unwrap_or_else(|| self.committed(id).unwrap_or_default());
            self.restore(field, &value);
            self.push(Action::Rejected);
            return;
        }
        self.ivars().drafts.borrow_mut().insert(id, text.clone());
        self.push(Action::Edit(id, text, page));
    }
    /** Engine failures roll back only the rejected draft, leaving newer valid intentions intact. */
    pub fn reject_edit(&self, page: &str, id: usize, submitted: &str) {
        if self.page_id().as_deref() != Some(page) {
            return;
        }
        let widgets = self.ivars().widgets.borrow();
        let Some(Widget::Input(field)) = widgets.get(&id) else {
            return;
        };
        if field.stringValue().to_string() != submitted {
            return;
        }
        if let Some(value) = self.committed(id) {
            self.restore(field, &value);
            self.ivars().drafts.borrow_mut().insert(id, value);
        }
    }
    /** A fixed snapshot is the only fallback when no valid local draft exists. */
    pub(super) fn committed(&self, id: usize) -> Option<String> {
        self.ivars()
            .snapshot
            .borrow()
            .as_ref()?
            .document
            .as_ref()?
            .nodes
            .iter()
            .find(|node| node.local_id == id)
            .map(|node| node.value.clone())
    }
    /** Active field editors need explicit text and a valid UTF-16 selection after rollback. */
    pub(in crate::desktop) fn restore(&self, field: &NSTextField, value: &str) {
        self.ivars().reconciling.set(true);
        field.setStringValue(&NSString::from_str(value));
        if let Some(editor) = field.currentEditor() {
            let caret = editor
                .selectedRange()
                .location
                .min(value.encode_utf16().count());
            editor.setString(&NSString::from_str(value));
            editor.setSelectedRange(NSRange::new(caret, 0));
        }
        self.ivars().reconciling.set(false);
    }
}

#[cfg(feature = "test-support")]
impl Canvas {
    /** Local operator-only verification uses an actual AppKit field editor and delegate callback. */
    pub fn edit_for_check(
        &self,
        window: &objc2_app_kit::NSWindow,
        id: usize,
        text: &str,
    ) -> crate::protocol::EngineResult<String> {
        let widgets = self.ivars().widgets.borrow();
        let Some(Widget::Input(field)) = widgets.get(&id) else {
            return Err(crate::protocol::EngineError::invalid(
                "Missing native input.",
            ));
        };
        if !window.makeFirstResponder(Some(&**field)) {
            return Err(crate::protocol::EngineError::invalid(
                "Native field could not receive focus.",
            ));
        }
        let editor = field.currentEditor().ok_or_else(|| {
            crate::protocol::EngineError::invalid("Native field editor is absent.")
        })?;
        field.setStringValue(&NSString::from_str(text));
        editor.setString(&NSString::from_str(text));
        let notification = unsafe {
            NSNotification::notificationWithName_object(
                &NSString::from_str("NSControlTextDidChangeNotification"),
                Some(&**field),
            )
        };
        unsafe {
            let _: () = msg_send![self,controlTextDidChange:&*notification];
        }
        Ok(field.stringValue().to_string())
    }
    /** Native activation sends the real fixed selector through AppKit's target/action machinery. */
    pub fn click_for_check(&self, id: usize) -> crate::protocol::EngineResult<()> {
        let widgets = self.ivars().widgets.borrow();
        let button: &NSButton = match widgets.get(&id) {
            Some(Widget::Button(button)) => button,
            Some(Widget::Push(button)) => button,
            _ => {
                return Err(crate::protocol::EngineError::invalid(
                    "Missing native button.",
                ));
            }
        };
        unsafe {
            button.performClick(None);
        }
        Ok(())
    }
}
