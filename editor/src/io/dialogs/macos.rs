//! Cached, window-owned sheets. No external process and no nested modal loop.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSModalResponseOK, NSOpenPanel, NSSavePanel, NSView};
use objc2_foundation::{MainThreadMarker, NSArray, NSOperationQueue, NSString, NSURL};

use super::{Request, Selection};

#[derive(Clone)]
struct Panels {
    open: Retained<NSOpenPanel>,
    save: Retained<NSSavePanel>,
}

thread_local! {
    static PANELS: RefCell<Option<Panels>> = const { RefCell::new(None) };
}

fn panels(mtm: MainThreadMarker) -> Panels {
    if let Some(panels) = PANELS.with(|cache| cache.borrow().clone()) {
        return panels;
    }
    // AppKit may pump events while initializing its file-browser service.
    // Never hold a RefCell borrow across a native call.
    let panels = unsafe {
        Panels {
            open: NSOpenPanel::openPanel(mtm),
            save: NSSavePanel::savePanel(mtm),
        }
    };
    PANELS.with(|cache| *cache.borrow_mut() = Some(panels.clone()));
    panels
}

fn on_main(action: impl Fn() + Send + 'static) {
    let block = RcBlock::new(action);
    // The main queue owns the block and runs it outside macroquad's draw
    // callback. UI objects are created and accessed only inside that block.
    unsafe {
        NSOperationQueue::mainQueue().addOperationWithBlock(&block);
    }
}

pub(super) fn warm_up() {
    on_main(|| {
        if let Some(mtm) = MainThreadMarker::new() {
            let _ = panels(mtm);
        }
    });
}

pub(super) fn present(
    request: Request,
    directory: PathBuf,
    reply: Sender<Result<Selection, String>>,
) {
    on_main(move || {
        if let Err(error) = show_sheet(request, &directory, reply.clone()) {
            let _ = reply.send(Err(error));
        }
    });
}

// These selectors also support macOS versions before allowedContentTypes / activate.
#[allow(deprecated)]
fn show_sheet(
    request: Request,
    directory: &std::path::Path,
    reply: Sender<Result<Selection, String>>,
) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("File dialog must run on the main thread")?;
    // Use the actual render view's owner, not whichever window happens to be
    // key after switching apps or opening another panel.
    let view = macroquad::miniquad::window::apple_view().cast::<NSView>();
    let parent = unsafe { view.as_ref() }
        .and_then(|view| view.window())
        .ok_or("Editor window is unavailable")?;
    if unsafe { parent.attachedSheet().is_some() } {
        return Err("Another sheet is already open".into());
    }
    let panels = panels(mtm);
    let (panel, title, types): (Retained<NSSavePanel>, &str, &[&str]) = match request {
        Request::Open | Request::Background => {
            unsafe {
                panels.open.setCanChooseFiles(true);
                panels
                    .open
                    .setCanChooseDirectories(request == Request::Open);
                panels.open.setAllowsMultipleSelection(false);
            }
            let panel = Retained::into_super(panels.open);
            if request == Request::Open {
                (panel, "Open chart", &[])
            } else {
                (panel, "Background image", &["png", "jpg", "jpeg"])
            }
        }
        Request::Save { save_as, native } => {
            if save_as {
                (
                    panels.save,
                    "Save chart (.txt Simai / .json native)",
                    if native {
                        &["json", "txt"]
                    } else {
                        &["txt", "json"]
                    },
                )
            } else {
                (panels.save, "Export Simai", &["txt"])
            }
        }
    };
    let types: Vec<_> = types.iter().map(|kind| NSString::from_str(kind)).collect();
    let types = (!types.is_empty()).then(|| NSArray::from_id_slice(&types));
    unsafe {
        panel.setTitle(Some(&NSString::from_str(title)));
        panel.setAllowedFileTypes(types.as_deref());
        panel.setAllowsOtherFileTypes(false);
        panel.setCanCreateDirectories(true);
        panel.setHidesOnDeactivate(false);
        let url = NSURL::fileURLWithPath_isDirectory(
            &NSString::from_str(&directory.to_string_lossy()),
            true,
        );
        panel.setDirectoryURL(Some(&url));
        let name = match request {
            Request::Save {
                save_as: true,
                native: true,
            } => "chart.json",
            Request::Save { .. } => "maidata.txt",
            _ => "",
        };
        panel.setNameFieldStringValue(&NSString::from_str(name));
    }
    let selected_panel = panel.clone();
    let owner = parent.clone();
    let completion = RcBlock::new(move |response| {
        let result = if response == NSModalResponseOK {
            unsafe {
                selected_panel
                    .URL()
                    .and_then(|url| {
                        let path = PathBuf::from(url.path()?.to_string());
                        let directory = if url.hasDirectoryPath() {
                            Some(path.clone())
                        } else {
                            path.parent().map(std::path::Path::to_path_buf)
                        };
                        Some(Selection {
                            path: Some(path),
                            directory,
                        })
                    })
                    .ok_or_else(|| "File dialog returned no path".to_string())
            }
        } else {
            Ok(Selection {
                path: None,
                directory: None,
            })
        };
        selected_panel.orderOut(None);
        if unsafe { NSApplication::sharedApplication(mtm).isActive() } {
            owner.makeKeyAndOrderFront(None);
        }
        let _ = reply.send(result);
    });
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    parent.makeKeyAndOrderFront(None);
    unsafe {
        panel.beginSheetModalForWindow_completionHandler(&parent, &completion);
    }
    panel.makeKeyAndOrderFront(None);
    Ok(())
}
