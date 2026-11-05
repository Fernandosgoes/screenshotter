//! Screenshotter: capturador de tela para Windows, sem rede.
//! Ponto de entrada: DPI por monitor, instância única, configurações, janela principal
//! oculta, bandeja, atalho global e loop de mensagens.

// Sem janela de console: o app roda só na bandeja.
#![windows_subsystem = "windows"]

mod capture;
mod config;
mod draw;
mod editor;
mod export;
mod geom;
mod gfx;
mod hotkey;
mod log;
mod model;
mod overlay;
mod pixelate;
mod settings;
mod startup;
mod toolbar;
mod tray;

use config::Settings;
use std::path::PathBuf;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MB_ICONINFORMATION, MB_OK, MSG,
    MessageBoxW, PM_NOREMOVE, PeekMessageW, PostQuitMessage, RegisterClassW,
    TranslateMessage, WM_COMMAND, WM_DESTROY, WM_HOTKEY, WM_RBUTTONUP, WNDCLASSW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const MAIN_CLASS: PCWSTR = w!("ScreenshotterMain");
const APP_NAME: PCWSTR = w!("Screenshotter");

pub(crate) fn settings_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("settings.json")))
        .unwrap_or_else(|| PathBuf::from("settings.json"))
}

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            // Configurações são relidas a cada captura: alterações valem na próxima tela.
            let settings = Settings::load(&settings_path());
            let _ = overlay::start_capture(&settings);
            LRESULT(0)
        }
        m if m == tray::WM_APP_TRAY => {
            if (lp.0 as u32) == WM_RBUTTONUP {
                tray::show_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wp.0 & 0xFFFF {
                id if id == tray::ID_MENU_EXIT => unsafe { PostQuitMessage(0) },
                id if id == tray::ID_MENU_SETTINGS => settings::open(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn message(text: &str) {
    let wtext: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(None, PCWSTR(wtext.as_ptr()), APP_NAME, MB_OK | MB_ICONINFORMATION);
    }
}

fn main() {
    // Avisa o Windows já na primeira linha que o app tem fila de mensagens: isso encerra o
    // cursor de "aplicativo iniciando" (seta com bolinha girando) em vez de esperá-lo estourar.
    unsafe {
        let mut m = MSG::default();
        let _ = PeekMessageW(&mut m, None, 0, 0, PM_NOREMOVE);
    }

    log::set_debug(std::env::args().any(|a| a == "--debug"));

    // Instância única: se já há uma rodando, a segunda sai sem fazer nada.
    let _single = unsafe {
        let h = CreateMutexW(None, true, w!("Screenshotter.SingleInstance"));
        match h {
            Ok(h) if GetLastError() != ERROR_ALREADY_EXISTS => Some(h),
            _ => return,
        }
    };

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // Desenho suavizado das barras de ferramentas.
    gfx::startup();

    // Se o settings.json existe, ele manda em "iniciar com o Windows". Roda em segundo plano:
    // chama o reg.exe, que pode demorar na primeira execução, e não deve atrasar a bandeja.
    let settings = Settings::load(&settings_path());
    if settings_path().exists() {
        let want = settings.start_with_windows;
        std::thread::spawn(move || {
            let _ = startup::set_enabled(want);
        });
    }

    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(main_proc),
            hInstance: hinst.into(),
            lpszClassName: MAIN_CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = match CreateWindowExW(
            Default::default(),
            MAIN_CLASS,
            APP_NAME,
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => {
                message("Não foi possível criar a janela principal.");
                return;
            }
        };

        if overlay::create().is_err() {
            message("Não foi possível criar a janela de captura.");
            return;
        }
        let _ = tray::add(hwnd);

        let (_chosen, warn) = hotkey::register(hwnd, settings.hotkey);
        if let Some(w) = warn {
            message(w);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
