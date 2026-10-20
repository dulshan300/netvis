; NetVis Windows installer (NSIS 3). Build with scripts/build-windows.sh,
; or: makensis -DVERSION=0.1.0 -DEXE=path\to\netvis.exe installer/netvis.nsi

Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef EXE
  !define EXE "..\target\x86_64-pc-windows-gnu\release\netvis.exe"
!endif
!ifndef OUTDIR
  !define OUTDIR "..\target\installer"
!endif

!define APP "NetVis"
!define PUBLISHER "WebPotato"
!define PROGID "NetVis.Project"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"

Name "${APP}"
OutFile "${OUTDIR}\NetVis-${VERSION}-setup.exe"
InstallDir "$PROGRAMFILES64\${APP}"
InstallDirRegKey HKLM "Software\${APP}" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "${APP} ${VERSION} · ${PUBLISHER}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "FileDescription" "${APP} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "© ${PUBLISHER}"

!define MUI_ICON "..\assets\netvis.ico"
!define MUI_UNICON "..\assets\netvis.ico"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_SMALLDESC
!define MUI_FINISHPAGE_RUN "$INSTDIR\netvis.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start ${APP}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${APP} needs 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

Section "${APP} (required)" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "/oname=netvis.exe" "${EXE}"
  File "/oname=THIRD-PARTY-FONTS.txt" "..\crates\netvis-report\fonts\LICENSE-DejaVu.txt"

  CreateDirectory "$SMPROGRAMS\${APP}"
  CreateShortcut "$SMPROGRAMS\${APP}\${APP}.lnk" "$INSTDIR\netvis.exe"
  CreateShortcut "$SMPROGRAMS\${APP}\Uninstall ${APP}.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKLM "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\netvis.exe,0"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Open .netvis files with ${APP}" SecAssoc
  WriteRegStr HKLM "Software\Classes\.netvis" "" "${PROGID}"
  WriteRegStr HKLM "Software\Classes\.netvis" "Content Type" "application/x-netvis"
  WriteRegStr HKLM "Software\Classes\${PROGID}" "" "NetVis project"
  WriteRegStr HKLM "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\netvis.exe,0"
  WriteRegStr HKLM "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\netvis.exe" "%1"'
  WriteRegStr HKLM "Software\Classes\Applications\netvis.exe\SupportedTypes" ".netvis" ""
  ; Tell Explorer the associations changed.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Sample projects" SecSamples
  SetOutPath "$INSTDIR\samples"
  File "..\samples\*.netvis"
  CreateShortcut "$SMPROGRAMS\${APP}\Sample projects.lnk" "$INSTDIR\samples"
SectionEnd

Section /o "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\netvis.exe"
SectionEnd

Section "-Size"
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "The ${APP} program and Start-menu shortcuts."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAssoc} "Double-click a .netvis project to open it in ${APP}."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecSamples} "Corescale HQ, Corescale Campus and the shopping-mall tutorial design."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "A ${APP} icon on the desktop."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$INSTDIR\netvis.exe"
  Delete "$INSTDIR\THIRD-PARTY-FONTS.txt"
  Delete "$INSTDIR\samples\corescale-hq.netvis"
  Delete "$INSTDIR\samples\corescale-campus.netvis"
  Delete "$INSTDIR\samples\shopping-mall.netvis"
  RMDir "$INSTDIR\samples"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP}\${APP}.lnk"
  Delete "$SMPROGRAMS\${APP}\Uninstall ${APP}.lnk"
  Delete "$SMPROGRAMS\${APP}\Sample projects.lnk"
  RMDir "$SMPROGRAMS\${APP}"
  Delete "$DESKTOP\${APP}.lnk"

  ; Only remove the association if it is still ours.
  ReadRegStr $0 HKLM "Software\Classes\.netvis" ""
  ${If} $0 == "${PROGID}"
    DeleteRegKey HKLM "Software\Classes\.netvis"
  ${EndIf}
  DeleteRegKey HKLM "Software\Classes\${PROGID}"
  DeleteRegKey HKLM "Software\Classes\Applications\netvis.exe"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "Software\${APP}"
  ; Settings in %APPDATA% are kept, as is anything the user saved here.
SectionEnd
