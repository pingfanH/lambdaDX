//! AppKit text responder for miniquad, whose native view only emits raw chars.
//! The child view becomes first responder only while egui requests text input.
use std::cell::{Cell, RefCell};

use egui::{Event, ImeEvent};
use objc2::{
    declare_class, msg_send, msg_send_id, mutability,
    rc::Retained,
    runtime::{AnyObject, Sel},
    ClassType, DeclaredClass,
};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSTextInputClient, NSView};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSObjectProtocol, NSPoint, NSRange, NSRect,
    NSSize, NSString,
};

const NOT_FOUND: usize = isize::MAX as usize;

#[derive(Default)]
struct State {
    events: RefCell<Vec<Event>>,
    marked: RefCell<String>,
    composing: Cell<bool>,
    selection: Cell<NSRange>,
    cursor: Cell<NSRect>,
    command: Cell<bool>,
}

declare_class!(
    struct TextInputView;

    unsafe impl ClassType for TextInputView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "EguiMiniquadTextInputView";
    }
    impl DeclaredClass for TextInputView {
        type Ivars = State;
    }
    unsafe impl NSObjectProtocol for TextInputView {}

    unsafe impl TextInputView {
        #[method(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool { true }

        #[method(keyDown:)]
        fn key_down(&self, event: &NSEvent) {
            unsafe {
                let modifiers = event.modifierFlags();
                if modifiers.intersects(NSEventModifierFlags::NSEventModifierFlagCommand | NSEventModifierFlags::NSEventModifierFlagControl) {
                    self.forward(event, true);
                    return;
                }
                let was_composing = self.ivars().composing.get();
                self.ivars().command.set(false);
                let handled = self.inputContext().is_some_and(|context| context.handleEvent(event));
                // AppKit maps editing/navigation keys to doCommandBySelector.
                // Let miniquad/egui handle them except while selecting an IME candidate.
                if !was_composing && (!handled || self.ivars().command.get()) {
                    self.forward(event, true);
                }
            }
        }

        #[method(keyUp:)]
        fn key_up(&self, event: &NSEvent) {
            unsafe { self.forward(event, false); }
        }
    }

    unsafe impl NSTextInputClient for TextInputView {
        #[method(insertText:replacementRange:)]
        fn insert_text(&self, string: &AnyObject, _replacement: NSRange) {
            let text = plain_text(string);
            let mut events = self.ivars().events.borrow_mut();
            if !self.ivars().composing.replace(false) {
                if !text.is_empty() { events.push(Event::Text(text)); }
            } else {
                self.ivars().marked.borrow_mut().clear();
                events.push(Event::Ime(ImeEvent::Commit(text)));
                events.push(Event::Ime(ImeEvent::Disabled));
            }
        }

        #[method(doCommandBySelector:)]
        fn do_command(&self, _selector: Sel) { self.ivars().command.set(true); }

        #[method(setMarkedText:selectedRange:replacementRange:)]
        fn set_marked_text(&self, string: &AnyObject, selection: NSRange, _replacement: NSRange) {
            let text = plain_text(string);
            let mut events = self.ivars().events.borrow_mut();
            if !self.ivars().composing.replace(true) {
                events.push(Event::Ime(ImeEvent::Enabled));
            }
            *self.ivars().marked.borrow_mut() = text.clone();
            self.ivars().selection.set(selection);
            events.push(Event::Ime(ImeEvent::Preedit(text)));
        }

        #[method(unmarkText)]
        fn unmark_text(&self) {
            // The text is already present as egui's preedit; collapse its selection.
            let text = std::mem::take(&mut *self.ivars().marked.borrow_mut());
            if self.ivars().composing.replace(false) {
                self.ivars().events.borrow_mut().extend([
                    Event::Ime(ImeEvent::Commit(text)), Event::Ime(ImeEvent::Disabled)
                ]);
            }
        }

        #[method(selectedRange)]
        fn selected_range(&self) -> NSRange { self.ivars().selection.get() }

        #[method(markedRange)]
        fn marked_range(&self) -> NSRange {
            let text = self.ivars().marked.borrow();
            if text.is_empty() { NSRange::new(NOT_FOUND, 0) }
            else { NSRange::new(0, text.encode_utf16().count()) }
        }

        #[method(hasMarkedText)]
        fn has_marked_text(&self) -> bool { !self.ivars().marked.borrow().is_empty() }

        #[method_id(attributedSubstringForProposedRange:actualRange:)]
        fn substring(&self, _range: NSRange, actual: *mut NSRange) -> Option<Retained<NSAttributedString>> {
            if !actual.is_null() { unsafe { *actual = NSRange::new(NOT_FOUND, 0); } }
            None
        }

        #[method_id(validAttributesForMarkedText)]
        fn valid_attributes(&self) -> Retained<NSArray<NSAttributedStringKey>> { NSArray::new() }

        #[method(firstRectForCharacterRange:actualRange:)]
        fn first_rect(&self, range: NSRange, actual: *mut NSRange) -> NSRect {
            if !actual.is_null() { unsafe { *actual = range; } }
            self.ivars().cursor.get()
        }

        #[method(characterIndexForPoint:)]
        fn character_index(&self, _point: NSPoint) -> usize { NOT_FOUND }
    }
);

