//! Protobuf-based RPC client for ZMK Studio.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use futures::channel::oneshot;
use js_sys::{Function, Promise, Reflect, Uint8Array};
use prost::Message;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use super::framing::{encode_frame, FrameDecoder};
use super::transport::ZmkTransport;

use zmk_studio_rust_proto::zmk::studio;
use zmk_studio_rust_proto::zmk_core_proto::zmk::core;
use zmk_studio_rust_proto::zmk_behaviors_proto::zmk::behaviors;
use zmk_studio_rust_proto::zmk_keymap_proto::zmk::keymap;

pub type RequestResponseOneOf = studio::request_response::Subsystem;

#[derive(Debug, Clone)]
pub enum NotificationOneOf {
    CoreLockStateChanged(i32),
    KeymapUnsavedChanges(bool),
}

type PendingMap = Rc<RefCell<HashMap<u32, oneshot::Sender<RequestResponseOneOf>>>>;

pub struct RpcClient {
    transport: Rc<dyn ZmkTransport>,
    pending: PendingMap,
    next_id: Rc<RefCell<u32>>,
    notification_cb: Rc<RefCell<Option<Box<dyn Fn(NotificationOneOf)>>>>,
    raw_response_cb: Rc<RefCell<Option<Box<dyn Fn(Vec<u8>)>>>>,
    _reader_cancel: Rc<RefCell<Option<JsValue>>>,
}

impl RpcClient {
    pub fn new(transport: Rc<dyn ZmkTransport>) -> Rc<Self> {
        let pending: PendingMap = Rc::new(RefCell::new(HashMap::new()));
        let notification_cb: Rc<RefCell<Option<Box<dyn Fn(NotificationOneOf)>>>> =
            Rc::new(RefCell::new(None));
        let raw_response_cb: Rc<RefCell<Option<Box<dyn Fn(Vec<u8>)>>>> =
            Rc::new(RefCell::new(None));
        let reader_cancel = Rc::new(RefCell::new(None::<JsValue>));

        let client = Rc::new(Self {
            transport: transport.clone(),
            pending: pending.clone(),
            next_id: Rc::new(RefCell::new(1)),
            notification_cb: notification_cb.clone(),
            raw_response_cb: raw_response_cb.clone(),
            _reader_cancel: reader_cancel.clone(),
        });

        {
            let pending = pending.clone();
            let notification_cb = notification_cb.clone();
            let raw_response_cb = raw_response_cb.clone();
            let transport = transport.clone();
            spawn_local(async move {
                if let Err(e) = read_loop(transport, pending, notification_cb, raw_response_cb).await {
                    log::error!("RPC read loop ended: {}", e);
                }
            });
        }

        client
    }

