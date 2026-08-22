// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Ported and adapted from gpui-tray (https://github.com/Yamrc/gpui-tray),
// Copyright (c) Yamrc, MPL-2.0. Modifications: menu items carry string ids,
// the icon is supplied as raw bytes, and tracing is used instead of log.

//! Windows system tray backend using `Shell_NotifyIconW` on a dedicated
//! win32 message loop thread.
//!
//! Ported from gpui-tray (MPL-2.0); adapted so menu entries carry string ids
//! and the icon is supplied as raw bytes.

use std::{
  collections::HashMap,
  ffi::OsStr,
  hash::{DefaultHasher, Hash, Hasher},
  os::windows::ffi::OsStrExt,
  thread,
  time::Duration,
};

use crossbeam_channel::Receiver;
use tracing::{debug, error};
use windows::{
  Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM},
    UI::{
      Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
        Shell_NotifyIconW,
      },
      WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos,
        GetWindowLongPtrW, HICON, HMENU, HWND_MESSAGE, ICONINFO, MF_POPUP, MF_SEPARATOR, MF_STRING,
        MSG, PM_REMOVE, PeekMessageW, PostMessageW, RegisterClassW, RegisterWindowMessageW,
        SetForegroundWindow, SetWindowLongPtrW, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu,
        TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_COMMAND,
        WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_MBUTTONUP, WM_NCCREATE, WM_NULL, WM_RBUTTONUP,
        WNDCLASSW,
      },
    },
  },
  core::PCWSTR,
};

use super::{
  Tray, TrayEvent, TrayMenuItem, TrayMouseButton,
  platform::{BackendError, Error, PlatformTray, Result},
};

const WM_TRAYICON: u32 = WM_APP + 71;
const TRAY_CLASS_NAME: &str = "WOOCRAFT::Tray";
const TRAY_ID: u32 = 1;

enum BackendCommand {
  SetTray {
    tray: Tray,
  },
  RemoveTray,
  IconDecoded {
    revision: u64,
    icon_key: u64,
    decoded: Result<DecodedIcon>,
  },
  Shutdown,
}

struct OwnedMenu(HMENU);

impl Drop for OwnedMenu {
  fn drop(&mut self) {
    if !self.0.is_invalid() {
      unsafe {
        let _ = DestroyMenu(self.0);
      }
    }
  }
}

struct TrayWindowState {
  event_tx: crossbeam_channel::Sender<TrayEvent>,
  command_tx: std::sync::mpsc::Sender<BackendCommand>,
  current_tray: Option<Tray>,
  current_icon: Option<OwnedIcon>,
  current_menu: Option<OwnedMenu>,
  menu_ids: HashMap<u16, String>,
  registered: bool,
  requested_icon_revision: u64,
  current_icon_key: Option<u64>,
  taskbar_restart_msg: u32,
}

impl TrayWindowState {
  fn new(
    event_tx: crossbeam_channel::Sender<TrayEvent>,
    command_tx: std::sync::mpsc::Sender<BackendCommand>,
  ) -> Self {
    Self {
      event_tx,
      command_tx,
      current_tray: None,
      current_icon: None,
      current_menu: None,
      menu_ids: HashMap::new(),
      registered: false,
      requested_icon_revision: 0,
      current_icon_key: None,
      taskbar_restart_msg: unsafe { RegisterWindowMessageW(windows::core::w!("TaskbarCreated")) },
    }
  }

  fn clear_menu(&mut self) {
    self.current_menu.take();
    self.menu_ids.clear();
  }
}

pub(crate) struct WindowsBackend {
  command_tx: std::sync::mpsc::Sender<BackendCommand>,
}

impl PlatformTray for WindowsBackend {
  /// Fire-and-forget: the message-loop thread applies the snapshot in the
  /// background so the caller is never blocked.
  fn set_tray(&self, tray: &Tray) -> Result<()> {
    self
      .command_tx
      .send(BackendCommand::SetTray { tray: tray.clone() })
      .map_err(|_| Error::Backend(BackendError::ChannelSend))
  }

