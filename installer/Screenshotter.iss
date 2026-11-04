; Instalador do Screenshotter (Inno Setup 6).
; Compilar: ISCC.exe installer\Screenshotter.iss   (gera dist\Screenshotter-Setup.exe)
; Pré-requisito: cargo build --release  (gera target\release\Screenshotter.exe)

#define MyAppName "Screenshotter"
#define MyAppVersion "1.0.0"
#define MyAppExe "Screenshotter.exe"

[Setup]
AppId={{582E6E4B-C079-4615-9847-7A4BF289D72D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher=Fernando Goes
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
; Instala só para o usuário: sem UAC, e o settings.json fica gravável ao lado do .exe.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=Screenshotter-Setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
UninstallDisplayName={#MyAppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
; Fecha o app antes de atualizar/desinstalar (usa o mutex de instância única do app).
AppMutex=Screenshotter.SingleInstance
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=auto

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "startwithwindows"; Description: "Iniciar o {#MyAppName} com o Windows"; GroupDescription: "Opções:"; Flags: unchecked
Name: "desktopicon"; Description: "Criar atalho na área de trabalho"; GroupDescription: "Opções:"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}"; DestName: "Screenshotter.ico"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; IconFilename: "{app}\Screenshotter.ico"
Name: "{group}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; IconFilename: "{app}\Screenshotter.ico"; Tasks: desktopicon

[Registry]
; Iniciar com o Windows (só se a tarefa estiver marcada). Removido na desinstalação.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Screenshotter"; ValueData: """{app}\{#MyAppExe}"""; Flags: uninsdeletevalue; Tasks: startwithwindows

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Iniciar o {#MyAppName} agora"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "taskkill.exe"; Parameters: "/IM {#MyAppExe} /F"; Flags: runhidden; RunOnceId: "StopScreenshotter"

[UninstallDelete]
Type: files; Name: "{app}\settings.json"
