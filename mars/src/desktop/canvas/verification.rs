/** Test builds observe actual AppKit frames without reading control contents or granting page authority. */
use super::*;
impl Canvas {
    /** Screenshot tests locate validation ink in root-view coordinates without exposing field contents. */
    pub(in crate::desktop) fn validation_regions(&self, root: &NSView) -> serde_json::Value {
        let snapshot = self.ivars().snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return serde_json::json!([]);
        };
        let Some(display) = snapshot.display.as_ref() else {
            return serde_json::json!([]);
        };
        serde_json::Value::Array(display.items.iter().flat_map(|item| item.validation.iter().map(move |line| (item, line))).map(|(item, line)| {
            let frame = self.convertRect_toView(rect(line.x, line.y, line.width, item.font_size * 1.5), Some(root));
            let y = if root.isFlipped() { frame.origin.y } else { root.bounds().size.height - frame.origin.y - frame.size.height };
            serde_json::json!({"x":frame.origin.x,"y":y,"width":frame.size.width,"height":frame.size.height})
        }).collect())
    }
    /** Native-frame evidence lets window tests compare platform widgets with the owned display list. */
    pub(in crate::desktop) fn native_frames(&self) -> serde_json::Value {
        let snapshot = self.ivars().snapshot.borrow();
        let Some(document) = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.document.as_ref())
        else {
            return serde_json::json!([]);
        };
        serde_json::Value::Array(self.ivars().widgets.borrow().iter().filter_map(|(id, widget)| {
            let node = document.nodes.iter().find(|node| node.local_id == *id)?;
            let frame = widget.view().frame();
            Some(serde_json::json!({"html_id":node.html_id,"x":frame.origin.x,"y":frame.origin.y,"width":frame.size.width,"height":frame.size.height}))
        }).collect())
    }
}
