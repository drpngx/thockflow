//! WebSerial and BLE transport bindings for ZMK Studio.

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

// ZMK Studio BLE service / characteristic UUIDs
const ZMK_BLE_SERVICE_UUID: &str = "00000000-0196-6107-c967-c5cfb1c2482a";
const ZMK_BLE_CHAR_UUID: &str = "00000001-0196-6107-c967-c5cfb1c2482a";

// ---------------------------------------------------------------------------
// WebSerial extern types
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type SerialPort;

    #[wasm_bindgen(method)]
    fn open(this: &SerialPort, options: &JsValue) -> Promise;

    #[wasm_bindgen(method)]
    fn close(this: &SerialPort) -> Promise;

    #[wasm_bindgen(method, getter)]
    fn readable(this: &SerialPort) -> JsValue;

    #[wasm_bindgen(method, getter)]
    fn writable(this: &SerialPort) -> JsValue;
}

// ---------------------------------------------------------------------------
// BLE extern types
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type BluetoothDevice;

    #[wasm_bindgen(method, getter)]
    fn gatt(this: &BluetoothDevice) -> JsValue;

    #[wasm_bindgen(method, getter)]
    fn name(this: &BluetoothDevice) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Transport trait
// ---------------------------------------------------------------------------

/// Unified transport interface for ZMK Studio communication.
pub trait ZmkTransport {
    fn write(&self, data: &[u8]) -> Result<js_sys::Promise, String>;
    fn setup_reader(&self) -> Result<(JsValue, JsValue), String>; // (reader, cancel_fn)
    fn close(&self) -> Result<js_sys::Promise, String>;
    fn name(&self) -> String;
}

// ---------------------------------------------------------------------------
// WebSerial transport
// ---------------------------------------------------------------------------

pub struct SerialTransport {
    port: SerialPort,
}

impl SerialTransport {
    pub fn port(&self) -> &SerialPort {
        &self.port
    }
}

impl ZmkTransport for SerialTransport {
    fn write(&self, data: &[u8]) -> Result<Promise, String> {
        let writable = self.port.writable();
        if writable.is_undefined() || writable.is_null() {
            return Err("Port not writable".into());
        }
        let writer: Function = Reflect::get(&writable, &"getWriter".into())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        let w = writer
            .call0(&writable)
            .map_err(|e| format!("{e:?}"))?;
        let write_fn: Function = Reflect::get(&w, &"write".into())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        let arr = Uint8Array::from(data);
        let promise: Promise = write_fn
            .call1(&w, &arr)
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        // Release the writer lock after writing
        let release_fn: Function = Reflect::get(&w, &"releaseLock".into())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        let _ = release_fn.call0(&w);
        Ok(promise)
    }

    fn setup_reader(&self) -> Result<(JsValue, JsValue), String> {
        let readable = self.port.readable();
        if readable.is_undefined() || readable.is_null() {
            return Err("Port not readable".into());
        }
        let get_reader: Function = Reflect::get(&readable, &"getReader".into())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        let reader = get_reader
            .call0(&readable)
            .map_err(|e| format!("{e:?}"))?;
        Ok((reader, JsValue::UNDEFINED))
    }

    fn close(&self) -> Result<Promise, String> {
        Ok(self.port.close())
    }

    fn name(&self) -> String {
        "Serial".into()
    }
}

// ---------------------------------------------------------------------------
// BLE transport
// ---------------------------------------------------------------------------

pub struct BleTransport {
    device: BluetoothDevice,
    characteristic: JsValue,
}

impl BleTransport {
    pub fn device(&self) -> &BluetoothDevice {
        &self.device
    }
}

impl ZmkTransport for BleTransport {
    fn write(&self, data: &[u8]) -> Result<Promise, String> {
        let write_fn: Function =
            Reflect::get(&self.characteristic, &"writeValueWithoutResponse".into())
                .map_err(|e| format!("{e:?}"))?
                .unchecked_into();
        let arr = Uint8Array::from(data);
        let promise: Promise = write_fn
            .call1(&self.characteristic, &arr.buffer())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        Ok(promise)
    }

    fn setup_reader(&self) -> Result<(JsValue, JsValue), String> {
        // BLE uses notifications — the reader is the characteristic itself.
        // The caller should subscribe to 'characteristicvaluechanged' events.
        Ok((self.characteristic.clone(), JsValue::UNDEFINED))
    }

    fn close(&self) -> Result<Promise, String> {
        let gatt = self.device.gatt();
        if gatt.is_undefined() || gatt.is_null() {
            return Err("No GATT server".into());
        }
        let disconnect_fn: Function = Reflect::get(&gatt, &"disconnect".into())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        let _ = disconnect_fn.call0(&gatt);
        // disconnect is synchronous, return resolved promise
        Ok(Promise::resolve(&JsValue::UNDEFINED))
    }