fn plain_text(object: &AnyObject) -> String {
    unsafe {
        let attributed: bool = msg_send![object, isKindOfClass: NSAttributedString::class()];
        let string: Retained<NSString> = if attributed {
            msg_send_id![object, string]
        } else {
            // NSTextInputClient permits NSString or NSAttributedString.
            Retained::retain(object as *const AnyObject as *mut NSString).unwrap()
        };
        string.to_string()
    }
}

impl TextInputView {
    unsafe fn forward(&self, event: &NSEvent, down: bool) {
        if let Some(parent) = self.superview() {
            if down {
                parent.keyDown(event);
            } else {
                parent.keyUp(event);
            }
        }
    }
}

pub(super) struct Ime {
    view: Retained<TextInputView>,
    focused: Option<egui::Id>,
}

impl Ime {
    pub fn new() -> Self {
        unsafe {
            // EguiMq is created by miniquad on AppKit's main thread.
            let marker = objc2_foundation::MainThreadMarker::new()
                .expect("egui must run on the main thread");
            let allocated = marker.alloc::<TextInputView>().set_ivars(State::default());
            let view: Retained<TextInputView> = msg_send_id![super(allocated), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))];
            let parent = &*(miniquad::window::apple_view() as *const NSView);
            parent.addSubview(&view);
            Self {
                view,
                focused: None,
            }
        }
    }

    pub fn drain(&self, events: &mut Vec<Event>) {
        events.extend(self.view.ivars().events.borrow_mut().drain(..));
    }

    pub fn update(
        &mut self,
        output: Option<egui::output::IMEOutput>,
        focused: Option<egui::Id>,
        scale: f32,
    ) {
        unsafe {
            let Some(parent) = self.view.superview() else {
                return;
            };
            let Some(window) = parent.window() else {
                return;
            };
            if self.focused != focused {
                if let Some(context) = self.view.inputContext() {
                    context.discardMarkedText();
                }
                self.view.ivars().marked.borrow_mut().clear();
                self.view.ivars().composing.set(false);
                self.view.ivars().events.borrow_mut().clear();
            }
            if let Some(output) = output.filter(|_| focused.is_some()) {
                // egui points -> native view points -> screen coordinates (AppKit y-up).
                let bounds = parent.bounds();
                let rect = output.cursor_rect;
                let native = NSRect::new(
                    NSPoint::new(
                        (rect.left() * scale) as f64,
                        bounds.size.height - (rect.bottom() * scale) as f64,
                    ),
                    NSSize::new(
                        (rect.width().max(1.0) * scale) as f64,
                        (rect.height() * scale) as f64,
                    ),
                );
                self.view
                    .ivars()
                    .cursor
                    .set(window.convertRectToScreen(parent.convertRect_toView(native, None)));
                window.makeFirstResponder(Some(&self.view));
                if let Some(context) = self.view.inputContext() {
                    context.invalidateCharacterCoordinates();
                }
                self.focused = focused;
            } else if self.focused.take().is_some() {
                if let Some(context) = self.view.inputContext() {
                    context.discardMarkedText();
                }
                self.view.ivars().marked.borrow_mut().clear();
                self.view.ivars().composing.set(false);
                let ours = window.firstResponder().is_some_and(|current| {
                    std::ptr::eq(
                        &*current as *const _,
                        &*self.view as *const TextInputView as *const _,
                    )
                });
                if ours {
                    window.makeFirstResponder(Some(&parent));
                }
                self.view.ivars().events.borrow_mut().clear();
            }
        }
    }
}

impl Drop for Ime {
    fn drop(&mut self) {
        self.update(None, None, 1.0);
        unsafe {
            self.view.removeFromSuperview();
        }
    }
}
