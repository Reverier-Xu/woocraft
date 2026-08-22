// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Ported and adapted from gpui-tray (https://github.com/Yamrc/gpui-tray),
// Copyright (c) Yamrc, MPL-2.0. Modifications: menu items carry string ids,
// the icon is supplied as raw bytes, and tracing is used instead of log.

//! Linux system tray backend implementing the freedesktop StatusNotifierItem
//! (SNI) + `com.canonical.dbusmenu` protocols over D-Bus.
//!
//! Ported from gpui-tray (MPL-2.0); adapted so menu entries carry string ids
//! and the icon is supplied as raw bytes.

pub(super) mod dbus;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crossbeam_channel::Receiver;
use tracing::{debug, error};

use dbus::{DbusService, ItemState, MenuState, TrayEvent};
use super::platform::{BackendError, Error, PlatformTray, Result};
use super::{Tray, TrayEvent as RuntimeTrayEvent, TrayMenuItem, TrayMouseButton};

enum BackendCommand {
    SetTray {
        tray: Tray,
        response: std::sync::mpsc::Sender<Result<()>>,
    },
    RemoveTray {
        response: std::sync::mpsc::Sender<Result<()>>,
    },
    Shutdown,
}

pub(crate) struct LinuxBackend {
    command_tx: std::sync::mpsc::Sender<BackendCommand>,
}

impl LinuxBackend {
    fn send_and_wait(&self, cmd: impl FnOnce(std::sync::mpsc::Sender<Result<()>>) -> BackendCommand) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.command_tx
            .send(cmd(tx))
            .map_err(|_| Error::Backend(BackendError::ChannelSend))?;

        rx.recv()
            .map_err(|_| Error::Backend(BackendError::ChannelReceive))?
    }
}

impl PlatformTray for LinuxBackend {
    fn set_tray(&self, tray: &Tray) -> Result<()> {
        self.send_and_wait(|response| BackendCommand::SetTray {
            tray: tray.clone(),
            response,
        })
    }

    fn remove_tray(&self) -> Result<()> {
        self.send_and_wait(|response| BackendCommand::RemoveTray { response })
    }

    fn shutdown(&self) -> Result<()> {
        if self.command_tx.send(BackendCommand::Shutdown).is_err() {
            return Err(Error::RuntimeClosed);
        }
        Ok(())
    }
}

struct WorkerState {
    service: Option<DbusService>,
    item_state: Arc<Mutex<ItemState>>,
    menu_state: Arc<Mutex<MenuState>>,
    menu_ids: HashMap<i32, String>,
    current_tray: Option<Tray>,
    tray_event_tx: std::sync::mpsc::Sender<dbus::TrayEvent>,
}

impl WorkerState {
    fn new(tray_event_tx: std::sync::mpsc::Sender<dbus::TrayEvent>) -> Self {
        Self {
            service: None,
            item_state: Arc::new(Mutex::new(ItemState::new())),
            menu_state: Arc::new(Mutex::new(MenuState::new())),
            menu_ids: HashMap::new(),
            current_tray: None,
            tray_event_tx,
        }
    }

    fn apply_set_tray(&mut self, tray: Tray) -> Result<()> {
        self.current_tray = Some(tray.clone());

        if !tray.visible {
            self.hide_tray();
            return Ok(());
        }

        let had_service = self.service.is_some();

        self.update_item_state(&tray)?;
        let menu_revision = self.rebuild_menu(&tray)?;
        self.ensure_service()?;

        if had_service {
            let service = self.service.as_ref().ok_or(Error::RuntimeClosed)?;
            service.notify_updated(menu_revision).map_err(|err| {
                Error::Backend(BackendError::platform(
                    "DbusService::notify_updated",
                    err.to_string(),
                ))
            })?;
        }

        Ok(())
    }

    fn apply_remove_tray(&mut self) -> Result<()> {
        if self.current_tray.is_none() {
            return Err(Error::NotFound);
        }

        self.current_tray = None;
        self.hide_tray();
        Ok(())
    }

