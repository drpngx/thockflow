//! WebHID API bindings for communicating with Vial keyboards.

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

const RAW_HID_USAGE_PAGE: u16 = 0xFF60;
const RAW_HID_USAGE_ID: u16 = 0x61;
const MSG_LEN: usize = vial_protocol::MSG_LEN;

// ---------------------------------------------------------------------------
// JS extern types for WebHID (not yet in web-sys)
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type HidDevice;

    #[wasm_bindgen(method, getter, js_name = opened)]
    pub fn opened(this: &HidDevice) -> bool;

    #[wasm_bindgen(method, getter, js_name = productName)]
    pub fn product_name(this: &HidDevice) -> String;

    #[wasm_bindgen(method, getter, js_name = vendorId)]
    pub fn vendor_id(this: &HidDevice) -> u16;

    #[wasm_bindgen(method, getter, js_name = productId)]
    pub fn product_id(this: &HidDevice) -> u16;

    #[wasm_bindgen(method, js_name = open)]
    fn open_device(this: &HidDevice) -> Promise;

    #[wasm_bindgen(method, js_name = close)]
    fn close_device(this: &HidDevice) -> Promise;

    #[wasm_bindgen(method, js_name = sendReport)]
    fn send_report(this: &HidDevice, report_id: u8, data: &Uint8Array) -> Promise;
}

