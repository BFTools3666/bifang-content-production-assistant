; Restore the approved product name without leaving a desktop shortcut to the
; earlier SEO-named binary. Archive only shortcuts whose targets match that
; registered installation; keep its executable and all application data intact.
!if "${PRODUCTNAME}" == "毕方内容生产助手"
Var BifangLegacyInstallDir

!macro BIFANG_ARCHIVE_LEGACY_SHORTCUT shortcut backupName
  !insertmacro IsShortcutTarget "${shortcut}" "$BifangLegacyInstallDir\${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 1
    CreateDirectory "$LOCALAPPDATA\BifangBackups\shortcuts\${VERSION}"
    ClearErrors
    Rename "${shortcut}" "$LOCALAPPDATA\BifangBackups\shortcuts\${VERSION}\${backupName}.lnk"
    ${If} ${Errors}
      DetailPrint "旧版快捷方式未能移入备份目录，已保留原文件。"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ReadRegStr $R0 SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\毕方 SEO 文章助手" "Publisher"
  ${If} $R0 == "${MANUFACTURER}"
    ReadRegStr $BifangLegacyInstallDir SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\毕方 SEO 文章助手" "InstallLocation"
    StrCpy $R1 $BifangLegacyInstallDir 1
    ${If} $R1 == '$\"'
      StrCpy $BifangLegacyInstallDir $BifangLegacyInstallDir -1 1
    ${EndIf}
    ${If} $BifangLegacyInstallDir != ""
      !insertmacro BIFANG_ARCHIVE_LEGACY_SHORTCUT "$DESKTOP\毕方 SEO 文章助手.lnk" "legacy-desktop"
      !insertmacro BIFANG_ARCHIVE_LEGACY_SHORTCUT "$SMPROGRAMS\毕方 SEO 文章助手.lnk" "legacy-start-menu"
    ${EndIf}
  ${EndIf}
!macroend
!endif