    fn hide_tray(&mut self) {
        self.service = None;
        self.menu_ids.clear();

        if let Ok(mut item_state) = self.item_state.lock() {
            item_state.icon = None;
        }

        if let Ok(mut menu_state) = self.menu_state.lock() {
            menu_state.clear();
        }
    }

    fn ensure_service(&mut self) -> Result<()> {
        if self.service.is_some() {
            return Ok(());
        }

        let service = DbusService::new(
            self.item_state.clone(),
            self.menu_state.clone(),
            self.tray_event_tx.clone(),
        )
        .map_err(|err| {
            Error::Backend(BackendError::platform("DbusService::new", err.to_string()))
        })?;
        self.service = Some(service);
        Ok(())
    }

    fn update_item_state(&mut self, tray: &Tray) -> Result<()> {
        let mut state = lock_mutex(&self.item_state)?;

        state.tooltip = tray
            .tooltip
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();

        state.title = tray
            .title
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| {
                if state.tooltip.is_empty() {
                    None
                } else {
                    Some(state.tooltip.clone())
                }
            })
            .unwrap_or_else(|| "wsrx".to_string());

        state.icon = match tray.icon_bytes.as_ref() {
            Some(bytes) => Some(Icon::from_bytes(bytes)?.as_pixmaps().to_vec()),
            None => None,
        };

        debug!(
            "linux item state updated: title='{}', tooltip_len={}, has_icon={}",
            state.title,
            state.tooltip.len(),
            state.icon.is_some()
        );

        Ok(())
    }

    fn rebuild_menu(&mut self, tray: &Tray) -> Result<u32> {
        let mut menu_ids = HashMap::new();
        let revision;
        {
            let mut menu_state = lock_mutex(&self.menu_state)?;
            menu_state.clear();

            for item in &tray.menu {
                add_menu_item(&mut menu_state, &mut menu_ids, item, 0);
            }

            menu_state.mark_updated();
            revision = menu_state.revision();
        }

        debug!("linux menu rebuild: ids={}, revision={}", menu_ids.len(), revision);
        self.menu_ids = menu_ids;
        Ok(revision)
    }
}

pub(crate) fn create() -> Result<(Box<dyn PlatformTray>, Receiver<RuntimeTrayEvent>)> {
    let (command_tx, command_rx) = std::sync::mpsc::channel::<BackendCommand>();
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<RuntimeTrayEvent>();
    let (boot_tx, boot_rx) = std::sync::mpsc::channel::<Result<()>>();

    thread::Builder::new()
        .name("woocraft-tray-linux".to_string())
        .spawn(move || {
            backend_thread_main(command_rx, event_tx, boot_tx);
        })
        .map_err(|err| Error::Backend(BackendError::platform("spawn", err.to_string())))?;

    boot_rx
        .recv()
        .map_err(|_| Error::Backend(BackendError::ChannelReceive))??;

    Ok((
        Box::new(LinuxBackend { command_tx }),
        event_rx,
    ))
}

