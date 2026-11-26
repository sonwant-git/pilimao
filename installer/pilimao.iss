; Pilimao One-Click Windows Installer (Inno Setup 6)
; Per-user install by default (no UAC admin prompt required), with optional file associations.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#ifndef MyBinaryPath
  #define MyBinaryPath "..\target\x86_64-pc-windows-msvc\release\pilimao.exe"
#endif

#ifndef MyOutputDir
  #define MyOutputDir "..\dist"
#endif

#ifndef MyOutputBaseFilename
  #define MyOutputBaseFilename "pilimao-" + MyAppVersion + "-windows-x86_64-setup"
#endif

[Setup]
AppId={{D4A9E62B-8C74-4F62-9B18-5A3D17F2C4E0}
AppName=Pilimao
AppVersion={#MyAppVersion}
AppVerName=Pilimao {#MyAppVersion}
AppPublisher=sonwant-git
AppPublisherURL=https://github.com/sonwant-git/pilimao
AppSupportURL=https://github.com/sonwant-git/pilimao/issues
AppUpdatesURL=https://github.com/sonwant-git/pilimao/releases
DefaultDirName={autopf}\Pilimao
DefaultGroupName=Pilimao
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir={#MyOutputDir}
OutputBaseFilename={#MyOutputBaseFilename}
SetupIconFile=..\assets\pilimao.ico
UninstallDisplayIcon={app}\pilimao.exe
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assoc_audio"; Description: "Associate audio & playlist files (.mp3, .flac, .wav, .ogg, .m4a, .m3u8, .pls)"; GroupDescription: "File Associations:"
Name: "assoc_winamp"; Description: "Associate Winamp skins & EQ presets (.wsz, .eqf)"; GroupDescription: "File Associations:"

[Files]
Source: "{#MyBinaryPath}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\pilimao.ico"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
Name: "{app}\skins"

[Icons]
Name: "{group}\Pilimao"; Filename: "{app}\pilimao.exe"; IconFilename: "{app}\pilimao.ico"
Name: "{group}\{cm:UninstallProgram,Pilimao}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Pilimao"; Filename: "{app}\pilimao.exe"; IconFilename: "{app}\pilimao.ico"; Tasks: desktopicon

[Registry]
; ProgID definitions under HKCU/HKLM (via HKA, automatically matches per-user or per-machine mode)
Root: HKA; Subkey: "Software\Classes\Pilimao.Audio"; ValueType: string; ValueName: ""; ValueData: "Pilimao Audio File"; Flags: uninsdeletekey; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\Pilimao.Audio\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\pilimao.exe,0"; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\Pilimao.Audio\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\pilimao.exe"" ""%1"""; Tasks: assoc_audio

Root: HKA; Subkey: "Software\Classes\.mp3\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.flac\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.wav\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.ogg\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.m4a\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.aac\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.m3u\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.m3u8\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio
Root: HKA; Subkey: "Software\Classes\.pls\OpenWithProgids"; ValueType: string; ValueName: "Pilimao.Audio"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc_audio

Root: HKA; Subkey: "Software\Classes\Pilimao.Skin"; ValueType: string; ValueName: ""; ValueData: "Winamp Classic Skin (.wsz)"; Flags: uninsdeletekey; Tasks: assoc_winamp
Root: HKA; Subkey: "Software\Classes\Pilimao.Skin\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\pilimao.exe,0"; Tasks: assoc_winamp
Root: HKA; Subkey: "Software\Classes\Pilimao.Skin\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\pilimao.exe"" ""%1"""; Tasks: assoc_winamp
Root: HKA; Subkey: "Software\Classes\.wsz"; ValueType: string; ValueName: ""; ValueData: "Pilimao.Skin"; Flags: uninsdeletevalue; Tasks: assoc_winamp

Root: HKA; Subkey: "Software\Classes\Pilimao.EqPreset"; ValueType: string; ValueName: ""; ValueData: "Winamp Equalizer Preset (.eqf)"; Flags: uninsdeletekey; Tasks: assoc_winamp
Root: HKA; Subkey: "Software\Classes\Pilimao.EqPreset\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\pilimao.exe,0"; Tasks: assoc_winamp
Root: HKA; Subkey: "Software\Classes\Pilimao.EqPreset\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\pilimao.exe"" ""%1"""; Tasks: assoc_winamp
Root: HKA; Subkey: "Software\Classes\.eqf"; ValueType: string; ValueName: ""; ValueData: "Pilimao.EqPreset"; Flags: uninsdeletevalue; Tasks: assoc_winamp

[Run]
Filename: "{app}\pilimao.exe"; Description: "{cm:LaunchProgram,Pilimao}"; Flags: nowait postinstall skipifsilent