  fn remove_tray(&self) -> Result<()> {
    self
      .command_tx
      .send(BackendCommand::RemoveTray)
      .map_err(|_| Error::Backend(BackendError::ChannelSend))
  }

  fn shutdown(&self) -> Result<()> {
    if self.command_tx.send(BackendCommand::Shutdown).is_err() {
      return Err(Error::RuntimeClosed);
    }
    Ok(())
  }
}

pub(crate) fn create() -> Result<(Box<dyn PlatformTray>, Receiver<TrayEvent>)> {
  let (command_tx, command_rx) = std::sync::mpsc::channel::<BackendCommand>();
  let (event_tx, event_rx) = crossbeam_channel::unbounded::<TrayEvent>();
  let (boot_tx, boot_rx) = std::sync::mpsc::channel::<Result<()>>();

  let thread_command_tx = command_tx.clone();
  thread::Builder::new()
    .name("woocraft-tray-windows".to_string())
    .spawn(move || {
      backend_thread_main(command_rx, thread_command_tx, event_tx, boot_tx);
    })
    .map_err(|err| Error::Backend(BackendError::platform("spawn", err.to_string())))?;

  boot_rx
    .recv()
    .map_err(|_| Error::Backend(BackendError::ChannelReceive))??;

  Ok((Box::new(WindowsBackend { command_tx }), event_rx))
}

fn backend_thread_main(
  command_rx: std::sync::mpsc::Receiver<BackendCommand>,
  command_tx: std::sync::mpsc::Sender<BackendCommand>,
  event_tx: crossbeam_channel::Sender<TrayEvent>, boot_tx: std::sync::mpsc::Sender<Result<()>>,
) {
  let class_name = encode_wide(TRAY_CLASS_NAME);
  let wc = WNDCLASSW {
    lpfnWndProc: Some(window_proc),
    lpszClassName: PCWSTR(class_name.as_ptr()),
    ..Default::default()
  };

  let atom = unsafe { RegisterClassW(&wc) };
  if atom == 0 {
    let _ = boot_tx.send(Err(
      BackendError::platform("RegisterClassW", "returned atom=0").into(),
    ));
    return;
  }

  let mut state = Box::new(TrayWindowState::new(event_tx, command_tx));
  let hwnd = unsafe {
    CreateWindowExW(
      WINDOW_EX_STYLE(0),
      PCWSTR(class_name.as_ptr()),
      None,
      WINDOW_STYLE(0),
      0,
      0,
      0,
      0,
      Some(HWND_MESSAGE),
      None,
      None,
      Some(state.as_mut() as *mut TrayWindowState as *const _),
    )
  };

  let hwnd = match hwnd {
    Ok(hwnd) => hwnd,
    Err(err) => {
      debug!("CreateWindowExW failed: {err:?}");
      let _ = boot_tx.send(Err(
        BackendError::platform("CreateWindowExW", format!("{err:?}")).into(),
      ));
      unsafe {
        let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), None);
      }
      return;
    }
  };

  let _ = boot_tx.send(Ok(()));

  let mut running = true;
  while running {
    process_window_messages();

    match command_rx.recv_timeout(Duration::from_millis(10)) {
      Ok(cmd) => {
        running = handle_command(hwnd, state.as_mut(), cmd);
        while let Ok(cmd) = command_rx.try_recv() {
          if !handle_command(hwnd, state.as_mut(), cmd) {
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
  }

  cleanup(hwnd, state.as_mut());
  unsafe {
    let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), None);
  }
}

fn process_window_messages() {
  let mut msg = MSG::default();
  while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() } {
    unsafe {
      let _ = TranslateMessage(&msg);
      DispatchMessageW(&msg);
    }
  }
}

