// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Ported and adapted from gpui-tray (https://github.com/Yamrc/gpui-tray),
// Copyright (c) Yamrc, MPL-2.0. Modifications: menu items carry string ids,
// the icon is supplied as raw bytes, and tracing is used instead of log.

//! StatusNotifierItem + `com.canonical.dbusmenu` D-Bus services.
//!
//! Ported from gpui-tray (MPL-2.0).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::debug;
use zbus::zvariant::Value;
use zbus::{blocking::Connection, interface};

use super::Pixmap;

const STATUS_NOTIFIER_ITEM_PATH: &str = "/StatusNotifierItem";
const DBUS_MENU_PATH: &str = "/MenuBar";
const STATUS_NOTIFIER_ITEM_IFACE: &str = "org.kde.StatusNotifierItem";
const DBUS_MENU_IFACE: &str = "com.canonical.dbusmenu";
const STATUS_NOTIFIER_WATCHER: &str = "org.kde.StatusNotifierWatcher";
const STATUS_NOTIFIER_WATCHER_PATH: &str = "/StatusNotifierWatcher";

pub(crate) type PixmapData = Vec<u8>;
pub(crate) type PixmapTuple = (i32, i32, PixmapData);
pub(crate) type Tooltip = (String, Vec<PixmapTuple>, String, String);
pub(crate) type LayoutItem = (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>);
pub(crate) type LayoutResult = (u32, LayoutItem);

#[derive(Debug, Clone)]
pub(crate) enum TrayEvent {
    Activate { x: i32, y: i32 },
    SecondaryActivate { x: i32, y: i32 },
    ContextMenu { x: i32, y: i32 },
    MenuClicked { id: i32 },
}

pub(crate) struct ItemState {
    pub title: String,
    pub tooltip: String,
    pub icon: Option<Vec<Pixmap>>,
}

impl ItemState {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            tooltip: String::new(),
            icon: None,
        }
    }
}

pub(crate) struct StatusNotifierItem {
    state: Arc<Mutex<ItemState>>,
    event_sender: std::sync::mpsc::Sender<TrayEvent>,
}