    pub fn set_notification_callback<F: Fn(NotificationOneOf) + 'static>(&self, cb: F) {
        *self.notification_cb.borrow_mut() = Some(Box::new(cb));
    }

    pub fn set_raw_response_callback<F: Fn(Vec<u8>) + 'static>(&self, cb: F) {
        *self.raw_response_cb.borrow_mut() = Some(Box::new(cb));
    }

    /// Close the transport connection.
    pub fn close(&self) -> Result<js_sys::Promise, String> {
        self.transport.close()
    }

    async fn call_raw(&self, subsystem: studio::request::Subsystem) -> Result<RequestResponseOneOf, String> {
        let id = {
            let mut next = self.next_id.borrow_mut();
            let id = *next;
            *next = next.wrapping_add(1);
            id
        };

        let req = studio::Request {
            request_id: id,
            subsystem: Some(subsystem),
        };

        let mut envelope = Vec::new();
        req.encode(&mut envelope).map_err(|e| format!("Encode error: {}", e))?;

        let (tx, rx) = oneshot::channel();
        self.pending.borrow_mut().insert(id, tx);

        let frame = encode_frame(&envelope);
        JsFuture::from(
            self.transport
                .write(&frame)
                .map_err(|e| format!("Write failed: {e}"))?,
        )
        .await
        .map_err(|e| format!("Write failed: {e:?}"))?;

        let resp = rx.await.map_err(|_| "Response channel closed".to_string())?;
        
        if let studio::request_response::Subsystem::Meta(_meta_resp) = &resp {
            return Err("Unlock required".to_string());
        }
        
        Ok(resp)
    }

    pub async fn get_device_info(&self) -> Result<core::GetDeviceInfoResponse, String> {
        let req = studio::request::Subsystem::Core(core::Request {
            request_type: Some(core::request::RequestType::GetDeviceInfo(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Core(core::Response { response_type: Some(core::response::ResponseType::GetDeviceInfo(data)) }) => Ok(data),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn get_lock_state(&self) -> Result<i32, String> {
        let req = studio::request::Subsystem::Core(core::Request {
            request_type: Some(core::request::RequestType::GetLockState(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Core(core::Response { response_type: Some(core::response::ResponseType::GetLockState(state)) }) => Ok(state),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn list_all_behaviors(&self) -> Result<behaviors::ListAllBehaviorsResponse, String> {
        let req = studio::request::Subsystem::Behaviors(behaviors::Request {
            request_type: Some(behaviors::request::RequestType::ListAllBehaviors(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Behaviors(behaviors::Response { response_type: Some(behaviors::response::ResponseType::ListAllBehaviors(data)) }) => Ok(data),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn get_behavior_details(&self, behavior_id: u32) -> Result<behaviors::GetBehaviorDetailsResponse, String> {
        let req = studio::request::Subsystem::Behaviors(behaviors::Request {
            request_type: Some(behaviors::request::RequestType::GetBehaviorDetails(behaviors::GetBehaviorDetailsRequest {
                behavior_id,
            })),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Behaviors(behaviors::Response { response_type: Some(behaviors::response::ResponseType::GetBehaviorDetails(data)) }) => Ok(data),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn get_keymap(&self) -> Result<keymap::Keymap, String> {
        let req = studio::request::Subsystem::Keymap(keymap::Request {
            request_type: Some(keymap::request::RequestType::GetKeymap(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Keymap(keymap::Response { response_type: Some(keymap::response::ResponseType::GetKeymap(data)) }) => Ok(data),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn get_physical_layouts(&self) -> Result<keymap::PhysicalLayouts, String> {
        let req = studio::request::Subsystem::Keymap(keymap::Request {
            request_type: Some(keymap::request::RequestType::GetPhysicalLayouts(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Keymap(keymap::Response { response_type: Some(keymap::response::ResponseType::GetPhysicalLayouts(data)) }) => Ok(data),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn set_layer_binding(
        &self,
        layer_id: u32,
        key_position: i32,
        behavior_id: i32,
        param1: u32,
        param2: u32,
    ) -> Result<(), String> {
        let req = studio::request::Subsystem::Keymap(keymap::Request {
            request_type: Some(keymap::request::RequestType::SetLayerBinding(keymap::SetLayerBindingRequest {
                layer_id,
                key_position,
                binding: Some(keymap::BehaviorBinding {
                    behavior_id,
                    param1,
                    param2,
                }),
            })),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Keymap(keymap::Response { response_type: Some(keymap::response::ResponseType::SetLayerBinding(resp)) }) => {
                match resp {
                    0 => Ok(()),
                    1 => Err("Invalid location".to_string()),
                    2 => Err("Invalid behavior".to_string()),
                    3 => Err("Invalid parameters".to_string()),
                    other => Err(format!("Unknown error code: {}", other)),
                }
            },
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn save_changes(&self) -> Result<(), String> {
        let req = studio::request::Subsystem::Keymap(keymap::Request {
            request_type: Some(keymap::request::RequestType::SaveChanges(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Keymap(keymap::Response { response_type: Some(keymap::response::ResponseType::SaveChanges(resp)) }) => {
                match resp.result {
                    Some(keymap::save_changes_response::Result::Ok(_)) => Ok(()),
                    Some(keymap::save_changes_response::Result::Err(raw)) => Err(format!("Save changes error code: {:?}", raw)),
                    None => Err("Missing response result".to_string()),
                }
            },
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }

    pub async fn discard_changes(&self) -> Result<bool, String> {
        let req = studio::request::Subsystem::Keymap(keymap::Request {
            request_type: Some(keymap::request::RequestType::DiscardChanges(true)),
        });
        match self.call_raw(req).await? {
            RequestResponseOneOf::Keymap(keymap::Response { response_type: Some(keymap::response::ResponseType::DiscardChanges(resp)) }) => Ok(resp),
            other => Err(format!("Unexpected response type: {:?}", other)),
        }
    }
}

async fn read_loop(
    transport: Rc<dyn ZmkTransport>,
    pending: PendingMap,
    notification_cb: Rc<RefCell<Option<Box<dyn Fn(NotificationOneOf)>>>>,
    raw_response_cb: Rc<RefCell<Option<Box<dyn Fn(Vec<u8>)>>>>,
) -> Result<(), String> {
    let (reader_js, _cancel) = transport.setup_reader()?;
    let mut decoder = FrameDecoder::new();

    let is_ble = reader_js.is_object()
        && Reflect::get(&reader_js, &"readValue".into())
            .map(|v| v.is_function())
            .unwrap_or(false);

    if is_ble {
        let pending_clone = pending.clone();
        let notification_cb_clone = notification_cb.clone();
        let raw_response_cb_clone = raw_response_cb.clone();
        let decoder = Rc::new(RefCell::new(decoder));

        let callback = Closure::wrap(Box::new(move |event: JsValue| {
            let value = Reflect::get(&event, &"target".into())
                .and_then(|t| Reflect::get(&t, &"value".into()))
                .unwrap_or(JsValue::UNDEFINED);
            if value.is_undefined() {
                return;
            }
            let buffer = Reflect::get(&value, &"buffer".into()).unwrap_or(JsValue::UNDEFINED);
            let array = Uint8Array::new(&buffer);
            let mut data = vec![0u8; array.length() as usize];
            array.copy_to(&mut data);

            let frames = decoder.borrow_mut().feed(&data);
            for frame in frames {
                dispatch_response(&frame, &pending_clone, &notification_cb_clone, &raw_response_cb_clone);
            }
        }) as Box<dyn FnMut(JsValue)>);

        let add_fn: Function = Reflect::get(&reader_js, &"addEventListener".into())
            .map_err(|e| format!("{e:?}"))?
            .unchecked_into();
        add_fn
            .call2(
                &reader_js,
                &"characteristicvaluechanged".into(),
                callback.as_ref(),
            )
            .map_err(|e| format!("{e:?}"))?;

        callback.forget();
        Ok(())
    } else {
        loop {
            let read_fn: Function = Reflect::get(&reader_js, &"read".into())
                .map_err(|e| format!("{e:?}"))?
                .unchecked_into();
            let promise: Promise = read_fn
                .call0(&reader_js)
                .map_err(|e| format!("{e:?}"))?
                .unchecked_into();
            let result = JsFuture::from(promise)
                .await
                .map_err(|e| format!("Read failed: {e:?}"))?;

            let done = Reflect::get(&result, &"done".into())
                .unwrap_or(JsValue::TRUE)
                .as_bool()
                .unwrap_or(true);
            if done {
                break;
            }

            let value = Reflect::get(&result, &"value".into()).unwrap_or(JsValue::UNDEFINED);
            if value.is_undefined() {
                continue;
            }
            let array: Uint8Array = value.unchecked_into();
            let mut data = vec![0u8; array.length() as usize];
            array.copy_to(&mut data);

            let frames = decoder.feed(&data);
            for frame in frames {
                dispatch_response(&frame, &pending, &notification_cb, &raw_response_cb);
            }
        }
        Ok(())
    }
}

fn dispatch_response(
    frame: &[u8],
    pending: &PendingMap,
    notification_cb: &Rc<RefCell<Option<Box<dyn Fn(NotificationOneOf)>>>>,
    raw_response_cb: &Rc<RefCell<Option<Box<dyn Fn(Vec<u8>)>>>>,
) {
    if let Some(ref cb) = *raw_response_cb.borrow() {
        cb(frame.to_vec());
    }
    if let Ok(resp) = studio::Response::decode(frame) {
        if let Some(r#type) = resp.r#type {
            match r#type {
                studio::response::Type::RequestResponse(req_resp) => {
                    if let Some(subsystem) = req_resp.subsystem {
                        if let Some(tx) = pending.borrow_mut().remove(&req_resp.request_id) {
                            let _res: Result<(), RequestResponseOneOf> = tx.send(subsystem);
                        }
                    }
                }
                studio::response::Type::Notification(notif) => {
                    if let Some(subsystem) = notif.subsystem {
                        match subsystem {
                            studio::notification::Subsystem::Core(core::Notification { notification_type: Some(core::notification::NotificationType::LockStateChanged(state)) }) => {
                                if let Some(ref cb) = *notification_cb.borrow() {
                                    cb(NotificationOneOf::CoreLockStateChanged(state));
                                }
                            }
                            studio::notification::Subsystem::Keymap(keymap::Notification { notification_type: Some(keymap::notification::NotificationType::UnsavedChangesStatusChanged(changed)) }) => {
                                if let Some(ref cb) = *notification_cb.borrow() {
                                    cb(NotificationOneOf::KeymapUnsavedChanges(changed));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
