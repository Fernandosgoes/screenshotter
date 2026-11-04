//! Atalho global. PrtScn por padrão; se o Windows não liberar a tecla (Ferramenta de
//! Captura), cai para Shift+PrtScn e informa o motivo.

use crate::config::HotkeyChoice;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey, VK_SNAPSHOT,
};

pub const ID_HOTKEY: i32 = 1;

pub const WARN_PRTSCN: &str = "A tecla PrtScn está em uso pelo Windows (Ferramenta de Captura).\n\
Desative em Configurações › Acessibilidade › Teclado, ou use Shift+PrtScn.";

fn mods(choice: HotkeyChoice) -> HOT_KEY_MODIFIERS {
    match choice {
        HotkeyChoice::PrtScn => MOD_NOREPEAT,
        HotkeyChoice::ShiftPrtScn => MOD_NOREPEAT | MOD_SHIFT,
    }
}

/// Retorna a tecla efetivamente registrada e um aviso opcional se houve fallback.
pub fn register(hwnd: HWND, choice: HotkeyChoice) -> (HotkeyChoice, Option<&'static str>) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), ID_HOTKEY);
        if RegisterHotKey(Some(hwnd), ID_HOTKEY, mods(choice), VK_SNAPSHOT.0 as u32).is_ok() {
            return (choice, None);
        }
        if choice == HotkeyChoice::PrtScn
            && RegisterHotKey(Some(hwnd), ID_HOTKEY, mods(HotkeyChoice::ShiftPrtScn), VK_SNAPSHOT.0 as u32)
                .is_ok()
        {
            return (HotkeyChoice::ShiftPrtScn, Some(WARN_PRTSCN));
        }
        (choice, Some("Não foi possível registrar o atalho global."))
    }
}
