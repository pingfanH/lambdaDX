//! Native callback -> adapter -> TextEdit regression; runs on AppKit's main thread.
#[cfg(target_os = "macos")]
fn main() {
    use miniquad as mq;
    use objc2::{msg_send, runtime::AnyObject, sel};
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};
    use objc2_foundation::{NSPoint, NSRange, NSString};

    unsafe fn key(client: &objc2_app_kit::NSResponder, text: &str, code: u16) {
        let text = NSString::from_str(text);
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown, NSPoint::new(0.0, 0.0), NSEventModifierFlags::empty(),
            0.0, 0, None, &text, &text, false, code,
        ).unwrap();
        client.keyDown(&event);
        let released = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyUp, NSPoint::new(0.0, 0.0), NSEventModifierFlags::empty(),
            0.0, 0, None, &text, &text, false, code,
        ).unwrap();
        client.keyUp(&released);
    }

    struct App {
        backend: Box<dyn mq::RenderingBackend>,
        egui: egui_miniquad::EguiMq,
        text: String,
        frame: usize,
        focus: bool,
    }
    impl mq::EventHandler for App {
        fn update(&mut self) {}
        fn key_down_event(&mut self, key: mq::KeyCode, modifiers: mq::KeyMods, _: bool) {
            self.egui.key_down_event(key, modifiers);
        }
        fn char_event(&mut self, character: char, _: mq::KeyMods, _: bool) {
            self.egui.char_event(character);
        }
        fn key_up_event(&mut self, key: mq::KeyCode, modifiers: mq::KeyMods) {
            self.egui.key_up_event(key, modifiers);
        }
        fn draw(&mut self) {
            self.egui.run(&mut *self.backend, |_, ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = ui.add(egui::TextEdit::multiline(&mut self.text));
                    if self.focus {
                        response.request_focus();
                    } else {
                        response.surrender_focus();
                    }
                });
            });
            self.egui.draw(&mut *self.backend);
            self.backend.commit_frame();
            unsafe {
                let view = &*(mq::window::apple_view() as *const NSView);
                let window = view.window().unwrap();
                if self.frame == 0 {
                    window.makeKeyAndOrderFront(None);
                }
                let client = window.firstResponder().unwrap();
                if self.frame == 1 {
                    let supported: bool = msg_send![&*client, respondsToSelector: sel!(setMarkedText:selectedRange:replacementRange:)];
                    assert!(
                        supported,
                        "Focused native responder does not support Chinese IME composition"
                    );
                    let marked = NSString::from_str("nihao");
                    let _: () = msg_send![&*client, setMarkedText: &*marked selectedRange: NSRange::new(5, 0) replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 2 {
                    assert_eq!(self.text, "nihao", "preedit must reach TextEdit");
                    let committed = NSString::from_str("你好世界");
                    let object: &AnyObject = &committed;
                    let _: () = msg_send![&*client, insertText: object replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 3 {
                    assert_eq!(
                        self.text, "你好世界",
                        "commit must replace preedit exactly once"
                    );
                    let marked = NSString::from_str("取消");
                    let _: () = msg_send![&*client, setMarkedText: &*marked selectedRange: NSRange::new(2, 0) replacementRange: NSRange::new(isize::MAX as usize, 0)];
                    key(&client, "\r", 36);
                }
                if self.frame == 4 {
                    assert_eq!(self.text, "你好世界取消");
                    let empty = NSString::from_str("");
                    let _: () = msg_send![&*client, setMarkedText: &*empty selectedRange: NSRange::new(0, 0) replacementRange: NSRange::new(isize::MAX as usize, 0)];
                    let _: () = msg_send![&*client, unmarkText];
                }
                if self.frame == 5 {
                    assert_eq!(self.text, "你好世界", "cancel must remove preedit");
                    let ascii = NSString::from_str("abc");
                    let _: () = msg_send![&*client, insertText: &*ascii replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 6 {
                    assert_eq!(
                        self.text, "你好世界abc",
                        "ordinary text must be inserted once"
                    );
                    key(&client, "x", 7);
                }
                if self.frame == 7 {
                    assert_eq!(
                        self.text, "你好世界abcx",
                        "native keyDown must insert ASCII exactly once"
                    );
                    key(&client, "\r", 36);
                }
                if self.frame == 8 {
                    assert_eq!(
                        self.text, "你好世界abcx\n",
                        "ordinary Return must still insert a newline"
                    );
                    let marked = NSString::from_str("shan");
                    let _: () = msg_send![&*client, setMarkedText: &*marked selectedRange: NSRange::new(4, 0) replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 9 {
                    assert_eq!(self.text, "你好世界abcx\nshan");
                    // AppKit can clear the final preedit without calling unmarkText.
                    let empty = NSString::from_str("");
                    let _: () = msg_send![&*client, setMarkedText: &*empty selectedRange: NSRange::new(0, 0) replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 10 {
                    assert_eq!(self.text, "你好世界abcx\n");
                    key(&client, "\u{7f}", 51);
                }
                if self.frame == 11 {
                    assert_eq!(
                        self.text, "你好世界abcx",
                        "Backspace must work after an empty preedit, without typing another character"
                    );
                    key(&client, "\u{7f}", 51);
                }
                if self.frame == 12 {
                    assert_eq!(self.text, "你好世界abc", "successive Backspace must keep working");
                    key(&client, "\u{f702}", 123); // Left
                    key(&client, "\u{f728}", 117); // Forward Delete
                }
                if self.frame == 13 {
                    assert_eq!(self.text, "你好世界ab", "navigation and forward Delete must work after cancellation");
                    let marked = NSString::from_str("shi");
                    let _: () = msg_send![&*client, setMarkedText: &*marked selectedRange: NSRange::new(3, 0) replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 14 {
                    assert_eq!(self.text, "你好世界abshi");
                    let committed = NSString::from_str("试");
                    let _: () = msg_send![&*client, insertText: &*committed replacementRange: NSRange::new(isize::MAX as usize, 0)];
                }
                if self.frame == 15 {
                    assert_eq!(self.text, "你好世界ab试", "Chinese composition must restart after cancellation");
                    key(&client, "\u{7f}", 51);
                }
                if self.frame == 16 {
                    assert_eq!(self.text, "你好世界ab", "Backspace must delete the committed Chinese character");
                    self.focus = false;
                }
                if self.frame == 18 {
                    assert_eq!(self.text, "你好世界ab");
                    let supported: bool = msg_send![&*client, respondsToSelector: sel!(setMarkedText:selectedRange:replacementRange:)];
                    assert!(
                        !supported,
                        "non-text focus must return native keyboard ownership"
                    );
                    println!(
                        "native Chinese IME preedit, commit, cancellation, deletion, ASCII and focus: PASS"
                    );
                    mq::window::order_quit();
                }
            }
            self.frame += 1;
        }
    }
    mq::start(
        mq::conf::Conf {
            window_title: "IME regression".into(),
            window_width: 400,
            window_height: 160,
            ..Default::default()
        },
        || {
            let mut backend = mq::window::new_rendering_backend();
            let egui = egui_miniquad::EguiMq::new(&mut *backend);
            Box::new(App {
                backend,
                egui,
                text: String::new(),
                frame: 0,
                focus: true,
            })
        },
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}
