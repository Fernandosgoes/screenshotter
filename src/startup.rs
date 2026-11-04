//! Iniciar com o Windows: valor em HKCU\...\Run apontando para o .exe atual.
//! Usa reg.exe para manter o código pequeno; só mexe na chave do próprio usuário.

use std::process::Command;

const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "iCatcher";

/// Verdadeiro se o valor de inicialização automática existe.
pub fn is_enabled() -> bool {
    Command::new("reg")
        .args(["query", KEY, "/v", NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
