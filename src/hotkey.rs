//! Atalho global. Padrão: Shift+Backspace. Se o Windows recusar o registro (outro programa
//! já usa a combinação), tenta a alternativa PrtScn e informa o motivo.

use crate::config::HotkeyChoice;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey, VK_BACK,
    VK_SNAPSHOT,
};

pub const ID_HOTKEY: i32 = 1;

pub const WARN_FALLBACK: &str = "A combinação escolhida já está em uso por outro programa.\n\
Usando PrtScn. Altere o atalho em settings.json (ao lado do .exe).";

/// (modificadores, tecla virtual) de cada escolha.
fn keys(choice: HotkeyChoice) -> (HOT_KEY_MODIFIERS, u32) {
    match choice {
        HotkeyChoice::ShiftBackspace => (MOD_NOREPEAT | MOD_SHIFT, VK_BACK.0 as u32),
        HotkeyChoice::PrtScn => (MOD_NOREPEAT, VK_SNAPSHOT.0 as u32),
        HotkeyChoice::ShiftPrtScn => (MOD_NOREPEAT | MOD_SHIFT, VK_SNAPSHOT.0 as u32),
    }
}

fn try_register(hwnd: HWND, choice: HotkeyChoice) -> bool {
    let (mods, vk) = keys(choice);
    unsafe { RegisterHotKey(Some(hwnd), ID_HOTKEY, mods, vk).is_ok() }
}

/// Retorna a tecla efetivamente registrada e um aviso opcional se houve fallback.
pub fn register(hwnd: HWND, choice: HotkeyChoice) -> (HotkeyChoice, Option<&'static str>) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), ID_HOTKEY);
    }
    if try_register(hwnd, choice) {
        return (choice, None);
    }
    if choice != HotkeyChoice::PrtScn && try_register(hwnd, HotkeyChoice::PrtScn) {
        return (HotkeyChoice::PrtScn, Some(WARN_FALLBACK));
    }
    (choice, Some("Não foi possível registrar o atalho global."))
}
