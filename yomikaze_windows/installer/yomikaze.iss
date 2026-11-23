; Yomikaze installer (Inno Setup 6).
; build with: ISCC.exe installer\yomikaze.iss (after cargo build --release)
; installs per user into %LOCALAPPDATA%\Programs\Yomikaze, no admin rights needed.
; user data lives in %APPDATA%\Yomikaze and is never touched by the installer.

#define AppName "Yomikaze"
#define AppVersion "0.2.7"
; shown in the installer and in installed apps, the file name and file version use AppVersion
#define AppVersionName "0.2.7 hotfix"
#define AppPublisher "Kairo"
#define AppExe "Yomikaze.exe"

[Setup]
; fixed id so a newer installer updates an existing install in place
AppId={{4ACB0501-91E8-479F-904D-9F8F96F7E35B}
AppName={#AppName}
AppVersion={#AppVersionName}
AppVerName={#AppName} {#AppVersionName}
VersionInfoVersion={#AppVersion}.1
AppPublisher={#AppPublisher}
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\..\yomikaze_windows_build
OutputBaseFilename=Yomikaze-Setup-{#AppVersion}-hotfix
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; close a running yomikaze first, it saves its data on exit
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "hungarian"; MessagesFile: "compiler:Languages\Hungarian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\target\release\yomikaze.exe"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "Kairo.Yomikaze"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon; AppUserModelID: "Kairo.Yomikaze"

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Messages]
english.FinishedLabel=Yomikaze is installed. Your library and reading progress are kept in their own folder (%APPDATA%\Yomikaze), so they stay even if you reinstall or uninstall.
hungarian.FinishedLabel=A Yomikaze telepítve. A könyvtárad és az olvasási állásod külön mappában van (%APPDATA%\Yomikaze), így újratelepítés vagy eltávolítás után is megmaradnak.