fn handle_command(hwnd: HWND, state: &mut TrayWindowState, cmd: BackendCommand) -> bool {
  match cmd {
    BackendCommand::SetTray { tray } => {
      match apply_tray_snapshot(hwnd, state, tray.clone()) {
        Ok(()) => schedule_icon_decode(state, tray),
        Err(err) => error!("failed to apply tray snapshot: {err}"),
      }
      true
    }
    BackendCommand::RemoveTray => {
      state.current_tray = None;
      state.requested_icon_revision = state.requested_icon_revision.saturating_add(1);
      remove_tray_icon(hwnd, state);
      state.current_icon = None;
      state.clear_menu();
      true
    }
    BackendCommand::IconDecoded {
      revision,
      icon_key,
      decoded,
    } => {
      if revision != state.requested_icon_revision {
        return true;
      }

      let Some(tray) = state.current_tray.as_ref() else {
        return true;
      };

      if !tray.visible {
        return true;
      }

      match decoded {
        Ok(decoded) => match create_hicon(&decoded) {
          Ok(icon) => {
            state.current_icon = Some(icon);
            state.current_icon_key = Some(icon_key);
            if let Err(err) = add_or_update_icon(hwnd, state, false) {
              error!("failed to apply decoded icon: {err}");
            }
          }
          Err(err) => {
            error!("failed to create icon handle: {err}");
          }
        },
        Err(err) => {
          error!("failed to decode tray icon: {err}");
        }
      }
      true
    }
    BackendCommand::Shutdown => false,
  }
}

fn schedule_icon_decode(state: &mut TrayWindowState, tray: Tray) {
  if let Some(bytes) = tray.icon_bytes {
    state.requested_icon_revision = state.requested_icon_revision.saturating_add(1);
    let revision = state.requested_icon_revision;
    let icon_key = icon_key(&bytes);

    if state.current_icon_key == Some(icon_key) && state.current_icon.is_some() {
      return;
    }

    let tx = state.command_tx.clone();
    thread::spawn(move || {
      let decoded = decode_icon(&bytes);
      let _ = tx.send(BackendCommand::IconDecoded {
        revision,
        icon_key,
        decoded,
      });
    });
  } else {
    state.requested_icon_revision = state.requested_icon_revision.saturating_add(1);
    state.current_icon_key = None;
  }
}

fn apply_tray_snapshot(hwnd: HWND, state: &mut TrayWindowState, tray: Tray) -> Result<()> {
  state.current_tray = Some(tray.clone());
  state.clear_menu();

  if !tray.visible {
    remove_tray_icon(hwnd, state);
    state.current_icon = None;
    state.current_icon_key = None;
    return Ok(());
  }

  if tray.icon_bytes.is_none() {
    state.current_icon = None;
    state.current_icon_key = None;
  }

  add_or_update_icon(hwnd, state, false)?;
  Ok(())
}

fn add_or_update_icon(hwnd: HWND, state: &mut TrayWindowState, force_add: bool) -> Result<()> {
  let Some(tray) = state.current_tray.as_ref() else {
    return Err(Error::NotFound);
  };

  let mut tip = [0u16; 128];
  if let Some(tooltip) = &tray.tooltip {
    for (index, ch) in encode_wide(tooltip.as_ref())
      .into_iter()
      .take(127)
      .enumerate()
    {
      tip[index] = ch;
    }
  }

  let hicon = state
    .current_icon
    .as_ref()
    .map(|icon| icon.0)
    .unwrap_or_default();
  let flags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
  let nid = NOTIFYICONDATAW {
    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
    hWnd: hwnd,
    uID: TRAY_ID,
    uFlags: flags,
    uCallbackMessage: WM_TRAYICON,
    hIcon: hicon,
    szTip: tip,
    ..unsafe { std::mem::zeroed() }
  };

  let op = if force_add || !state.registered {
    NIM_ADD
  } else {
    NIM_MODIFY
  };

  let result = unsafe { Shell_NotifyIconW(op, &nid) };
  if result != TRUE {
    return Err(
      BackendError::platform("Shell_NotifyIconW", format!("operation {op:?} failed")).into(),
    );
  }

  state.registered = true;
  Ok(())
}

fn remove_tray_icon(hwnd: HWND, state: &mut TrayWindowState) {
  if !state.registered {
    return;
  }

  let nid = NOTIFYICONDATAW {
    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
    hWnd: hwnd,
    uID: TRAY_ID,
    ..unsafe { std::mem::zeroed() }
  };
  let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
  state.registered = false;
}