    fn name(&self) -> String {
        self.device
            .name()
            .unwrap_or_else(|| "BLE Device".into())
    }
}

// ---------------------------------------------------------------------------
// Connection helpers
// ---------------------------------------------------------------------------

/// Open the browser's serial port picker and return a connected SerialTransport.
pub async fn connect_serial() -> Result<SerialTransport, String> {
    let window = web_sys::window().ok_or("No window")?;
    let navigator = Reflect::get(&window, &"navigator".into()).map_err(|_| "No navigator")?;
    let serial = Reflect::get(&navigator, &"serial".into()).map_err(|_| "WebSerial not supported")?;
    if serial.is_undefined() {
        return Err("WebSerial not supported. Use Chrome or Edge.".into());
    }

    // requestPort with no filters — user picks any serial device
    let request_fn: Function = Reflect::get(&serial, &"requestPort".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    let promise: Promise = request_fn
        .call1(&serial, &Object::new())
        .map_err(|e| format!("requestPort failed: {e:?}"))?
        .unchecked_into();
    let port_js = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Port selection failed: {e:?}"))?;
    let port: SerialPort = port_js.unchecked_into();

    // Open at 115200 baud (ZMK Studio default)
    let options = Object::new();
    Reflect::set(&options, &"baudRate".into(), &JsValue::from(115200))
        .map_err(|e| format!("{e:?}"))?;
    JsFuture::from(port.open(&options.into()))
        .await
        .map_err(|e| format!("Failed to open port: {e:?}"))?;

    Ok(SerialTransport { port })
}

/// Open the browser's Bluetooth picker and return a connected BleTransport.
pub async fn connect_ble() -> Result<BleTransport, String> {
    let window = web_sys::window().ok_or("No window")?;
    let navigator = Reflect::get(&window, &"navigator".into()).map_err(|_| "No navigator")?;
    let bluetooth =
        Reflect::get(&navigator, &"bluetooth".into()).map_err(|_| "Web Bluetooth not supported")?;
    if bluetooth.is_undefined() {
        return Err("Web Bluetooth not supported. Use Chrome or Edge.".into());
    }

    // Build request options with ZMK Studio service filter
    let filter = Object::new();
    let services = Array::new();
    services.push(&JsValue::from_str(ZMK_BLE_SERVICE_UUID));
    Reflect::set(&filter, &"services".into(), &services).map_err(|e| format!("{e:?}"))?;

    let filters = Array::new();
    filters.push(&filter);

    let options = Object::new();
    Reflect::set(&options, &"filters".into(), &filters).map_err(|e| format!("{e:?}"))?;
    let optional_services = Array::new();
    optional_services.push(&JsValue::from_str(ZMK_BLE_SERVICE_UUID));
    Reflect::set(&options, &"optionalServices".into(), &optional_services)
        .map_err(|e| format!("{e:?}"))?;

    let request_fn: Function = Reflect::get(&bluetooth, &"requestDevice".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    let promise: Promise = request_fn
        .call1(&bluetooth, &options)
        .map_err(|e| format!("requestDevice failed: {e:?}"))?
        .unchecked_into();
    let device_js = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Device selection failed: {e:?}"))?;
    let device: BluetoothDevice = device_js.unchecked_into();

    // Connect GATT
    let gatt = device.gatt();
    let connect_fn: Function = Reflect::get(&gatt, &"connect".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    let server = JsFuture::from(connect_fn.call0(&gatt).map_err(|e| format!("{e:?}"))?.unchecked_into::<Promise>())
        .await
        .map_err(|e| format!("GATT connect failed: {e:?}"))?;

    // Get service
    let get_service_fn: Function = Reflect::get(&server, &"getPrimaryService".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    let service = JsFuture::from(
        get_service_fn
            .call1(&server, &JsValue::from_str(ZMK_BLE_SERVICE_UUID))
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into::<Promise>(),
    )
    .await
    .map_err(|e| format!("Failed to get service: {e:?}"))?;

    // Get characteristic
    let get_char_fn: Function = Reflect::get(&service, &"getCharacteristic".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    let characteristic = JsFuture::from(
        get_char_fn
            .call1(&service, &JsValue::from_str(ZMK_BLE_CHAR_UUID))
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into::<Promise>(),
    )
    .await
    .map_err(|e| format!("Failed to get characteristic: {e:?}"))?;

    // Start notifications
    let start_fn: Function = Reflect::get(&characteristic, &"startNotifications".into())
        .map_err(|e| format!("{e:?}"))?
        .unchecked_into();
    JsFuture::from(
        start_fn
            .call0(&characteristic)
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into::<Promise>(),
    )
    .await
    .map_err(|e| format!("Failed to start notifications: {e:?}"))?;

    Ok(BleTransport {
        device,
        characteristic,
    })
}
