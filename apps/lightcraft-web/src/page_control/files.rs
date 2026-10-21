//! Cancellable browser picker. Late file reads cannot import after host cancellation.

use lightcraft_ui_egui::{
    LightcraftApp,
    c0_host::{MAX_FILE_BYTES, encode_bytes},
};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};

struct Picker {
    input: web_sys::HtmlInputElement,
    _change: Closure<dyn FnMut()>,
    _cancel: Closure<dyn FnMut()>,
}

impl Drop for Picker {
    fn drop(&mut self) {
        self.input.set_onchange(None);
        self.input.set_oncancel(None);
    }
}

type Active = Rc<RefCell<Option<Picker>>>;

type TicketState = (Rc<Cell<u64>>, Rc<Cell<bool>>);
thread_local! {static TICKET: RefCell<Option<TicketState>>=const {RefCell::new(None)};}

pub(super) fn ticket_valid(ticket: u64) -> bool {
    TICKET.with(|slot| slot.borrow().as_ref().is_some_and(|(generation, pending)| generation.get() == ticket && pending.get()))
}

fn report(error: impl std::fmt::Display) {
    let error = format!("Photo import: {error}");
    log::error!("{error}");
    crate::safety::notice(error);
}

pub fn attach(app: &mut LightcraftApp, ctx: &egui::Context) {
    let generation = Rc::new(Cell::new(0u64));
    let pending = Rc::new(Cell::new(false));
    TICKET.with(|slot| *slot.borrow_mut() = Some((generation.clone(), pending.clone())));
    let active: Active = Rc::default();
    let (g, p) = (generation.clone(), pending.clone());
    app.c0.picker_pending = Some(Box::new(move || p.get()));
    let p = pending.clone();
    let picker = active.clone();
    app.c0.cancel_picker = Some(Box::new(move || {
        g.set(g.get().wrapping_add(1));
        picker.borrow_mut().take();
        p.replace(false)
    }));
    let ctx = ctx.clone();
    app.services.pick_files = Some(Box::new(move || {
        if let Err(e) = open(generation.clone(), pending.clone(), active.clone(), ctx.clone()) {
            report(e);
        }
        Vec::new()
    }));
}

fn open(generation: Rc<Cell<u64>>, pending: Rc<Cell<bool>>, active: Active, ctx: egui::Context) -> Result<(), String> {
    active.borrow_mut().take();
    let doc = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let input = doc
        .create_element("input")
        .map_err(|e| format!("create picker: {e:?}"))?
        .dyn_into::<web_sys::HtmlInputElement>()
        .map_err(|_| "not an input")?;
    input.set_type("file");
    input.set_multiple(true);
    input.set_accept(crate::web::ACCEPT);
    let ticket = generation.get().wrapping_add(1);
    generation.set(ticket);
    pending.set(true);
    let inp = input.clone();
    let (g, p) = (generation.clone(), pending.clone());
    let picker = Rc::downgrade(&active);
    let change = Closure::<dyn FnMut()>::new(move || {
        if g.get() != ticket {
            return;
        }
        let files = inp.files();
        if let Some(active) = picker.upgrade() {
            active.borrow_mut().take();
        }
        let Some(files) = files else {
            p.set(false);
            return;
        };
        wasm_bindgen_futures::spawn_local(read(files, g.clone(), p.clone(), ticket, ctx.clone()));
    });
    input.set_onchange(Some(change.as_ref().unchecked_ref()));
    let picker = Rc::downgrade(&active);
    let cancel = Closure::<dyn FnMut()>::new(move || {
        if generation.get() == ticket {
            pending.set(false);
            if let Some(active) = picker.upgrade() {
                active.borrow_mut().take();
            }
        }
    });
    input.set_oncancel(Some(cancel.as_ref().unchecked_ref()));
    *active.borrow_mut() = Some(Picker { input: input.clone(), _change: change, _cancel: cancel });
    input.click();
    Ok(())
}

async fn read(files: web_sys::FileList, generation: Rc<Cell<u64>>, pending: Rc<Cell<bool>>, ticket: u64, ctx: egui::Context) {
    for i in 0..files.length() {
        if generation.get() != ticket {
            break;
        }
        let Some(file) = files.get(i) else {
            continue;
        };
        if file.size() > MAX_FILE_BYTES as f64 {
            report(format!("{} exceeds the file limit", file.name()));
            continue;
        }
        let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
        if generation.get() != ticket {
            break;
        }
        match buffer {
            Ok(buffer) => {
                let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                let request = json!({"method":"c0.openFile","params":{"name":file.name(),"bytes":encode_bytes(&bytes)},"ticket":ticket});
                let promise = crate::web::command(super::PICKER_COMMAND.into(), request.to_string());
                ctx.request_repaint();
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(reply) => {
                        let value = reply.as_string().and_then(|s| serde_json::from_str::<Value>(&s).ok());
                        match value {
                            Some(value) if value.get("ok") == Some(&Value::Bool(true)) => {}
                            Some(value) => report(value.get("error").and_then(Value::as_str).unwrap_or("file import failed")),
                            None => report("file import returned an invalid reply"),
                        }
                    }
                    Err(e) => report(format!("import failed: {e:?}")),
                }
            }
            Err(e) => report(format!("reading {}: {e:?}", file.name())),
        }
    }
    if generation.get() == ticket {
        pending.set(false);
    }
}