fn cleanup(hwnd: HWND, state: &mut TrayWindowState) {
  remove_tray_icon(hwnd, state);
  state.current_icon = None;
  state.clear_menu();

  unsafe {
    let _ = DestroyWindow(hwnd);
  }
}

unsafe extern "system" fn window_proc(
  hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
  if msg == WM_NCCREATE {
    let create =
      unsafe { &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW) };
    let ptr = create.lpCreateParams as *mut TrayWindowState;
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize) };
    return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
  }

  let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut TrayWindowState;
  if ptr.is_null() {
    return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
  }

  let state = unsafe { &mut *ptr };

  match msg {
    WM_TRAYICON => {
      let event = lparam.0 as u32;
      match event {
        WM_LBUTTONUP => dispatch_click(state, TrayMouseButton::Left),
        WM_MBUTTONUP => dispatch_click(state, TrayMouseButton::Middle),
        WM_RBUTTONUP => {
          dispatch_click(state, TrayMouseButton::Right);
          show_context_menu(hwnd, state);
        }
        WM_LBUTTONDBLCLK => {
          let _ = state.event_tx.send(TrayEvent::DoubleClick);
        }
        _ => {}
      }
      return LRESULT(0);
    }
    WM_COMMAND => {
      let action_id = (wparam.0 & 0xFFFF) as u16;
      if let Some(id) = state.menu_ids.get(&action_id) {
        let _ = state
          .event_tx
          .send(TrayEvent::MenuClicked { id: id.clone() });
      }
      return LRESULT(0);
    }
    _ => {
      if msg == state.taskbar_restart_msg && state.current_tray.is_some() {
        let _ = add_or_update_icon(hwnd, state, true);
        return LRESULT(0);
      }
    }
  }

  unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn dispatch_click(state: &TrayWindowState, button: TrayMouseButton) {
  let mut pos = POINT::default();
  let _ = unsafe { GetCursorPos(&mut pos) };
  let _ = state.event_tx.send(TrayEvent::Click {
    button,
    position: (pos.x as f32, pos.y as f32),
  });
}

fn show_context_menu(hwnd: HWND, state: &mut TrayWindowState) {
  let Some(tray) = state.current_tray.as_ref() else {
    return;
  };

  if tray.menu.is_empty() {
    return;
  }

  let mut next_id: u16 = 0;
  let mut menu_ids = HashMap::new();
  let Some(menu) = build_menu(&tray.menu, &mut next_id, &mut menu_ids) else {
    return;
  };

  state.current_menu = Some(OwnedMenu(menu));
  state.menu_ids = menu_ids;

  let mut cursor = POINT::default();
  let _ = unsafe { GetCursorPos(&mut cursor) };
  unsafe {
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(
      menu,
      TPM_BOTTOMALIGN | TPM_LEFTALIGN,
      cursor.x,
      cursor.y,
      Some(0),
      hwnd,
      None,
    );
    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
  }
}

fn build_menu(
  items: &[TrayMenuItem], next_id: &mut u16, menu_ids: &mut HashMap<u16, String>,
) -> Option<HMENU> {
  let menu = unsafe { CreatePopupMenu().ok()? };

  for item in items {
    match item {
      TrayMenuItem::Separator => unsafe {
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
      },
      TrayMenuItem::Action { id, label, .. } => {
        *next_id = next_id.saturating_add(1);
        let menu_id = *next_id;
        let wide = encode_wide(label.as_str());
        let result =
          unsafe { AppendMenuW(menu, MF_STRING, menu_id as usize, PCWSTR(wide.as_ptr())) };
        if result.is_ok() {
          menu_ids.insert(menu_id, id.clone());
        }
      }
      TrayMenuItem::Submenu { label, items } => {
        if let Some(sub) = build_menu(items, next_id, menu_ids) {
          let wide = encode_wide(label.as_str());
          let _ = unsafe { AppendMenuW(menu, MF_POPUP, sub.0 as usize, PCWSTR(wide.as_ptr())) };
        }
      }
    }
  }

  Some(menu)
}

