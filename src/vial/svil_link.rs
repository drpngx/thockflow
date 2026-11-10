//! SVIL transport: the client-ID wrapper over WebHID.
//!
//! Every request takes the link's lock, so the matrix poller and UI writes never
//! interleave, and replies are matched by client ID and command echo, so other
//! apps talking to the same keyboard don't confuse us.

use std::cell::Cell;

use futures::lock::Mutex;
use js_sys::{Reflect, Uint8Array};
use vial_protocol::svil::{self, BootstrapReply, WrappedReply};
use wasm_bindgen::JsCast;

use super::webhid::{self, HidDevice};

const REPLY_TIMEOUT_MS: i32 = 1000;
const BOOTSTRAP_TIMEOUT_MS: i32 = 500;
const BOOTSTRAP_ATTEMPTS: usize = 5;

pub struct SvilLink {
    device: HidDevice,
    report_len: usize,
    client_id: Cell<u32>,
    expires_at: Cell<f64>,
    lock: Mutex<()>,
}

impl PartialEq for SvilLink {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn nonce() -> [u8; svil::NONCE_SIZE] {
    let mut out = [0u8; svil::NONCE_SIZE];
    let arr = Uint8Array::new_with_length(svil::NONCE_SIZE as u32);
    let filled = web_sys::window()
        .and_then(|w| Reflect::get(&w, &"crypto".into()).ok())
        .and_then(|c| {
            let f: js_sys::Function = Reflect::get(&c, &"getRandomValues".into()).ok()?.dyn_into().ok()?;
            f.call1(&c, &arr).ok()
        })
        .is_some();
    if filled {
        arr.copy_to(&mut out);
    } else {
        for b in out.iter_mut() {
            *b = (js_sys::Math::random() * 256.0) as u8;
        }
    }
    out
}

impl SvilLink {
    /// Prompt for an SVIL keyboard and open it.
    pub async fn request() -> Result<SvilLink, String> {
        let device = webhid::request_device_with(svil::RAW_HID_USAGES).await?;
        let report_len = webhid::report_length(&device, svil::RAW_HID_USAGES)
            .filter(|&n| n >= svil::MIN_REPORT_LENGTH)
            .unwrap_or(svil::DEFAULT_REPORT_LENGTH);
        log::info!("SVIL device opened: {} ({report_len}-byte reports)", device.product_name());
        Ok(SvilLink { device, report_len, client_id: Cell::new(0), expires_at: Cell::new(0.0), lock: Mutex::new(()) })
    }

    pub fn device(&self) -> &HidDevice {
        &self.device
    }

    pub fn report_len(&self) -> usize {
        self.report_len
    }

    pub async fn close(&self) {
        let _ = webhid::close_device(&self.device).await;
    }

    async fn ensure_client(&self) -> Result<u32, String> {
        if self.client_id.get() != 0 && now_ms() < self.expires_at.get() {
            return Ok(self.client_id.get());
        }
        self.bootstrap().await
    }

    async fn bootstrap(&self) -> Result<u32, String> {
        for _round in 0..2 {
            let request = svil::build_bootstrap(self.report_len, &nonce());
            for _attempt in 0..BOOTSTRAP_ATTEMPTS {
                let req = request.clone();
                let reply = webhid::transact(
                    &self.device,
                    &request,
                    move |r| svil::parse_bootstrap_reply(r, &req) != BootstrapReply::Other,
                    BOOTSTRAP_TIMEOUT_MS,
                )
                .await;
                let Ok(reply) = reply else { continue };
                match svil::parse_bootstrap_reply(&reply, &request) {
                    BootstrapReply::Granted { client_id, ttl_secs } => {
                        self.client_id.set(client_id);
                        self.expires_at.set(now_ms() + svil::lease_millis(ttl_secs));
                        log::info!("SVIL client ID 0x{client_id:08x}, TTL {ttl_secs}s");
                        return Ok(client_id);
                    }
                    BootstrapReply::ZeroId => break, // new nonce, ask again
                    BootstrapReply::Unhandled | BootstrapReply::Echoed => {
                        return Err("This keyboard doesn't speak SVIL (it looks like Vial or VIA firmware). \
                                    Use the Vial tab, or update it to Svalboard QMK firmware."
                            .into());
                    }
                    BootstrapReply::Error(code) => return Err(format!("Keyboard refused a client ID (error {code}).")),
                    BootstrapReply::Other => {}
                }
            }
        }
        Err("The keyboard didn't respond. Close other apps that use it (Vial, VIA, Keybard), \
             unplug it and plug it back in, then connect again."
            .into())
    }

    async fn send_once(&self, protocol: u8, payload: &[u8], echo: usize) -> Result<Vec<u8>, String> {
        let client_id = self.ensure_client().await?;
        let report = svil::build_wrapped(self.report_len, client_id, protocol, payload);
        let expect: Vec<u8> = payload[..echo.min(payload.len())].to_vec();
        let expect_in = expect.clone();
        let accept = move |r: &[u8]| match svil::classify_reply(r, client_id) {
            WrappedReply::NotOurs => false,
            WrappedReply::ClientError(_) => true,
            WrappedReply::Payload { data, .. } => {
                data.starts_with(&expect_in) || (data.first() == Some(&svil::QMK_UNHANDLED) && !expect_in.is_empty())
            }
        };
        let reply = webhid::transact(&self.device, &report, accept, REPLY_TIMEOUT_MS).await?;
        match svil::classify_reply(&reply, client_id) {
            WrappedReply::ClientError(code) => Err(format!("client-error:{code}")),
            WrappedReply::Payload { data, .. } => {
                if data.first() == Some(&svil::QMK_UNHANDLED) && expect.first() != Some(&svil::QMK_UNHANDLED) {
                    Err(format!("Keyboard doesn't support command 0x{:02x}", payload[0]))
                } else {
                    Ok(data.to_vec())
                }
            }
            WrappedReply::NotOurs => Err("Unexpected reply".into()),
        }
    }

    /// Send one wrapped request and return the reply payload (from the command
    /// echo on). `echo` is how many leading payload bytes the reply must repeat.
    pub async fn send(&self, protocol: u8, payload: &[u8], echo: usize) -> Result<Vec<u8>, String> {
        let _guard = self.lock.lock().await;
        match self.send_once(protocol, payload, echo).await {
            Err(e) if e == format!("client-error:{}", svil::CLIENT_ERR_INVALID_ID) => {
                log::warn!("Keyboard dropped our client ID; renewing and retrying");
                self.client_id.set(0);
                self.send_once(protocol, payload, echo).await.map_err(describe)
            }
            other => other.map_err(describe),
        }
    }

    /// A VIA command carried in the wrapper; the reply must echo the command byte.
    pub async fn send_via(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        self.send(svil::VIA_PREFIX, payload, 1).await
    }

    /// A SVIL command; the reply must echo the command byte.
    pub async fn svil(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        self.send(svil::SVIL_PREFIX, payload, 1).await
    }
}

fn describe(e: String) -> String {
    match e.strip_prefix("client-error:") {
        Some(code) => format!("Keyboard wrapper error {code}"),
        None => e,
    }
}
