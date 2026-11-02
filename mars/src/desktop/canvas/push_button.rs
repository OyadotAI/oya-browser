/** A native bordered button paints the engine's measured lines after AppKit's bezel. */
use super::*;
use std::cell::RefCell;
/** Immutable paint geometry contains no runtime or agent transport. */
#[derive(Default)]
pub(super) struct Paint {
    item: RefCell<Option<DisplayItem>>,
    /// Author-styled buttons draw no bezel: the canvas already painted their CSS box and children.
    styled: std::cell::Cell<bool>,
}
define_class!(
    #[unsafe(super=NSButton)]
    #[thread_kind=MainThreadOnly]
    #[ivars=Paint]
    pub(super) struct PushButton;
    unsafe impl NSObjectProtocol for PushButton {}
    impl PushButton {
        /** Local coordinates match the document's top-left origin. */
        #[unsafe(method(isFlipped))]
        fn flipped(&self)->bool {true}
        /** AppKit draws focus/pressed state; Oya chooses text wrapping. */
        #[unsafe(method(drawRect:))]
        fn draw(&self,dirty:NSRect) {
            unsafe {let _:()=msg_send![super(self),drawRect:dirty];}
            if let Some(item)=self.ivars().item.borrow().as_ref() {
                let authored=item.color.as_deref().filter(|_| self.ivars().styled.get()).and_then(literal_color);
                let color=match authored {Some(color)=>color,None if self.isEnabled()=>NSColor::controlTextColor(),None=>NSColor::disabledControlTextColor()};
                for line in &item.lines {platform::colored(&line.text,item.font_size,item.bold,&color).drawAtPoint(NSPoint::new(line.x-item.x,line.y-item.y));}
            }
        }
    }
);
impl PushButton {
    /** Activations use the audited fixed Canvas selector. */
    pub fn new(item: &DisplayItem, target: &Canvas) -> Retained<Self> {
        let this = Self::alloc(target.mtm()).set_ivars(Paint::default());
        let this: Retained<Self> =
            unsafe { msg_send![super(this),initWithFrame:rect(0.0,0.0,item.width,item.height)] };
        this.setTitle(&NSString::from_str(""));
        this.setTag(item.local_id as isize + NODE_TAG);
        this.setBordered(true);
        // Flexible push keeps a rounded native bezel at Chromium's 21 point height.
        this.setBezelStyle(NSBezelStyle::FlexiblePush);
        unsafe {
            this.setTarget(Some(target));
            this.setAction(Some(sel!(activate:)));
        }
        this.update(item, false);
        this
    }
    /** Reflow retains the native activation/focus object. A `styled` button (author background or
    border, as Chromium drops the native look for) loses its bezel so the page's own paint shows. */
    pub fn update(&self, item: &DisplayItem, styled: bool) {
        self.ivars().styled.set(styled);
        self.setBordered(!styled);
        *self.ivars().item.borrow_mut() = Some(item.clone());
        NSView::setNeedsDisplay(self, true);
    }
}