fn encode_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
  s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

fn icon_key(bytes: &[u8]) -> u64 {
  let mut hasher = DefaultHasher::new();
  bytes.hash(&mut hasher);
  hasher.finish()
}

// ---------------------------------------------------------------------------
// Icon decode (ported from gpui-tray, MPL-2.0)
// ---------------------------------------------------------------------------

struct DecodedIcon {
  rgba: Vec<u8>,
  width: u32,
  height: u32,
}

struct OwnedIcon(HICON);

impl Drop for OwnedIcon {
  fn drop(&mut self) {
    if !self.0.is_invalid() {
      unsafe {
        let _ = DestroyIcon(self.0);
      }
    }
  }
}

fn decode_icon(bytes: &[u8]) -> Result<DecodedIcon> {
  let decoded = image::load_from_memory(bytes).map_err(|_| Error::InvalidIcon)?;
  let resized = decoded.resize_to_fill(32, 32, image::imageops::FilterType::Lanczos3);
  let rgba = resized.to_rgba8().into_raw();
  Ok(DecodedIcon {
    rgba,
    width: 32,
    height: 32,
  })
}

fn create_hicon(decoded: &DecodedIcon) -> Result<OwnedIcon> {
  use windows::{
    Win32::Graphics::Gdi::{
      BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
      GetDC, ReleaseDC,
    },
    core::BOOL,
  };

  unsafe {
    let hdc = GetDC(None);
    if hdc.is_invalid() {
      return Err(BackendError::platform("GetDC", "invalid device context").into());
    }

    let bmi = BITMAPINFO {
      bmiHeader: BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: decoded.width as i32,
        biHeight: -(decoded.height as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
      },
      bmiColors: [Default::default(); 1],
    };

    let mut bits: *mut u8 = std::ptr::null_mut();
    let hbitmap = CreateDIBSection(
      Some(hdc),
      &bmi,
      DIB_RGB_COLORS,
      &mut bits as *mut _ as *mut *mut std::ffi::c_void,
      None,
      0,
    )
    .map_err(|err| BackendError::platform("CreateDIBSection", format!("{err:?}")))?;

    let bgra: Vec<u8> = decoded
      .rgba
      .chunks_exact(4)
      .flat_map(|chunk| [chunk[2], chunk[1], chunk[0], chunk[3]])
      .collect();
    std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits, bgra.len());

    let _ = ReleaseDC(None, hdc);

    let mut and_mask = vec![0xFFu8; (decoded.width.div_ceil(8) * decoded.height) as usize];
    for (i, chunk) in decoded.rgba.chunks_exact(4).enumerate() {
      let alpha = chunk[3];
      if alpha < 128 {
        let x = (i % decoded.width as usize) as u32;
        let y = (i / decoded.width as usize) as u32;
        let byte_index = (y * decoded.width.div_ceil(8) + (x / 8)) as usize;
        let bit_index = x % 8;
        and_mask[byte_index] &= !(1 << (7 - bit_index));
      }
    }

    let hmask = CreateBitmap(
      decoded.width as i32,
      decoded.height as i32,
      1,
      1,
      Some(and_mask.as_ptr() as *const _),
    );

    if hmask.is_invalid() {
      let _ = DeleteObject(hbitmap.into());
      return Err(BackendError::platform("CreateBitmap", "failed to create mask bitmap").into());
    }

    let icon_info = ICONINFO {
      fIcon: BOOL(1),
      xHotspot: 0,
      yHotspot: 0,
      hbmMask: hmask,
      hbmColor: hbitmap,
    };

    let hicon = CreateIconIndirect(&icon_info)
      .map_err(|err| BackendError::platform("CreateIconIndirect", format!("{err:?}")))?;

    let _ = DeleteObject(hbitmap.into());
    let _ = DeleteObject(hmask.into());

    if hicon.is_invalid() {
      return Err(Error::InvalidIcon);
    }

    Ok(OwnedIcon(hicon))
  }
}
