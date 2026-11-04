//! Ícone na bandeja e menu de contexto.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, IDI_APPLICATION, LoadIconW,
    MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_RIGHTBUTTON,
    TrackPopupMenu, WM_APP, WM_NULL,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::{PCWSTR, w};

pub const WM_APP_TRAY: u32 = WM_APP + 1;
pub const ID_MENU_SETTINGS: usize = 1002;
pub const ID_MENU_EXIT: usize = 1001;
const TRAY_ID: u32 = 1;

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_APP_TRAY,
        ..Default::default()
    };
    let tip: Vec<u16> = "iCatcher".encode_utf16().collect();
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    nid
}

pub fn add(hwnd: HWND) -> windows::core::Result<()> {
    let mut nid = data(hwnd);
    // Ícone embutido no .exe pelo build.rs (recurso de ID 1); fallback para o padrão.
    nid.hIcon = unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        LoadIconW(Some(hinst.into()), PCWSTR(1usize as *const u16))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))?
    };
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid).ok() }
}

pub fn remove(hwnd: HWND) {
    let nid = data(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn show_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MF_STRING, ID_MENU_SETTINGS, w!("Configurações"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, w!("Sair"));
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Necessário para o menu fechar ao clicar fora.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, Default::default(), Default::default());
        let _ = DestroyMenu(menu);
    }
}
