//! ZMK Studio tab — connect to ZMK keyboards via WebSerial/BLE and edit keymaps.

pub mod convert;
pub mod framing;
pub mod rpc;
pub mod transport;

use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::keymap::{KeymapData, KeymapRenderer};

use convert::{BehaviorCache, ProtoLayer};
use rpc::{NotificationOneOf, RpcClient};
use transport::ZmkTransport;

#[derive(Clone, PartialEq)]
enum TransportType {
    Serial,
    Bluetooth,
}

#[function_component]
pub fn ZmkStudioHome() -> Html {
    let connected = use_state(|| false);
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let device_name = use_state(String::new);
    let lock_state = use_state(|| 0u32); // 0 = locked, 1 = unlocked
    let has_unsaved_changes = use_state(|| false);
    let keymap_data = use_state(|| None::<KeymapData>);
    let rpc_client = use_state(|| None::<Rc<RpcClient>>);
    let behavior_cache = use_state(BehaviorCache::new);
    let proto_layers = use_state(Vec::<ProtoLayer>::new);
    let transport_type = use_state(|| TransportType::Serial);
    let dumped_bytes = use_state(Vec::<Vec<u8>>::new);

    let is_dump_init = use_memo((), |_| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(location) = window.location().search() {
                    return location.contains("dump_init=1");
                }
            }
        }
        false
    });
    let is_dump_init = *is_dump_init;

    // --- Connect handler ---
    let on_connect = {
        let connected = connected.clone();
        let loading = loading.clone();
        let error = error.clone();
        let device_name = device_name.clone();
        let lock_state = lock_state.clone();
        let keymap_data = keymap_data.clone();
        let rpc_client = rpc_client.clone();
        let behavior_cache = behavior_cache.clone();
        let proto_layers = proto_layers.clone();
        let has_unsaved_changes = has_unsaved_changes.clone();
        let transport_type = transport_type.clone();
        let dumped_bytes = dumped_bytes.clone();

        Callback::from(move |_: MouseEvent| {
            let connected = connected.clone();
            let loading = loading.clone();
            let error = error.clone();
            let device_name = device_name.clone();
            let lock_state_state = lock_state.clone();
            let keymap_data = keymap_data.clone();
            let rpc_client_state = rpc_client.clone();
            let behavior_cache = behavior_cache.clone();
            let proto_layers = proto_layers.clone();
            let has_unsaved_changes = has_unsaved_changes.clone();
            let dumped_bytes = dumped_bytes.clone();
            let is_ble = *transport_type == TransportType::Bluetooth;

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                let result = if is_ble {
                    transport::connect_ble().await.map(|t| Rc::new(t) as Rc<dyn ZmkTransport>)
                } else {
                    transport::connect_serial().await.map(|t| Rc::new(t) as Rc<dyn ZmkTransport>)
                };

                let transport = match result {
                    Ok(t) => t,
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                        return;
                    }
                };

                let client = RpcClient::new(transport);

                if is_dump_init {
                    let db = dumped_bytes.clone();
                    client.set_raw_response_callback(move |bytes| {
                        let mut current = (*db).clone();
                        current.push(bytes);
                        db.set(current);
                    });
                }

                // Set up notification callback
                {
                    let lock_state_cb = lock_state_state.clone();
                    let unsaved_cb = has_unsaved_changes.clone();
                    client.set_notification_callback(move |notif| match notif {
                        NotificationOneOf::CoreLockStateChanged(state) => {
                            lock_state_cb.set(state as u32);
                        }
                        NotificationOneOf::KeymapUnsavedChanges(changed) => {
                            unsaved_cb.set(changed);
                        }
                    });
                }

                // Get lock state
                let current_lock_state = match client.get_lock_state().await {
                    Ok(state) => {
                        lock_state_state.set(state as u32);
                        state as u32
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to get lock state: {}", e)));
                        loading.set(false);
                        return;
                    }
                };

                // Get device info
                match client.get_device_info().await {
                    Ok(info) => {
                        device_name.set(info.name.clone());
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to get device info: {}", e)));
                        loading.set(false);
                        return;
                    }
                }

                if current_lock_state == 0 {
                    // Device is locked. We can't load behaviors, layouts or keymap yet.
                    // The notification callback will handle re-loading once unlocked.
                    rpc_client_state.set(Some(client));
                    connected.set(true);
                    loading.set(false);
                    return;
                }

                // Get behaviors
                let mut cache = BehaviorCache::new();
                match client.list_all_behaviors().await {
                    Ok(data) => {
                        for bid in data.behaviors {
                            match client.get_behavior_details(bid).await {
                                Ok(detail) => {
                                    cache.register_behavior(&detail);
                                }
                                Err(e) => {
                                    log::warn!("Failed to get behavior details for {}: {}", bid, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to list behaviors: {}", e)));
                        loading.set(false);
                        return;
                    }
                }

                // Get physical layouts
                let physical_keys = match client.get_physical_layouts().await {
                    Ok(data) => {
                        let (_active, layouts) = convert::parse_physical_layouts(&data);
                        if let Some((_, keys)) = layouts.into_iter().next() {
                            keys
                        } else {
                            error.set(Some("No physical layouts found".into()));
                            loading.set(false);
                            return;
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to get physical layouts: {}", e)));
                        loading.set(false);
                        return;
                    }
                };

                // Get keymap
                match client.get_keymap().await {
                    Ok(data) => {
                        let (layers, _available, _max_name) =
                            convert::parse_keymap(&data, &cache);
                        let km = convert::to_keymap_data(physical_keys, &layers, &cache);
                        proto_layers.set(layers);
                        keymap_data.set(Some(km));
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to get keymap: {}", e)));
                        loading.set(false);
                        return;
                    }
                }

                behavior_cache.set(cache);
                rpc_client_state.set(Some(client));
                connected.set(true);
                loading.set(false);
            });
        })
    };

    // --- Disconnect handler ---
    let on_disconnect = {
        let connected = connected.clone();
        let keymap_data = keymap_data.clone();
        let rpc_client = rpc_client.clone();
        let device_name = device_name.clone();
        let error = error.clone();
        let loading = loading.clone();

        Callback::from(move |_: MouseEvent| {
            loading.set(true);
            
            // Close the transport connection first
            let client_opt = (*rpc_client).clone();
            let rpc_client_clone = rpc_client.clone();
            let connected_clone = connected.clone();
            let keymap_data_clone = keymap_data.clone();
            let device_name_clone = device_name.clone();
            let error_clone = error.clone();
            let loading_clone = loading.clone();
            
            if let Some(client) = client_opt {
                spawn_local(async move {
                    // Try to close the transport properly
                    if let Ok(close_promise) = client.close() {
                        let _ = wasm_bindgen_futures::JsFuture::from(close_promise).await;
                    }
                    
                    // Now clear all state
                    rpc_client_clone.set(None);
                    connected_clone.set(false);
                    keymap_data_clone.set(None);
                    device_name_clone.set(String::new());
                    error_clone.set(None);
                    loading_clone.set(false);
                });
            } else {
                rpc_client.set(None);
                connected.set(false);
                keymap_data.set(None);
                device_name.set(String::new());
                error.set(None);
                loading.set(false);
            }
        })
    };

    // --- Keymap update handler (diff and send RPCs) ---
    let on_update = {
        let keymap_data = keymap_data.clone();
        let rpc_client = rpc_client.clone();
        let behavior_cache = behavior_cache.clone();
        let proto_layers = proto_layers.clone();
        let error = error.clone();
        let has_unsaved_changes = has_unsaved_changes.clone();

        Callback::from(move |new_data: KeymapData| {
            let old_data = (*keymap_data).clone();
            keymap_data.set(Some(new_data.clone()));

            if let (Some(old), Some(client)) = (old_data, (*rpc_client).clone()) {
                let cache = (*behavior_cache).clone();
                let current_proto_layers = (*proto_layers).clone();
                let error = error.clone();
                let has_unsaved = has_unsaved_changes.clone();

                spawn_local(async move {
                    has_unsaved.set(true);
                    // Diff old vs new and send set_layer_binding for each change
                    for (layer_idx, (old_layer, new_layer)) in
                        old.layers.iter().zip(new_data.layers.iter()).enumerate()
                    {
                        let layer_id = current_proto_layers
                            .get(layer_idx)
                            .map(|pl| pl.id)
                            .unwrap_or(layer_idx as u32);

                        for (key_idx, (old_binding, new_binding)) in
                            old_layer.bindings.iter().zip(new_layer.bindings.iter()).enumerate()
                        {
                            if old_binding != new_binding {
                                if let Some((behavior_id, param1, param2)) = convert::string_to_binding(new_binding, &cache) {
                                    let behavior_id: i32 = behavior_id;
                                    log::info!("Updating binding: layer={}, key={}, behavior={}, p1={}, p2={}", layer_id, key_idx, behavior_id, param1, param2);
                                    if let Err(e) = client
                                        .set_layer_binding(
                                            layer_id,
                                            key_idx as i32,
                                            behavior_id,
                                            param1,
                                            param2,
                                        )
                                        .await
                                    {
                                        log::error!("Failed to set binding: {}", e);
                                        error.set(Some(format!(
                                            "Failed to update key: {}",
                                            e
                                        )));
                                    }
                                } else {
                                    log::error!("Failed to parse binding string: {}", new_binding);
                                    let behavior_name = new_binding.split_whitespace().next().unwrap_or("Unknown");
                                    error.set(Some(format!(
                                        "Unsupported behavior: '{}'. The keyboard firmware might not be compiled with support for this feature.",
                                        behavior_name
                                    )));
                                    // Revert the unsaved state because this change was dropped locally and never sent to device.
                                    has_unsaved.set(false);
                                }                            }
                        }
                    }
                });
            }
        })
    };

    // --- Save handler ---
    let on_save = {
        let rpc_client = rpc_client.clone();
        let error = error.clone();
        let has_unsaved_changes = has_unsaved_changes.clone();
        let keymap_data = keymap_data.clone();
        let behavior_cache = behavior_cache.clone();
        let proto_layers = proto_layers.clone();

        Callback::from(move |_: MouseEvent| {
            if let Some(client) = (*rpc_client).clone() {
                let error = error.clone();
                let has_unsaved = has_unsaved_changes.clone();
                let keymap_data = keymap_data.clone();
                let cache = (*behavior_cache).clone();
                let proto_layers = proto_layers.clone();
                
                spawn_local(async move {
                    log::info!("Checking keyboard connection before saving...");
                    match client.get_lock_state().await {
                        Ok(_) => {
                            log::info!("Connection check passed. Saving changes to flash...");
                            match client.save_changes().await {
                                Ok(_) => {
                                    log::info!("Save successful. Verifying changes on device...");
                                    has_unsaved.set(false);
                                    
                                    // Verify changes by fetching keymap again
                                    match client.get_keymap().await {
                                        Ok(data) => {
                                            if let Some(existing) = &*keymap_data {
                                                let (layers, _, _) = convert::parse_keymap(&data, &cache);
                                                let verified_km = convert::to_keymap_data(
                                                    existing.physical_layout.clone(),
                                                    &layers,
                                                    &cache,
                                                );
                                                
                                                if verified_km != *existing {
                                                    log::error!("Verification failed! Device keymap does not match local keymap.");
                                                    
                                                    // Log exactly what changed to console to help debug
                                                    for (layer_idx, (old_layer, new_layer)) in existing.layers.iter().zip(verified_km.layers.iter()).enumerate() {
                                                        for (key_idx, (old_binding, new_binding)) in old_layer.bindings.iter().zip(new_layer.bindings.iter()).enumerate() {
                                                            if old_binding != new_binding {
                                                                log::error!("Layer {}, Key {}: Expected '{}', but device returned '{}'", layer_idx, key_idx, old_binding, new_binding);
                                                            }
                                                        }
                                                    }

                                                    error.set(Some("Verification failed: The device did not correctly save the layout. Your changes might be lost on restart.".to_string()));
                                                    
                                                    // Sync back the real device state
                                                    proto_layers.set(layers);
                                                    keymap_data.set(Some(verified_km));
                                                } else {
                                                    log::info!("Verification successful! Device matches local state perfectly.");
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!("Could not fetch keymap to verify save: {}", e);
                                            error.set(Some(format!("Saved, but could not verify device state: {}", e)));
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to save: {}", e);
                                    error.set(Some(format!("Failed to save: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed connection check: {}", e);
                            error.set(Some(format!("Keyboard connection check failed: {}", e)));
                        }
                    }
                });
            }
        })
    };

    let on_download_keymap = {
        let keymap_data = keymap_data.clone();
        let device_name = device_name.clone();
        let error = error.clone();
        let loading = loading.clone();

        Callback::from(move |_: MouseEvent| {
            if let Some(data) = &*keymap_data {
                let data = data.clone();
                let dn = (*device_name).clone();
                let error = error.clone();
                let loading = loading.clone();

                loading.set(true);
                spawn_local(async move {
                    let result = gloo_net::http::Request::post("/api/save-keymap")
                        .json(&crate::keymap::SaveKeymapRequest {
                            original_content: String::new(),
                            data,
                        })
                        .unwrap()
                        .send()
                        .await;

                    match result {
                        Ok(resp) => {
                            loading.set(false);
                            if resp.ok() {
                                match resp.json::<crate::keymap::SaveKeymapResponse>().await {
                                    Ok(res) => {
                                        let blob = web_sys::Blob::new_with_str_sequence(
                                            &js_sys::Array::of1(&JsValue::from_str(&res.content)),
                                        )
                                        .unwrap();
                                        let url = web_sys::Url::create_object_url_with_blob(&blob)
                                            .unwrap();
                                        let window = web_sys::window().unwrap();
                                        let document = window.document().unwrap();
                                        let link = document
                                            .create_element("a")
                                            .unwrap()
                                            .dyn_into::<web_sys::HtmlAnchorElement>()
                                            .unwrap();
                                        link.set_href(&url);
                                        let filename = if dn.is_empty() {
                                            "keymap.keymap".to_string()
                                        } else {
                                            format!("{}.keymap", dn)
                                        };
                                        link.set_download(&filename);
                                        link.click();
                                        web_sys::Url::revoke_object_url(&url).unwrap();
                                    }
                                    Err(e) => {
                                        error.set(Some(format!(
                                            "Failed to parse server response: {}",
                                            e
                                        )));
                                    }
                                }
                            } else {
                                let error_text = resp
                                    .text()
                                    .await
                                    .unwrap_or_else(|_| "Unknown error".to_string());
                                error.set(Some(format!("Server error: {}", error_text)));
                            }
                        }
                        Err(e) => {
                            loading.set(false);
                            error.set(Some(format!("Network error: {}", e)));
                        }
                    }
                });
            }
        })
    };

    // --- Patch keymap handler (merge layers into uploaded file) ---
    let on_patch_keymap = {
        let keymap_data = keymap_data.clone();
        let error = error.clone();
        let loading = loading.clone();

        Callback::from(move |_: MouseEvent| {
            if keymap_data.is_none() {
                return;
            }
            let keymap_data = keymap_data.clone();
            let error = error.clone();
            let loading = loading.clone();

            spawn_local(async move {
                // Open file picker to select an existing keymap file
                let options = js_sys::Object::new();
                let types = js_sys::Array::new();
                let type0 = js_sys::Object::new();
                js_sys::Reflect::set(&type0, &"description".into(), &"ZMK Keymap Files".into())
                    .unwrap();
                let accept = js_sys::Object::new();
                let extensions = js_sys::Array::new();
                extensions.push(&".keymap".into());
                js_sys::Reflect::set(&accept, &"text/plain".into(), &extensions).unwrap();
                js_sys::Reflect::set(&type0, &"accept".into(), &accept).unwrap();
                types.push(&type0);
                js_sys::Reflect::set(&options, &"types".into(), &types).unwrap();
                js_sys::Reflect::set(
                    &options,
                    &"excludeAcceptAllOption".into(),
                    &JsValue::from(true),
                )
                .unwrap();
                js_sys::Reflect::set(&options, &"multiple".into(), &JsValue::from(false)).unwrap();

                let picker_promise = crate::keymap::show_open_file_picker(&options);
                let result = wasm_bindgen_futures::JsFuture::from(picker_promise).await;

                match result {
                    Ok(handles) => {
                        let handles: js_sys::Array = handles.unchecked_into();
                        if handles.length() > 0 {
                            let handle_val = handles.get(0);
                            let handle: crate::keymap::FileSystemFileHandle = handle_val.unchecked_into();

                            loading.set(true);
                            error.set(None);
                            
                            let file_promise = handle.get_file();
                            let file_result = wasm_bindgen_futures::JsFuture::from(file_promise).await;

                            match file_result {
                                Ok(file_val) => {
                                    let file: web_sys::File = file_val.unchecked_into();
                                    let content_promise = file.text();
                                    let content_result = wasm_bindgen_futures::JsFuture::from(content_promise).await;

                                    match content_result {
                                        Ok(content_val) => {
                                            let file_content = content_val.as_string().unwrap_or_default();
                                            
                                            if let Some(data) = &*keymap_data {
                                                let data = data.clone();
                                                
                                                // Send to server for patching
                                                let patch_result = gloo_net::http::Request::post("/api/patch-keymap")
                                                    .json(&crate::keymap::PatchKeymapRequest {
                                                        file_content,
                                                        data,
                                                    })
                                                    .unwrap()
                                                    .send()
                                                    .await;

                                                match patch_result {
                                                    Ok(resp) => {
                                                        loading.set(false);
                                                        if resp.ok() {
                                                            match resp.json::<crate::keymap::PatchKeymapResponse>().await {
                                                                Ok(res) => {
                                                                    // Save back to the original file handle
                                                                    let writable_promise = handle.create_writable();
                                                                    let writable_result = wasm_bindgen_futures::JsFuture::from(writable_promise).await;

                                                                    match writable_result {
                                                                        Ok(writable_val) => {
                                                                            let writable: crate::keymap::FileSystemWritableFileStream = writable_val.unchecked_into();
                                                                            let write_promise = writable.write(&JsValue::from_str(&res.content));
                                                                            let _ = wasm_bindgen_futures::JsFuture::from(write_promise).await;
                                                                            let close_promise = writable.close();
                                                                            let _ = wasm_bindgen_futures::JsFuture::from(close_promise).await;
                                                                            error.set(None);
                                                                        }
                                                                        Err(e) => {
                                                                            error.set(Some(format!(
                                                                                "Failed to save file: {:?}",
                                                                                e
                                                                            )));
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    error.set(Some(format!(
                                                                        "Failed to parse server response: {}",
                                                                        e
                                                                    )));
                                                                }
                                                            }
                                                        } else {
                                                            let error_text = resp
                                                                .text()
                                                                .await
                                                                .unwrap_or_else(|_| "Unknown error".to_string());
                                                            error.set(Some(format!("Server error: {}", error_text)));
                                                        }
                                                    }
                                                    Err(e) => {
                                                        loading.set(false);
                                                        error.set(Some(format!("Network error: {}", e)));
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            loading.set(false);
                                            error.set(Some(format!(
                                                "Failed to read file content: {:?}",
                                                e
                                            )));
                                        }
                                    }
                                }
                                Err(e) => {
                                    loading.set(false);
                                    error.set(Some(format!(
                                        "Failed to get file from handle: {:?}",
                                        e
                                    )));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // User cancelled file picker, not an error
                        log::debug!("File picker cancelled: {:?}", e);
                    }
                }
            });
        })
    };

    // --- Discard handler ---
    let on_discard = {
        let rpc_client = rpc_client.clone();
        let error = error.clone();
        let keymap_data = keymap_data.clone();
        let behavior_cache = behavior_cache.clone();
        let proto_layers = proto_layers.clone();
        let has_unsaved_changes = has_unsaved_changes.clone();

        Callback::from(move |_: MouseEvent| {
            if let Some(client) = (*rpc_client).clone() {
                let error = error.clone();
                let keymap_data = keymap_data.clone();
                let cache = (*behavior_cache).clone();
                let proto_layers = proto_layers.clone();
                let has_unsaved = has_unsaved_changes.clone();
                spawn_local(async move {
                    match client.discard_changes().await {
                        Ok(_) => {
                            has_unsaved.set(false);
                            // Re-fetch keymap
                            match client.get_keymap().await {
                                Ok(data) => {
                                    let (layers, _, _) =
                                        convert::parse_keymap(&data, &cache);
                                    if let Some(existing) = &*keymap_data {
                                        let km = convert::to_keymap_data(
                                            existing.physical_layout.clone(),
                                            &layers,
                                            &cache,
                                        );
                                        proto_layers.set(layers);
                                        keymap_data.set(Some(km));
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to re-fetch keymap: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Failed to discard: {}", e)));
                        }
                    }
                });
            }
        })
    };

    // --- Global keyboard shortcuts ---
    {
        let save = on_save.clone();
        let has_unsaved = has_unsaved_changes.clone();
        use_effect(move || {
            let listener = Closure::wrap(Box::new(move |e: KeyboardEvent| {
                if (e.ctrl_key() || e.meta_key()) && e.key() == "s" {
                    if *has_unsaved {
                        e.prevent_default();
                        save.emit(MouseEvent::new("click").unwrap());
                    }
                }
            }) as Box<dyn FnMut(KeyboardEvent)>);

            let window = web_sys::window().expect("should have a window");
            window
                .add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
                .expect("failed to add listener");

            move || {
                let window = web_sys::window().expect("should have a window");
                window
                    .remove_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
                    .expect("failed to remove listener");
                drop(listener);
            }
        });
    }

    // --- Reload data when unlocked ---
    {
        let connected = connected.clone();
        let loading = loading.clone();
        let error = error.clone();
        let lock_state = lock_state.clone();
        let rpc_client = rpc_client.clone();
        let behavior_cache = behavior_cache.clone();
        let keymap_data = keymap_data.clone();
        let proto_layers = proto_layers.clone();
        let device_name = device_name.clone();
        let dumped_bytes = dumped_bytes.clone();

        use_effect_with(lock_state, move |ls| {
            if !*connected || **ls == 0 || keymap_data.is_some() {
                return;
            }

            let client = match &*rpc_client {
                Some(c) => c.clone(),
                None => return,
            };

            let loading = loading.clone();
            let error = error.clone();
            let behavior_cache = behavior_cache.clone();
            let keymap_data = keymap_data.clone();
            let proto_layers = proto_layers.clone();
            let device_name = device_name.clone();
            let db = dumped_bytes.clone();

            spawn_local(async move {
                loading.set(true);
                error.set(None);

                // Re-fetch device info if empty
                if device_name.is_empty() {
                    match client.get_device_info().await {
                        Ok(info) => {
                            device_name.set(info.name.clone());
                        }
                        Err(_) => {}
                    }
                }

                // Get behaviors
                let mut cache = BehaviorCache::new();
                match client.list_all_behaviors().await {
                    Ok(data) => {
                        for bid in data.behaviors {
                            match client.get_behavior_details(bid).await {
                                Ok(detail) => {
                                    cache.register_behavior(&detail);
                                }
                                Err(e) => {
                                    log::warn!("Failed to get behavior details for {}: {}", bid, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to list behaviors: {}", e)));
                        loading.set(false);
                        return;
                    }
                }

                // Get physical layouts
                let physical_keys = match client.get_physical_layouts().await {
                    Ok(data) => {
                        let (_active, layouts) = convert::parse_physical_layouts(&data);
                        if let Some((_, keys)) = layouts.into_iter().next() {
                            keys
                        } else {
                            error.set(Some("No physical layouts found".into()));
                            loading.set(false);
                            return;
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to get physical layouts: {}", e)));
                        loading.set(false);
                        return;
                    }
                };

                // Get keymap
                match client.get_keymap().await {
                    Ok(data) => {
                        let (layers, _available, _max_name) =
                            convert::parse_keymap(&data, &cache);
                        let km = convert::to_keymap_data(physical_keys, &layers, &cache);
                        proto_layers.set(layers);
                        keymap_data.set(Some(km));
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to get keymap: {}", e)));
                        loading.set(false);
                        return;
                    }
                }

                behavior_cache.set(cache);
                loading.set(false);

                if is_dump_init && !db.is_empty() {
                    let hex_dumps: Vec<String> = db.iter().map(|b| hex::encode(b)).collect();
                    if let Ok(json) = serde_json::to_string_pretty(&hex_dumps) {
                        let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&json))).unwrap();
                        let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                        let document = web_sys::window().unwrap().document().unwrap();
                        let anchor = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                        anchor.set_href(&url);
                        anchor.set_download("zmk_init_dump.json");
                        anchor.click();
                        web_sys::Url::revoke_object_url(&url).unwrap();
                    }
                }
            });
        });
    }

    html! {
        <div class="w-full flex flex-col items-center">
            <h2 class="text-2xl font-bold mt-4 mb-2">{"ZMK Studio"}</h2>

            { if !*connected {
                html! {
                    <div class="flex flex-col items-center gap-4 my-8">
                        <p class="text-gray-600 dark:text-gray-400 max-w-md text-center">
                            {"Connect to a ZMK keyboard via USB Serial or Bluetooth to view and edit its keymap in real time."}
                        </p>
                        <div class="flex gap-4 items-center">
                            <label class="flex items-center gap-2 cursor-pointer">
                                <input
                                    type="radio"
                                    name="transport"
                                    checked={*transport_type == TransportType::Serial}
                                    onchange={let tt = transport_type.clone(); Callback::from(move |_| tt.set(TransportType::Serial))}
                                />
                                {"USB Serial"}
                            </label>
                            <label class="flex items-center gap-2 cursor-pointer">
                                <input
                                    type="radio"
                                    name="transport"
                                    checked={*transport_type == TransportType::Bluetooth}
                                    onchange={let tt = transport_type.clone(); Callback::from(move |_| tt.set(TransportType::Bluetooth))}
                                />
                                {"Bluetooth"}
                            </label>
                        </div>
                        <button
                            onclick={on_connect}
                            disabled={*loading}
                            class="px-6 py-2.5 bg-blue-600 text-white font-medium text-sm leading-tight uppercase rounded shadow-md hover:bg-blue-700 hover:shadow-lg focus:bg-blue-700 focus:shadow-lg focus:outline-none focus:ring-0 active:bg-blue-800 active:shadow-lg transition duration-150 ease-in-out disabled:opacity-50"
                        >
                            { if *loading { "Connecting..." } else { "Connect" } }
                        </button>
                    </div>
                }
            } else {
                html! {
                    <div class="flex flex-col items-center w-full">
                        <div class="flex items-center gap-4 mb-4">
                            <span class="inline-flex items-center gap-1.5 text-sm text-green-600 dark:text-green-400">
                                <span class="w-2 h-2 bg-green-500 rounded-full inline-block"></span>
                                {"Connected to "}
                                <strong>{&*device_name}</strong>
                            </span>
                            { if *lock_state == 0 {
                                html! {
                                    <span class="text-sm text-yellow-600 dark:text-yellow-400 bg-yellow-50 dark:bg-yellow-900/30 px-2 py-1 rounded">
                                        {"Locked — unlock on keyboard to edit"}
                                    </span>
                                }
                            } else {
                                html! {
                                    <div class="flex items-center gap-2">
                                        <button onclick={on_save} disabled={!*has_unsaved_changes} class={classes!("px-4", "py-1.5", "text-white", "font-medium", "text-xs", "uppercase", "rounded", "shadow", "transition", if *has_unsaved_changes { vec!["bg-green-600", "hover:bg-green-700"] } else { vec!["bg-gray-400", "cursor-not-allowed", "opacity-50"] })}>
                                            {"Save to Flash"}
                                        </button>
                                        <button onclick={on_download_keymap} class="px-4 py-1.5 bg-blue-600 text-white font-medium text-xs uppercase rounded shadow hover:bg-blue-700 transition">
                                            {"Download Keymap"}
                                        </button>
                                        <button onclick={on_patch_keymap} class="px-4 py-1.5 bg-purple-600 text-white font-medium text-xs uppercase rounded shadow hover:bg-purple-700 transition">
                                            {"Patch Keymap"}
                                        </button>
                                        { if *has_unsaved_changes {
                                            html! {
                                                <button onclick={on_discard} class="px-4 py-1.5 bg-orange-500 text-white font-medium text-xs uppercase rounded shadow hover:bg-orange-600 transition">
                                                    {"Discard Changes"}
                                                </button>
                                            }
                                        } else { html! {} }}
                                        <button onclick={let kd = keymap_data.clone(); let dn = device_name.clone(); Callback::from(move |_: MouseEvent| {
                                            if let Some(km) = &*kd {
                                                let svg = crate::keymap::generate_svg(km, false, false, false);
                                                let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&JsValue::from_str(&svg))).unwrap();
                                                let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                                                let document = web_sys::window().unwrap().document().unwrap();
                                                let anchor = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                                                anchor.set_href(&url);
                                                let filename = if dn.is_empty() { "keymap.svg".to_string() } else { format!("{}.svg", *dn) };
                                                anchor.set_download(&filename);
                                                anchor.click();
                                                web_sys::Url::revoke_object_url(&url).unwrap();
                                            }
                                        })} class="px-4 py-1.5 bg-indigo-600 text-white font-medium text-xs uppercase rounded shadow hover:bg-indigo-700 transition">
                                            {"Download SVG"}
                                        </button>
                                    </div>
                                }
                            }}
                            <button onclick={on_disconnect} class="px-4 py-1.5 bg-gray-500 text-white font-medium text-xs uppercase rounded shadow hover:bg-gray-600 transition">
                                {"Disconnect"}
                            </button>
                        </div>
                        <p class="mb-4 text-xs text-gray-500 dark:text-gray-400">{"Type "} <kbd class="px-1.5 py-0.5 font-sans font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-lg dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500">{"j"}</kbd> {" to start jump mode"}</p>
                    </div>
                }
            }}

            { if *loading {
                html! { <div class="text-blue-500 mb-4 animate-pulse">{"Connecting and loading keymap..."}</div> }
            } else { html! {} }}

            { if let Some(err) = &*error {
                html! { <div class="text-red-500 mb-4 px-4 py-2 bg-red-50 dark:bg-red-900/20 rounded">{err}</div> }
            } else { html! {} }}

            { if let Some(data) = &*keymap_data {
                html! { <KeymapRenderer data={data.clone()} on_update={on_update.clone()} /> }
            } else if *connected && !*loading {
                html! { <div class="text-gray-500 italic mt-4">{"No keymap data loaded."}</div> }
            } else { html! {} }}
        </div>
    }
}
