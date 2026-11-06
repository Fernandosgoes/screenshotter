//! Fontes da marca embutidas no .exe (Bricolage Grotesque, Poppins e Orbitron, todas SIL OFL).
//! Ficam registradas só para este processo: não instalam nada no Windows, e os textos saem
//! iguais em outros computadores.

use std::ffi::c_void;
use windows::Win32::Graphics::Gdi::AddFontMemResourceEx;

const FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/BricolageGrotesque-Variable.ttf"),
    include_bytes!("../assets/fonts/Poppins-Regular.ttf"),
    include_bytes!("../assets/fonts/Poppins-Bold.ttf"),
    include_bytes!("../assets/fonts/Orbitron-Variable.ttf"),
];

/// Registra as fontes embutidas. Chamar uma vez, antes de criar qualquer fonte.
pub fn load() {
    for data in FONTS {
        let mut n = 0u32;
        unsafe {
            AddFontMemResourceEx(data.as_ptr() as *const c_void, data.len() as u32, None, &mut n);
        }
    }
}