impl StatusNotifierItem {
    pub fn new(
        state: Arc<Mutex<ItemState>>,
        event_sender: std::sync::mpsc::Sender<TrayEvent>,
    ) -> Self {
        Self { state, event_sender }
    }
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> String {
        self.state.lock().map(|s| s.title.clone()).unwrap_or_default()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        self.state.lock().map(|s| s.title.clone()).unwrap_or_default()
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property, name = "IconName")]
    fn icon_name(&self) -> &str {
        ""
    }

    #[zbus(property, name = "IconPixmap")]
    fn icon_pixmap(&self) -> Vec<PixmapTuple> {
        self.state
            .lock()
            .ok()
            .and_then(|s| {
                s.icon.as_ref().map(|pixmaps| {
                    pixmaps
                        .iter()
                        .map(|p| (p.width, p.height, p.data.clone()))
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    #[zbus(property, name = "ToolTip")]
    fn tooltip(&self) -> Tooltip {
        let state = self.state.lock().unwrap();
        (String::new(), Vec::new(), state.tooltip.clone(), String::new())
    }

    #[zbus(property)]
    fn menu(&self) -> zbus::zvariant::ObjectPath<'_> {
        zbus::zvariant::ObjectPath::from_static_str(DBUS_MENU_PATH)
            .unwrap_or_else(|_| zbus::zvariant::ObjectPath::from_static_str("/").unwrap())
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    fn activate(&self, x: i32, y: i32) {
        debug!("Received activate with position=({x}, {y})");
        let _ = self.event_sender.send(TrayEvent::Activate { x, y });
    }

    fn secondary_activate(&self, x: i32, y: i32) {
        debug!("Received secondary_activate with position=({x}, {y})");
        let _ = self
            .event_sender
            .send(TrayEvent::SecondaryActivate { x, y });
    }

    fn context_menu(&self, x: i32, y: i32) {
        debug!("Received context_menu with position=({x}, {y})");
        let _ = self.event_sender.send(TrayEvent::ContextMenu { x, y });
    }

    fn scroll(&self, _delta: i32, _orientation: &str) {}
}

struct MenuItem {
    id: i32,
    label: String,
    enabled: bool,
    visible: bool,
    item_type: MenuItemType,
    children: Vec<i32>,
}

enum MenuItemType {
    Standard,
    Separator,
}

pub(crate) struct MenuState {
    items: HashMap<i32, MenuItem>,
    next_id: i32,
    revision: u32,
}

impl MenuState {
    pub fn new() -> Self {
        let mut state = Self {
            items: HashMap::new(),
            next_id: 1,
            revision: 1,
        };
        state.clear();
        state
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.items.insert(
            0,
            MenuItem {
                id: 0,
                label: String::new(),
                enabled: true,
                visible: true,
                item_type: MenuItemType::Standard,
                children: Vec::new(),
            },
        );
        self.next_id = 1;
    }

    pub fn add_item(&mut self, label: impl Into<String>, enabled: bool, parent_id: i32) -> i32 {
        let id = self.next_id;
        self.next_id += 1;

        let item = MenuItem {
            id,
            label: label.into(),
            enabled,
            visible: true,
            item_type: MenuItemType::Standard,
            children: Vec::new(),
        };

        self.items.insert(id, item);

        if let Some(parent) = self.items.get_mut(&parent_id) {
            parent.children.push(id);
        }

        id
    }

    pub fn add_separator(&mut self, parent_id: i32) -> i32 {
        let id = self.next_id;
        self.next_id += 1;

        let item = MenuItem {
            id,
            label: String::new(),
            enabled: false,
            visible: true,
            item_type: MenuItemType::Separator,
            children: Vec::new(),
        };

        self.items.insert(id, item);

        if let Some(parent) = self.items.get_mut(&parent_id) {
            parent.children.push(id);
        }

        id
    }

    pub fn mark_updated(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn revision(&self) -> u32 {
        self.revision
    }

    fn item_to_properties(
        &self, item: &MenuItem, property_names: &[String],
    ) -> HashMap<String, Value<'static>> {
        let mut props = HashMap::new();
        let include_all = property_names.is_empty();

        if include_all || property_names.iter().any(|p| p == "label") {
            props.insert("label".to_string(), Value::from(item.label.clone()));
        }

        if include_all || property_names.iter().any(|p| p == "enabled") {
            props.insert("enabled".to_string(), Value::from(item.enabled));
        }

        if include_all || property_names.iter().any(|p| p == "visible") {
            props.insert("visible".to_string(), Value::from(item.visible));
        }

        if include_all || property_names.iter().any(|p| p == "type") {
            let type_str = match item.item_type {
                MenuItemType::Standard => "standard",
                MenuItemType::Separator => "separator",
            };
            props.insert("type".to_string(), Value::from(type_str));
        }

        if !item.children.is_empty()
            && (include_all || property_names.iter().any(|p| p == "children-display"))
        {
            props.insert("children-display".to_string(), Value::from("submenu"));
        }

        props
    }

    fn build_layout(
        &self, item_id: i32, recursion_depth: i32, property_names: &[String],
    ) -> LayoutItem {
        let item = match self.items.get(&item_id) {
            Some(item) => item,
            None => return (item_id, HashMap::new(), Vec::new()),
        };

        let properties = self.item_to_properties(item, property_names);

        let children = if recursion_depth != 0 {
            item.children
                .iter()
                .map(|&child_id| {
                    let (id, props, _) =
                        self.build_layout(child_id, recursion_depth - 1, property_names);
                    let tuple: LayoutItem = (id, props, Vec::new());
                    Value::from(tuple)
                })
                .collect()
        } else {
            Vec::new()
        };

        (item.id, properties, children)
    }
}

pub(crate) struct DBusMenu {
    state: Arc<Mutex<MenuState>>,
    event_sender: std::sync::mpsc::Sender<TrayEvent>,
}

impl DBusMenu {
    pub fn new(
        state: Arc<Mutex<MenuState>>,
        event_sender: std::sync::mpsc::Sender<TrayEvent>,
    ) -> Self {
        Self { state, event_sender }
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl DBusMenu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    fn get_layout(
        &self, parent_id: i32, recursion_depth: i32, property_names: Vec<String>,
    ) -> LayoutResult {
        debug!(
            "DBusMenu::get_layout called: parent_id={parent_id}, recursion_depth={recursion_depth}"
        );

        let state = self.state.lock().unwrap();
        let layout = state.build_layout(parent_id, recursion_depth, &property_names);
        debug!("DBusMenu::get_layout returning children={}", layout.2.len());
        (state.revision(), layout)
    }

    fn get_group_properties(
        &self, ids: Vec<i32>, property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, Value<'static>>)> {
        let state = self.state.lock().unwrap();
        ids.into_iter()
            .filter_map(|id| {
                state.items.get(&id).map(|item| {
                    let props = state.item_to_properties(item, &property_names);
                    (id, props)
                })
            })
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> Value<'_> {
        let state = self.state.lock().unwrap();
        state
            .items
            .get(&id)
            .and_then(|item| {
                state
                    .item_to_properties(item, std::slice::from_ref(&name))
                    .into_iter()
                    .find(|(k, _)| k == &name)
                    .map(|(_, v)| v)
            })
            .unwrap_or_else(|| Value::from(""))
    }

    fn event(&self, id: i32, event_id: String, _data: Value<'_>, _timestamp: u32) {
        debug!("Received menu_event with id={id}, event_id={event_id}");
        if event_id == "clicked" {
            let _ = self.event_sender.send(TrayEvent::MenuClicked { id });
        }
    }

    fn event_group(&self, _events: Vec<(i32, String, Value<'_>, u32)>) -> Vec<i32> {
        Vec::new()
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }
}

pub(crate) struct DbusService {
    connection: Arc<Connection>,
}

impl DbusService {
    pub fn new(
        item_state: Arc<Mutex<ItemState>>,
        menu_state: Arc<Mutex<MenuState>>,
        event_sender: std::sync::mpsc::Sender<TrayEvent>,
    ) -> Result<Self, zbus::Error> {
        let service_name = format!(
            "org.freedesktop.StatusNotifierItem-WOOCRAFT-{}",
            std::process::id()
        );

        debug!("D-Bus service create with name={service_name}");

        let item = StatusNotifierItem::new(item_state, event_sender.clone());
        let menu = DBusMenu::new(menu_state, event_sender);

        // Request the name and serve the objects atomically so no method
        // calls are lost (avoids the "requesting name before setting up the
        // object server" warning).
        let connection = Arc::new(
            zbus::blocking::connection::Builder::session()?
                .name(service_name.as_str())?
                .serve_at(STATUS_NOTIFIER_ITEM_PATH, item)?
                .serve_at(DBUS_MENU_PATH, menu)?
                .build()?,
        );

        register_status_notifier_item(&connection, service_name.as_str())?;

        Ok(Self { connection })
    }

    pub fn notify_updated(&self, menu_revision: u32) -> Result<(), zbus::Error> {
        self.connection.emit_signal(
            None::<&str>,
            STATUS_NOTIFIER_ITEM_PATH,
            STATUS_NOTIFIER_ITEM_IFACE,
            "NewIcon",
            &(),
        )?;

        self.connection.emit_signal(
            None::<&str>,
            STATUS_NOTIFIER_ITEM_PATH,
            STATUS_NOTIFIER_ITEM_IFACE,
            "NewToolTip",
            &(),
        )?;

        self.connection.emit_signal(
            None::<&str>,
            STATUS_NOTIFIER_ITEM_PATH,
            STATUS_NOTIFIER_ITEM_IFACE,
            "NewTitle",
            &(),
        )?;

        self.connection.emit_signal(
            None::<&str>,
            DBUS_MENU_PATH,
            DBUS_MENU_IFACE,
            "LayoutUpdated",
            &(menu_revision, 0i32),
        )?;

        debug!("dbus notify_updated: menu_revision={menu_revision}");
        Ok(())
    }
}

fn register_status_notifier_item(
    connection: &Connection, service_name: &str,
) -> Result<(), zbus::Error> {
    let proxy = zbus::blocking::Proxy::new(
        connection,
        STATUS_NOTIFIER_WATCHER,
        STATUS_NOTIFIER_WATCHER_PATH,
        STATUS_NOTIFIER_WATCHER,
    )?;

    if let Err(err) = proxy.call_method("RegisterStatusNotifierItem", &(STATUS_NOTIFIER_ITEM_PATH,))
    {
        debug!("RegisterStatusNotifierItem by path failed: {err}; fallback to service name");
        proxy.call_method("RegisterStatusNotifierItem", &(service_name,))?;
    }

    Ok(())
}
