//! The page API borrows unpresented egui passes from eframe's runner. No second inbox.

use lightcraft_ui_egui::{
    LightcraftApp,
    control::{self, ControlRequest, Outcome},
};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};

use super::{COMMAND, PICKER_COMMAND, READY_COMMAND};
const PAGE_FUNCTION: &str = "__lightcraftControl";
const PUMP_EVENT: &str = "lightcraft-control-pass";

thread_local! {
    static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
    static PUMP: RefCell<Option<Pump>> = const { RefCell::new(None) };
    static CONTEXT: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
    static READY: RefCell<BTreeMap<u64,Prepared>>=const {RefCell::new(BTreeMap::new())};
    static ACTIVE: Cell<usize>=const {Cell::new(0)};
    static NEXT: Cell<u64>=const {Cell::new(0)};
}

struct Prepared {
    generation: u64,
    ticket: Option<u64>,
    method: String,
    params: Value,
    reply: js_sys::Function,
}

pub fn runner(runner: &eframe::WebRunner) {
    RUNNER.with(|slot| *slot.borrow_mut() = Some(runner.clone()));
}

#[derive(Clone)]
struct Pump {
    target: web_sys::EventTarget,
    ran: Rc<Cell<bool>>,
    last: Rc<Cell<f64>>,
}

impl Pump {
    fn get() -> Option<Self> {
        if let Some(pump) = PUMP.with(|slot| slot.borrow().clone()) {
            return Some(pump);
        }
        let runner = RUNNER.with(|slot| slot.borrow().clone())?;
        let pump = Self { target: web_sys::EventTarget::new().ok()?, ran: Rc::default(), last: Rc::default() };
        let ran = pump.ran.clone();
        runner
            .add_event_listener::<web_sys::Event>(&pump.target, PUMP_EVENT, move |_, app_runner| {
                app_runner.logic();
                ran.set(true);
            })
            .ok()?;
        PUMP.with(|slot| *slot.borrow_mut() = Some(pump.clone()));
        Some(pump)
    }

    fn pass(&self) -> bool {
        self.ran.set(false);
        let Ok(event) = web_sys::Event::new(PUMP_EVENT) else {
            return false;
        };
        let _ = self.target.dispatch_event(&event);
        if self.ran.get() {
            self.last.set(js_sys::Date::now());
        }
        self.ran.get()
    }
}

pub fn attach(
    app: &mut LightcraftApp,
    ctx: &egui::Context,
    originals: crate::store::Originals,
    backend: Option<crate::backend::Backend>,
    files: crate::files::Files,
) {
    app.c0.chrome = lightcraft_ui_egui::c0_host::Chrome::for_page(true);
    super::files::attach(app, ctx);
    super::io::attach(app, originals, backend, files);
    CONTEXT.with(|slot| *slot.borrow_mut() = Some(ctx.clone()));
    let Some(window) = web_sys::window() else {
        return;
    };
    let ctx = ctx.clone();
    let call = Closure::<dyn FnMut(JsValue) -> js_sys::Promise>::new(move |text: JsValue| {
        if text.is_string() && text.unchecked_ref::<js_sys::JsString>().length() as usize > lightcraft_ui_egui::c0_host::MAX_REQUEST_BYTES {
            return resolved(json!({"ok":false,"error":"request exceeds the host request limit"}));
        }
        let request = text.as_string().ok_or_else(|| "request must be JSON text".to_owned()).and_then(|s| super::parse(&s));
        let (method, params) = match request {
            Ok(r) => r,
            Err(e) => return resolved(json!({"ok":false,"error":e})),
        };
        // Wrap the existing wasm command export, which feeds WebApp's original INBOX.
        let promise = crate::web::command(COMMAND.into(), json!({"method":method,"params":params}).to_string());
        let pump = Pump::get();
        if let Some(pump) = &pump {
            pump.pass();
            for _ in 0..super::inline_passes(&method) {
                if !pump.pass() {
                    break;
                }
            }
        } else {
            ctx.request_repaint();
        }
        if !super::reads_only(&method) {
            ctx.request_repaint_after(std::time::Duration::from_millis(1));
        }
        promise
    });
    if let Err(e) = js_sys::Reflect::set(&window, &PAGE_FUNCTION.into(), call.as_ref().unchecked_ref()) {
        log::error!("page-control: cannot install {PAGE_FUNCTION}: {e:?}");
        return;
    }
    call.forget();
}