impl PartialEq for HidDevice {
    fn eq(&self, other: &Self) -> bool {
        self.vendor_id() == other.vendor_id() && self.product_id() == other.product_id()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Request a Vial-compatible HID device from the user via the browser picker.
pub async fn request_device() -> Result<HidDevice, String> {
    request_device_with(&[(RAW_HID_USAGE_PAGE, RAW_HID_USAGE_ID)]).await
}

/// Request a HID device matching any of the `(usage_page, usage)` filters.
pub async fn request_device_with(usages: &[(u16, u16)]) -> Result<HidDevice, String> {
    let window = web_sys::window().ok_or("No window")?;
    let navigator = Reflect::get(&window, &"navigator".into()).map_err(|_| "No navigator")?;
    let hid = Reflect::get(&navigator, &"hid".into()).map_err(|_| "WebHID not supported")?;
    if hid.is_undefined() {
        return Err("WebHID not supported. Use Chrome or Edge.".into());
    }

    // Build filters: { filters: [{ usagePage, usage }, ...] }
    let filters = Array::new();
    for &(page, usage) in usages {
        let filter = Object::new();
        Reflect::set(&filter, &"usagePage".into(), &JsValue::from(page)).map_err(|e| format!("{e:?}"))?;
        Reflect::set(&filter, &"usage".into(), &JsValue::from(usage)).map_err(|e| format!("{e:?}"))?;
        filters.push(&filter);
    }
    let options = Object::new();
    Reflect::set(&options, &"filters".into(), &filters).map_err(|e| format!("{e:?}"))?;

    let request_fn: Function = Reflect::get(&hid, &"requestDevice".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    let promise: Promise = request_fn
        .call1(&hid, &options.into())
        .map_err(|e| format!("requestDevice failed: {e:?}"))?
        .unchecked_into();
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Device selection failed: {e:?}"))?;

    let devices: Array = result.unchecked_into();
    if devices.length() == 0 {
        return Err("No device selected".into());
    }

    let device: HidDevice = devices.get(0).unchecked_into();
    if !device.opened() {
        JsFuture::from(device.open_device())
            .await
            .map_err(|e| format!("Failed to open device: {e:?}"))?;
    }

    Ok(device)
}

/// Send a 32-byte HID report and wait for the next `inputreport` response.
pub async fn send_message(
    device: &HidDevice,
    msg: &[u8; MSG_LEN],
) -> Result<[u8; MSG_LEN], String> {
    // Build a one-shot Promise that resolves on the next inputreport event.
    let device_js: &JsValue = device.as_ref();
    let device_for_listener = device_js.clone();

    let response_promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        let dev = device_for_listener.clone();
        let callback = Closure::once_into_js(move |event: JsValue| {
            let data = Reflect::get(&event, &"data".into()).unwrap_or(JsValue::UNDEFINED);
            let buffer = Reflect::get(&data, &"buffer".into()).unwrap_or(JsValue::UNDEFINED);
            let array = Uint8Array::new(&buffer);
            let mut resp = [0u8; MSG_LEN];
            let len = (array.length() as usize).min(MSG_LEN);
            array.slice(0, len as u32).copy_to(&mut resp[..len]);
            let out = Uint8Array::from(&resp[..]);
            let _ = resolve.call1(&JsValue::NULL, &out);
        });

        // addEventListener("inputreport", cb, { once: true })
        let opts = Object::new();
        let _ = Reflect::set(&opts, &"once".into(), &JsValue::TRUE);
        let add_fn: Function = Reflect::get(&dev, &"addEventListener".into())
            .unwrap()
            .unchecked_into();
        let _ = add_fn.call3(&dev, &"inputreport".into(), &callback, &opts.into());
    });

    // Send the report (report ID 0x00).
    let data = Uint8Array::from(&msg[..]);
    JsFuture::from(device.send_report(0x00, &data))
        .await
        .map_err(|e| format!("Failed to send report: {e:?}"))?;

    // Await the response.
    let result = JsFuture::from(response_promise)
        .await
        .map_err(|e| format!("Failed to receive response: {e:?}"))?;

    let array: Uint8Array = result.unchecked_into();
    let mut response = [0u8; MSG_LEN];
    array.copy_to(&mut response);
    Ok(response)
}

/// Close the HID device.
pub async fn close_device(device: &HidDevice) -> Result<(), String> {
    JsFuture::from(device.close_device())
        .await
        .map_err(|e| format!("Failed to close device: {e:?}"))?;
    Ok(())
}

/// Raw HID report length of a device: the size of the first output report of
/// its raw HID collection matching one of `usages`, from the report
/// descriptor. `None` when the descriptor gives no usable size.
pub fn report_length(device: &HidDevice, usages: &[(u16, u16)]) -> Option<usize> {
    let dev: &JsValue = device.as_ref();
    let get = |v: &JsValue, k: &str| Reflect::get(v, &k.into()).unwrap_or(JsValue::UNDEFINED);
    let num = |v: &JsValue, k: &str| get(v, k).as_f64().unwrap_or(0.0) as u64;
    let collections: Array = get(dev, "collections").dyn_into().ok()?;
    let raw = collections.iter().find(|c| {
        let (page, usage) = (num(c, "usagePage") as u16, num(c, "usage") as u16);
        usages.iter().any(|&(p, u)| p == page && u == usage)
    })?;
    let outputs: Array = get(&raw, "outputReports").dyn_into().ok()?;
    let report = outputs.get(0);
    let items: Array = get(&report, "items").dyn_into().ok()?;
    let bits: u64 = items.iter().map(|i| num(&i, "reportSize") * num(&i, "reportCount")).sum();
    let len = (bits / 8) as usize;
    (len > 0).then_some(len)
}

/// Resolve after `ms` milliseconds.
pub async fn sleep_ms(ms: i32) {
    let promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(promise).await;
}

/// Send `report` (report ID 0) and wait for the first input report that
/// `accept` returns true for, at most `timeout_ms`. The listener is installed
/// before sending, so a fast reply cannot be missed; reports that don't match
/// (other clients' traffic, stragglers) are skipped.
pub async fn transact(
    device: &HidDevice,
    report: &[u8],
    accept: impl FnMut(&[u8]) -> bool + 'static,
    timeout_ms: i32,
) -> Result<Vec<u8>, String> {
    use futures::channel::oneshot;
    use futures::future::{select, Either};
    use std::cell::RefCell;
    use std::rc::Rc;

    let (tx, rx) = oneshot::channel::<Vec<u8>>();
    let tx = Rc::new(RefCell::new(Some(tx)));
    let accept = RefCell::new(accept);
    let listener = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        let data = Reflect::get(&event, &"data".into()).unwrap_or(JsValue::UNDEFINED);
        let get = |k: &str| Reflect::get(&data, &k.into()).unwrap_or(JsValue::UNDEFINED);
        let buffer = get("buffer");
        if buffer.is_undefined() {
            return;
        }
        let offset = get("byteOffset").as_f64().unwrap_or(0.0) as u32;
        let len = get("byteLength").as_f64().unwrap_or(0.0) as u32;
        let bytes = Uint8Array::new_with_byte_offset_and_length(&buffer, offset, len).to_vec();
        if (accept.borrow_mut())(&bytes) {
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(bytes);
            }
        }
    });

    let dev: &JsValue = device.as_ref();
    let call = |name: &str| -> Result<(), String> {
        let f: Function = Reflect::get(dev, &name.into()).map_err(|e| format!("{e:?}"))?.unchecked_into();
        f.call2(dev, &"inputreport".into(), listener.as_ref()).map_err(|e| format!("{e:?}"))?;
        Ok(())
    };
    call("addEventListener")?;

    let data = Uint8Array::from(report);
    let sent = JsFuture::from(device.send_report(0x00, &data)).await;
    let result = match sent {
        Err(e) => Err(format!("Failed to send report: {e:?}")),
        Ok(_) => match select(rx, Box::pin(sleep_ms(timeout_ms))).await {
            Either::Left((Ok(bytes), _)) => Ok(bytes),
            Either::Left((Err(_), _)) => Err("Report listener dropped".to_string()),
            Either::Right(_) => Err("USB command timeout".to_string()),
        },
    };
    let _ = call("removeEventListener");
    drop(listener);
    result
}

