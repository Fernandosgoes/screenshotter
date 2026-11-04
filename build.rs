// Embute o ícone no .exe (recurso de ID 1, usado pela bandeja em tray.rs).
//
// Chama o windres direto, a partir do OUT_DIR e com nomes relativos: caminhos com
// espaços (este projeto fica em "Projetos pessoais\NOvo lightshot") quebram o
// pré-processador quando o caminho completo é passado.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    std::fs::copy("assets/icon.ico", out.join("icon.ico")).expect("copiar icon.ico");
    std::fs::write(out.join("app.rc"), "1 ICON \"icon.ico\"\n").expect("escrever app.rc");

    let run = |prog: &str, args: &[&str]| {
        let st = Command::new(prog)
            .current_dir(&out)
            .args(args)
            .status()
            .unwrap_or_else(|e| panic!("não foi possível executar {prog}: {e}"));
        assert!(st.success(), "{prog} falhou com {st}");
    };

    // Objeto COFF com o ícone, empacotado numa biblioteca estática para o linker.
    run("windres", &["-i", "app.rc", "-O", "coff", "-o", "app.o"]);
    run("ar", &["rcs", "libicatcher_res.a", "app.o"]);
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=icatcher_res");
}
