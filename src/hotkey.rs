//! Atalho global. Padrão: Shift+Backspace. Se o Windows recusar o registro (outro programa
//! já usa a combinação), tenta a alternativa PrtScn e informa o motivo.

use crate::config::{HotkeyChoice, Settings};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey, VK_BACK,
    VK_SNAPSHOT,
};

pub const ID_HOTKEY: i32 = 1;

pub const WARN_FALLBACK: &str = "A combinação escolhida já está em uso por outro programa.\n\
Usando PrtScn. Escolha outro atalho em Configurações (menu da bandeja).";

/// (modificadores, tecla virtual) da escolha atual.
fn keys(s: &Settings) -> (HOT_KEY_MODIFIERS, u32) {
    match s.hotkey {
        HotkeyChoice::ShiftBackspace => (MOD_NOREPEAT | MOD_SHIFT, VK_BACK.0 as u32),
        HotkeyChoice::PrtScn => (MOD_NOREPEAT, VK_SNAPSHOT.0 as u32),
        HotkeyChoice::ShiftPrtScn => (MOD_NOREPEAT | MOD_SHIFT, VK_SNAPSHOT.0 as u32),
        HotkeyChoice::Custom => match s.custom_hotkey {
            Some(h) => (HOT_KEY_MODIFIERS(h.mods) | MOD_NOREPEAT, h.vk),
            None => (MOD_NOREPEAT | MOD_SHIFT, VK_BACK.0 as u32),
        },
    }
}

fn try_register(hwnd: HWND, mods: HOT_KEY_MODIFIERS, vk: u32) -> bool {
    unsafe { RegisterHotKey(Some(hwnd), ID_HOTKEY, mods, vk).is_ok() }
}

/// Registra o atalho das configurações. Retorna a escolha efetivamente registrada e um
/// aviso opcional se houve fallback para PrtScn.
pub fn register(hwnd: HWND, s: &Settings) -> (HotkeyChoice, Option<&'static str>) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), ID_HOTKEY);
    }
    let (mods, vk) = keys(s);
    if try_register(hwnd, mods, vk) {
        return (s.hotkey, None);
    }
    if s.hotkey != HotkeyChoice::PrtScn && try_register(hwnd, MOD_NOREPEAT, VK_SNAPSHOT.0 as u32) {
        return (HotkeyChoice::PrtScn, Some(WARN_FALLBACK));
    }
    (s.hotkey, Some("Não foi possível registrar o atalho global."))
}
