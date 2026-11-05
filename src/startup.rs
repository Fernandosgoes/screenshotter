//! Iniciar com o Windows: valor em HKCU\...\Run apontando para o .exe atual.
//! Usa reg.exe para manter o código pequeno; só mexe na chave do próprio usuário.

use std::process::Command;

const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "Screenshotter";

/// Verdadeiro se o valor de inicialização automática existe. Lê o registro direto
/// (rápido); chamar o reg.exe aqui atrasava a abertura das configurações.
pub fn is_enabled() -> bool {
    use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW};
    use windows::core::{HSTRING, w};
    unsafe {
        let mut key = HKEY::default();
        let sub = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub, None, KEY_READ, &mut key).is_err() {
            return false;
        }
        let found = RegQueryValueExW(key, &HSTRING::from(NAME), None, None, None, None).is_ok();
        let _ = RegCloseKey(key);
        found
    }
}

pub fn set_enabled(on: bool) -> std::io::Result<()> {
    if on {
        let exe = std::env::current_exe()?;
        let data = format!("\"{}\"", exe.display());
        let st = Command::new("reg")
            .args(["add", KEY, "/v", NAME, "/t", "REG_SZ", "/d", &data, "/f"])
            .status()?;
        if st.success() { Ok(()) } else { Err(std::io::Error::other("reg add falhou")) }
    } else {
        let _ = Command::new("reg").args(["delete", KEY, "/v", NAME, "/f"]).status()?;
        Ok(())
    }
}
