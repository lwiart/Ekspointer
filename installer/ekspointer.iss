; Programme d'installation d'Ekspointer (Inno Setup 6).
; Construit par scripts\build-release.ps1, qui fournit AppVersion et ExePath.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef ExePath
  #define ExePath "..\target\release\ekspointer.exe"
#endif

[Setup]
; La langue de l'assistant suit celle de Windows (français, sinon anglais), sans poser la question.
ShowLanguageDialog=no
; Ne jamais changer AppId : c'est lui qui permet les mises à jour.
AppId={{D98EDD00-716E-497D-A73D-636807B092EF}
AppName=Ekspointer
AppVersion={#AppVersion}
AppVerName=Ekspointer {#AppVersion}
AppPublisher=Eksplodo
AppPublisherURL=https://eksplodo.fr/
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Ekspointer
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
DisableDirPage=yes
DisableWelcomePage=no
CloseApplications=no
SetupIconFile=..\assets\ekspointer.ico
WizardSmallImageFile=..\assets\wizard-small.bmp
WizardImageFile=..\assets\wizard-large.bmp
UninstallDisplayIcon={app}\ekspointer.exe
UninstallDisplayName=Ekspointer
OutputDir=..\dist
OutputBaseFilename=Ekspointer-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
french.DesktopIcon=Créer un raccourci sur le Bureau
english.DesktopIcon=Create a desktop shortcut
french.AutoStart=Lancer Ekspointer au démarrage de Windows
english.AutoStart=Start Ekspointer with Windows
french.LaunchApp=Lancer Ekspointer
english.LaunchApp=Launch Ekspointer
french.DeleteSettings=Voulez-vous aussi supprimer vos réglages Ekspointer ?
english.DeleteSettings=Do you also want to delete your Ekspointer settings?

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked
Name: "autostart"; Description: "{cm:AutoStart}"; Flags: unchecked

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; DestName: "ekspointer.exe"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Ekspointer"; Filename: "{app}\ekspointer.exe"
Name: "{userdesktop}\Ekspointer"; Filename: "{app}\ekspointer.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Ekspointer"; ValueData: """{app}\ekspointer.exe"""; Tasks: autostart

[Run]
Filename: "{app}\ekspointer.exe"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent

[Code]
// Demande à l'instance en cours de se fermer et attend jusqu'à 3 secondes qu'elle l'ait fait.
procedure QuitRunning(const Dir: String);
var
  ResultCode, I: Integer;
begin
  if not FileExists(Dir + '\ekspointer.exe') then
    Exit;
  Exec(Dir + '\ekspointer.exe', '--quit', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  for I := 1 to 30 do
  begin
    if not CheckForMutexes('Ekspointer.Instance') then
      Break;
    Sleep(100);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  QuitRunning(ExpandConstant('{app}'));
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  QuitRunning(ExpandConstant('{app}'));
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Dir: String;
begin
  if CurUninstallStep <> usPostUninstall then
    Exit;
  // Le lancement au démarrage a pu être activé depuis les réglages de l'application.
  RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Ekspointer');
  Dir := ExpandConstant('{app}');
  if FileExists(Dir + '\config.json') or FileExists(Dir + '\config.invalide.json') or FileExists(Dir + '\ekspointer.log') then
  begin
    if SuppressibleMsgBox(CustomMessage('DeleteSettings'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) = IDYES then
    begin
      DeleteFile(Dir + '\config.json');
      DeleteFile(Dir + '\config.invalide.json');
      DeleteFile(Dir + '\ekspointer.log');
      RemoveDir(Dir);
    end;
  end;
end;