fn backend_thread_main(
    command_rx: std::sync::mpsc::Receiver<BackendCommand>,
    runtime_event_tx: crossbeam_channel::Sender<RuntimeTrayEvent>,
    boot_tx: std::sync::mpsc::Sender<Result<()>>,
) {
    let (tray_event_tx, tray_event_rx) = std::sync::mpsc::channel::<dbus::TrayEvent>();
    let mut state = WorkerState::new(tray_event_tx);

    let _ = boot_tx.send(Ok(()));

    let mut running = true;
    while running {
        match command_rx.recv_timeout(Duration::from_millis(12)) {
            Ok(command) => {
                running = handle_command(&mut state, command);

                while let Ok(command) = command_rx.try_recv() {
                    if !handle_command(&mut state, command) {
                        running = false;
                        break;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                running = false;
            }
        }

        while let Ok(event) = tray_event_rx.try_recv() {
            handle_tray_event(&state, event, &runtime_event_tx);
        }
    }

    state.hide_tray();
}

fn handle_command(state: &mut WorkerState, command: BackendCommand) -> bool {
    match command {
        BackendCommand::SetTray { tray, response } => {
            let _ = response.send(state.apply_set_tray(tray));
            true
        }
        BackendCommand::RemoveTray { response } => {
            let _ = response.send(state.apply_remove_tray());
            true
        }
        BackendCommand::Shutdown => false,
    }
}

fn handle_tray_event(
    state: &WorkerState, event: TrayEvent, runtime_event_tx: &crossbeam_channel::Sender<RuntimeTrayEvent>,
) {
    match event {
        TrayEvent::Activate { x, y } => {
            dispatch_click(runtime_event_tx, TrayMouseButton::Left, x, y);
        }
        TrayEvent::SecondaryActivate { x, y } => {
            dispatch_click(runtime_event_tx, TrayMouseButton::Middle, x, y);
        }
        TrayEvent::ContextMenu { x, y } => {
            dispatch_click(runtime_event_tx, TrayMouseButton::Right, x, y);
        }
        TrayEvent::MenuClicked { id } => {
            if let Some(item_id) = state.menu_ids.get(&id) {
                let _ = runtime_event_tx.send(RuntimeTrayEvent::MenuClicked {
                    id: item_id.clone(),
                });
            } else {
                error!("linux menu click id={id} had no mapped action");
            }
        }
    }
}

fn dispatch_click(
    runtime_event_tx: &crossbeam_channel::Sender<RuntimeTrayEvent>, button: TrayMouseButton, x: i32,
    y: i32,
) {
    debug!("linux click button={button:?}, x={x}, y={y}");

    let _ = runtime_event_tx.send(RuntimeTrayEvent::Click {
        button,
        position: (x as f32, y as f32),
    });
}

fn add_menu_item(
    menu_state: &mut MenuState, menu_ids: &mut HashMap<i32, String>, item: &TrayMenuItem,
    parent_id: i32,
) {
    match item {
        TrayMenuItem::Separator => {
            menu_state.add_separator(parent_id);
        }
        TrayMenuItem::Action { id, label, enabled } => {
            let item_id = menu_state.add_item(label.clone(), *enabled, parent_id);
            menu_ids.insert(item_id, id.clone());
        }
        TrayMenuItem::Submenu { label, items } => {
            let sub_id = menu_state.add_item(label.clone(), true, parent_id);
            for child in items {
                add_menu_item(menu_state, menu_ids, child, sub_id);
            }
        }
    }
}

fn lock_mutex<'a, T>(mutex: &'a Mutex<T>) -> Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| Error::RuntimeClosed)
}

/// Decodes tray icon bytes into a set of ARGB pixmaps at common tray sizes.
///
/// Ported from gpui-tray (MPL-2.0).
const ICON_SIZES: [u32; 4] = [16, 24, 32, 48];

struct Icon {
    pixmaps: Arc<Vec<Pixmap>>,
}

impl Icon {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let img = image::load_from_memory(bytes).map_err(|_| Error::InvalidIcon)?;

        let mut pixmaps = Vec::with_capacity(ICON_SIZES.len());
        for size in ICON_SIZES {
            let resized = img.resize_to_fill(size, size, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8();
            pixmaps.push(Pixmap::new(size as i32, size as i32, rgba_to_argb(&rgba)));
        }

        Ok(Self {
            pixmaps: Arc::new(pixmaps),
        })
    }

    fn as_pixmaps(&self) -> &[Pixmap] {
        &self.pixmaps
    }
}

#[derive(Clone)]
pub(crate) struct Pixmap {
    width: i32,
    height: i32,
    data: Vec<u8>,
}

impl Pixmap {
    fn new(width: i32, height: i32, data: Vec<u8>) -> Self {
        Self { width, height, data }
    }
}

impl From<Pixmap> for zbus::zvariant::Structure<'_> {
    fn from(value: Pixmap) -> Self {
        zbus::zvariant::StructureBuilder::new()
            .add_field(value.width)
            .add_field(value.height)
            .add_field(value.data)
            .build()
            .expect("Pixmap structure build should not fail")
    }
}

fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    let mut argb = Vec::with_capacity(rgba.len());
    for chunk in rgba.as_chunks::<4>().0 {
        argb.push(chunk[3]);
        argb.push(chunk[0]);
        argb.push(chunk[1]);
        argb.push(chunk[2]);
    }
    argb
}