pub fn run(app: &mut LightcraftApp, id: &str, text: &str, resolve: &js_sys::Function) -> bool {
    if id == READY_COMMAND {
        let token = serde_json::from_str::<u64>(text).ok();
        let request = token.and_then(|token| READY.with(|slot| slot.borrow_mut().remove(&token)));
        let result = if let Some(request) = request {
            release();
            let result = if request.ticket.is_some_and(|t| !super::files::ticket_valid(t)) {
                json!({"ok":true,"result":{"cancelled":true}})
            } else if request.generation != app.session.library_generation() {
                json!({"ok":false,"error":"document changed while original bytes were loading; retry the request"})
            } else {
                dispatch(app, request.method, request.params)
            };
            reply(&request.reply, &result);
            result
        } else {
            json!({"ok":false,"error":"no pending file request"})
        };
        reply(resolve, &result);
        return true;
    }
    if id != COMMAND && id != PICKER_COMMAND {
        return false;
    }
    let result = (|| {
        let (method, mut params) = match super::parse(text) {
            Ok(request) => request,
            Err(error) => return json!({"ok":false,"error":error}),
        };
        #[derive(serde::Deserialize)]
        struct Ticket {
            ticket: u64,
        }
        let ticket = if id == PICKER_COMMAND {
            let ticket = serde_json::from_str::<Ticket>(text).ok().map(|t| t.ticket);
            if ticket.is_none_or(|ticket| !super::files::ticket_valid(ticket)) {
                return json!({"ok":true,"result":{"cancelled":true}});
            }
            ticket
        } else {
            None
        };
        match super::io::prepare(app, &method, &mut params) {
            Err(error) => json!({"ok":false,"error":error}),
            Ok(None) => dispatch(app, method, params),
            Ok(Some(work)) => {
                let request = Prepared { generation: app.session.library_generation(), ticket, method, params, reply: resolve.clone() };
                match defer(work, request) {
                    Ok(()) => Value::Null,
                    Err(error) => json!({"ok":false,"error":error}),
                }
            }
        }
    })();
    if !result.is_null() {
        reply(resolve, &result);
    }
    true
}

fn dispatch(app: &mut LightcraftApp, method: String, params: Value) -> Value {
    CONTEXT.with(|slot| {
        let Some(ctx) = slot.borrow().clone() else {
            return json!({"ok":false,"error":"app is not ready"});
        };
        let (req, _) = ControlRequest::new(method, params);
        match control::handle(app, &ctx, &req) {
            Outcome::Done(value) => value,
            Outcome::Screenshot { .. } => json!({"ok":false,"error":"viewport screenshots are not available in the page"}),
            Outcome::Gesture { .. } => json!({"ok":false,"error":"synthetic input is not available in the page"}),
        }
    })
}

fn reply(resolve: &js_sys::Function, result: &Value) {
    if let Err(e) = resolve.call1(&JsValue::NULL, &result.to_string().into()) {
        log::error!("page-control: replying failed: {e:?}");
    }
}

fn release() {
    ACTIVE.with(|n| n.set(n.get().saturating_sub(1)));
}

fn defer(work: super::io::Work, request: Prepared) -> Result<(), String> {
    if ACTIVE.with(Cell::get) >= super::MAX_PENDING_IO {
        return Err("file I/O is busy; retry after a pending request finishes".into());
    }
    let token = NEXT.with(|n| n.get().checked_add(1).inspect(|next| n.set(*next))).ok_or("request counter exhausted; reload the page")?;
    ACTIVE.with(|n| n.set(n.get().saturating_add(1)));
    wasm_bindgen_futures::spawn_local(async move {
        let result = super::io::perform(work).await;
        if request.ticket.is_some_and(|ticket| !super::files::ticket_valid(ticket)) {
            release();
            reply(&request.reply, &json!({"ok":true,"result":{"cancelled":true}}));
            return;
        }
        if let Err(error) = result {
            release();
            log::error!("page-control original I/O: {error}");
            reply(&request.reply, &json!({"ok":false,"error":error}));
            return;
        }
        let Some(pump) = Pump::get() else {
            release();
            reply(&request.reply, &json!({"ok":false,"error":"app is no longer ready"}));
            return;
        };
        READY.with(|slot| slot.borrow_mut().insert(token, request));
        let _ = crate::web::command(READY_COMMAND.into(), token.to_string());
        if !pump.pass()
            && let Some(request) = READY.with(|slot| slot.borrow_mut().remove(&token))
        {
            release();
            reply(&request.reply, &json!({"ok":false,"error":"app could not process the file request"}));
        }
    });
    Ok(())
}

fn resolved(value: Value) -> js_sys::Promise {
    js_sys::Promise::resolve(&JsValue::from_str(&value.to_string()))
}
