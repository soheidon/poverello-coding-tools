; Poverello Coding Tools NSIS migration guards.
; This file is included by Tauri's generated installer before page and section
; definitions. Preflight is read-only until a consistent legacy install is
; fully validated.
!ifndef PCT_UNSTRCASE_DEFINED
!define PCT_UNSTRCASE_DEFINED
${UnStrCase}
!endif

Var PCTAttemptStarted
Var PCTRegistrationVerified
Var PCTRegistrationFailed
Var PCTCleanupDone
Var PCTHadExistingPoverello
Var PCTLegacyUpgradePending
Var PCTLegacyUpgradeComplete
Var PCTLegacyCleanupMarker
Var PCTLegacyCleanupOperationFailed
Var PCTLegacyCleanupReadbackFailed
Var PCTLegacyCleanupFailed
Var PCTLegacyCleanupKey
Var PCTLegacyCleanupStatus
Var PCTLegacyCleanupPhase
Var PCTLegacyHasRegistry
Var PCTLegacyOrphanExecutable
Var PCTLegacyValid
Var PCTNewHasRegistry
Var PCTNewValid
Var PCTNewRawPublisherPath
Var PCTPendingMarker
Var PCTPendingPath
Var PCTLegacyPath
Var PCTLegacyRawPath
Var PCTNewPath
Var PCTNewDisplayName
Var PCTNewPublisher
Var PCTNewVersion
Var PCTNewInstallLocation
Var PCTNewUninstallString
Var PCTNewDisplayIcon
Var PCTNewMainBinaryName
Var PCTLegacyDisplayName
Var PCTLegacyPublisher
Var PCTLegacyVersion
Var PCTLegacyInstallLocation
Var PCTLegacyUninstallString
Var PCTLegacyDisplayIcon
Var PCTLegacyDisplayIconExists
Var PCTLegacyMainBinaryName
Var PCTNormalizeInput
Var PCTNormalizeWork
Var PCTNormalizeOutput
Var PCTPathFirst
Var PCTPathLast
Var PCTPathLength
Var PCTPathIndex
Var PCTPathDrive
Var PCTPathColon
Var PCTPathSlash
Var PCTExpectedPath
Var PCTRequireCurrentVersion
Var PCTSavedPublisherPath
Var PCTSavedDisplayName
Var PCTSavedDisplayIcon
Var PCTSavedVersion
Var PCTSavedPublisher
Var PCTSavedInstallLocation
Var PCTSavedUninstallString
Var PCTSavedMainBinaryName
Var PCTSavedNoModify
Var PCTSavedNoRepair
Var PCTSavedEstimatedSize
Var PCTSavedNoModifyExists
Var PCTSavedNoRepairExists
Var PCTSavedEstimatedSizeExists
Var PCTRecoveryVerified
Var PCTRecoveryFailed
Var PCTRecoveryOperationFailed
Var PCTRecoveryAttempted
Var PCTCheckKey
Var PCTCheckValue
Var PCTCheckExpected
Var PCTCheckExpectedExists
Var PCTRegistryStringValue
Var PCTRegistryDwordValue
Var PCTRegistryStatus
Var PCTUninstallDataPath
Var PCTUninstallScanPath
Var PCTUninstallDataFailed
Var PCTUninstallReparseFound
Var PCTUninstallRegistrationReadFailed
Var PCTUninstallRegistrationOperationFailed
Var PCTUninstallRegistrationVerified
Var PCTUninstallSnapshotValid
Var PCTUninstallSnapshotPath
Var PCTUninstallRawValue
Var PCTUninstallKeyStatus
!macro PCT_UN_READ_REG_STR OUTPUT KEY VALUE
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Call un.PCTTestReadStateRegStr
  Pop ${OUTPUT}
!else
  ClearErrors
  ReadRegStr ${OUTPUT} SHCTX "${KEY}" "${VALUE}"
!endif
!macroend
!macro PCT_UN_READ_REG_DWORD OUTPUT KEY VALUE
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Call un.PCTTestReadStateRegStr
  Pop ${OUTPUT}
!else
  ClearErrors
  ReadRegDWORD ${OUTPUT} SHCTX "${KEY}" "${VALUE}"
!endif
!macroend
!macro PCT_UN_WRITE_REG_STR KEY VALUE DATA
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Push "${DATA}"
  Call un.PCTTestWriteStateRegStr
!else
  ClearErrors
  WriteRegStr SHCTX "${KEY}" "${VALUE}" "${DATA}"
!endif
!macroend
!macro PCT_UN_WRITE_REG_DWORD KEY VALUE DATA
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Push "${DATA}"
  Call un.PCTTestWriteStateRegStr
!else
  ClearErrors
  WriteRegDWORD SHCTX "${KEY}" "${VALUE}" "${DATA}"
!endif
!macroend
!macro PCT_UN_DELETE_REG_KEY KEY
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Call un.PCTTestDeleteStateRegKey
!else
  ClearErrors
  DeleteRegKey SHCTX "${KEY}"
!endif
!macroend
!macro PCT_READ_STATE_REG_STR OUTPUT KEY VALUE
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Call PCTTestReadStateRegStr
  Pop ${OUTPUT}
!else
  ReadRegStr ${OUTPUT} HKCU "${KEY}" "${VALUE}"
!endif
!macroend
!macro PCT_READ_STATE_REG_DWORD OUTPUT KEY VALUE
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Call PCTTestReadStateRegStr
  Pop ${OUTPUT}
!else
  ReadRegDWORD ${OUTPUT} HKCU "${KEY}" "${VALUE}"
!endif
!macroend
!macro PCT_DELETE_STATE_REG_VALUE KEY VALUE
!ifdef PCT_NSIS_TEST_BACKEND
  Push "${KEY}"
  Push "${VALUE}"
  Call PCTTestDeleteStateRegValue
!else
  DeleteRegValue HKCU "${KEY}" "${VALUE}"
!endif
!macroend
!ifndef PCT_NSIS_TEST_BACKEND
Var PCTUninstallDeleteRuntimeData
!endif
!ifdef PCT_NSIS_TEST_BACKEND
Var PCTTestFailOperation
Var PCTTestFailKey
Var PCTTestFailValue
Var PCTTestFailRegistrationDeleteKey
Var PCTFakePublisherKeyExists
Var PCTFakePublisherPath
Var PCTFakeUninstallKeyExists
Var PCTFakeDisplayName
Var PCTFakeDisplayIcon
Var PCTFakeDisplayVersion
Var PCTFakePublisher
Var PCTFakeInstallLocation
Var PCTFakeUninstallString
Var PCTFakeMainBinaryName
Var PCTFakeNoModifyExists
Var PCTFakeNoModify
Var PCTFakeNoRepairExists
Var PCTFakeNoRepair
Var PCTFakeEstimatedSizeExists
Var PCTFakeEstimatedSize
Var PCTFakeLegacyUninstallKeyExists
Var PCTFakeLegacyPublisherKeyExists
Var PCTFakeLegacyDisplayName
Var PCTFakeLegacyPublisher
Var PCTFakeLegacyVersion
Var PCTFakeLegacyInstallLocation
Var PCTFakeLegacyUninstallString
Var PCTFakeLegacyDisplayIcon
Var PCTFakeLegacyMainBinaryName
Var PCTFakeLegacyPublisherPath
Var PCTFakeLegacyCleanupMarker
Var PCTFakeLegacyCompleteMarker
Var PCTTestRegistryStatePath
Var PCTTestRegistryKey
Var PCTTestRegistryValue
Var PCTTestRegistrySection
Var PCTTestRegistryIniValue
Var PCTTestRegistryReadFailed
Var PCTTestLegacyOrphanPath
!endif

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri's default checkbox branch deletes bundle-ID-derived APPDATA and
  ; LOCALAPPDATA paths, which are not the runtime's current data roots. Keep
  ; its selection, but suppress that broad generated cleanup branch.
  StrCpy $PCTUninstallDeleteRuntimeData $DeleteAppDataCheckboxState
  StrCpy $DeleteAppDataCheckboxState 0
  StrCpy $PCTUninstallDataFailed 0
  StrCpy $PCTUninstallRegistrationReadFailed 0
  StrCpy $PCTUninstallRegistrationOperationFailed 0
  StrCpy $PCTUninstallRegistrationVerified 0
  StrCpy $PCTUninstallSnapshotValid 0
!ifdef PCT_NSIS_TEST_BACKEND
  ReadEnvStr $PCTTestRegistryStatePath "PCT_NSIS_TEST_REGISTRY_STATE"
  ReadEnvStr $PCTTestFailOperation "PCT_NSIS_TEST_FAIL_OPERATION"
  ReadEnvStr $PCTTestFailKey "PCT_NSIS_TEST_FAIL_KEY"
  ReadEnvStr $PCTTestFailValue "PCT_NSIS_TEST_FAIL_VALUE"
  ReadEnvStr $PCTTestFailRegistrationDeleteKey "PCT_NSIS_TEST_FAIL_REGISTRATION_DELETE_KEY"
!endif
  ${If} $UpdateMode == 1
    Call un.PCTSnapshotUpdateRegistration
    ${If} $PCTUninstallSnapshotValid != 1
      SetErrorLevel 1
      ${If} ${Silent}
        DetailPrint "Poverello update stopped before uninstall because the existing registration could not be snapshotted safely. No file rollback is claimed; repair the registration before retrying."
      ${Else}
        MessageBox MB_ICONSTOP|MB_OK "Poverello update stopped before uninstall because the existing registration could not be snapshotted safely. Repair the registration before retrying. No application files were intentionally removed."
      ${EndIf}
      Quit
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode == 1
    ; Tauri deletes ${UNINSTKEY} unconditionally before this hook, even for
    ; /UPDATE. Restore and verify the saved registration before the replacement
    ; installer resumes. Never turn this into a user-data deletion.
    Call un.PCTRestoreUpdateRegistration
    ${If} $PCTUninstallRegistrationVerified != 1
      ; The old application files have already been removed by Tauri. If the
      ; snapshot cannot be restored, try to remove both partial registration
      ; keys so the next run is not misclassified as a healthy install.
      Call un.PCTRemovePoverelloRegistration
!ifdef PCT_NSIS_TEST_BACKEND
      ReadEnvStr $8 "PCT_NSIS_TEST_RESULT_PATH"
      ${If} $8 != ""
        FileOpen $7 $8 a
        FileWrite $7 "status=failed|reason=update-registration-restore|fallback-cleanup-verified=$PCTUninstallRegistrationVerified|no-file-rollback-claimed=1$\n"
        FileClose $7
      ${EndIf}
!endif
      SetErrorLevel 1
      ${If} $PCTUninstallRegistrationVerified == 1
        ${If} ${Silent}
          DetailPrint "Poverello update could not restore its uninstall registration. The fallback cleanup was verified; application files may already be removed. Rerun the installer to recover. No file rollback is claimed."
        ${Else}
          MessageBox MB_ICONSTOP|MB_OK "Poverello update could not restore its uninstall registration. The fallback cleanup was verified; application files may already be removed. Rerun the installer to recover. No file rollback is claimed."
        ${EndIf}
      ${Else}
        ${If} ${Silent}
          DetailPrint "Poverello update could not restore its uninstall registration, and fallback registration cleanup could not be verified. Registration may remain and application files may already be removed. Repair the installation before retrying. No file rollback is claimed."
        ${Else}
          MessageBox MB_ICONSTOP|MB_OK "Poverello update could not restore its uninstall registration, and fallback registration cleanup could not be verified. Registration may remain and application files may already be removed. Repair the installation before retrying. No file rollback is claimed."
        ${EndIf}
      ${EndIf}
      Quit
    ${EndIf}
  ${Else}
    ; The checkbox gates only the stable runtime data root. Registration must
    ; be removed on every completed uninstall so a later clean install works.
    ${If} $PCTUninstallDeleteRuntimeData == 1
      Call un.PCTDeleteStableUserData
    ${EndIf}
    Call un.PCTRemovePoverelloRegistration
    ${If} $PCTUninstallRegistrationVerified != 1
      StrCpy $PCTUninstallRegistrationOperationFailed 1
    ${EndIf}
    ${If} $PCTUninstallDataFailed == 1
      Call un.PCTReportDataCleanupFailure
    ${ElseIf} $PCTUninstallRegistrationOperationFailed == 1
!ifdef PCT_NSIS_TEST_BACKEND
      ReadEnvStr $8 "PCT_NSIS_TEST_RESULT_PATH"
      ${If} $8 != ""
        FileOpen $7 $8 a
        FileWrite $7 "status=failed|reason=registration-cleanup|registration-verified=$PCTUninstallRegistrationVerified|no-file-rollback-claimed=1$\n"
        FileClose $7
      ${EndIf}
!endif
      SetErrorLevel 1
      ${If} ${Silent}
        DetailPrint "Poverello was removed, but its registry registration could not be fully removed or verified. The data checkbox does not imply that all application files were rolled back. Repair the registration before reinstalling."
      ${Else}
        MessageBox MB_ICONSTOP|MB_OK "Poverello was removed, but its registry registration could not be fully removed or verified. Repair the registration before reinstalling. No application-file rollback is claimed."
      ${EndIf}
      Quit
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Persist the installer-selected language before any terminal failure path.
  ${If} $language == "1041"
    StrCpy $0 "ja"
  ${ElseIf} $language == "2052"
    StrCpy $0 "zh-CN"
  ${ElseIf} $language == "1028"
    StrCpy $0 "zh-TW"
  ${ElseIf} $language == "1042"
    StrCpy $0 "ko"
  ${ElseIf} $language == "1036"
    StrCpy $0 "fr"
  ${ElseIf} $language == "1031"
    StrCpy $0 "de"
  ${ElseIf} $language == "1034"
    StrCpy $0 "es"
  ${Else}
    StrCpy $0 "en"
  ${EndIf}
  CreateDirectory "$INSTDIR\resources"
  FileOpen $1 "$INSTDIR\resources\installer_lang.txt" w
  FileWrite $1 $0
  FileClose $1

  ; Tauri writes these registration values in Section Install. Do not retire
  ; the previous product's registration or shortcuts until this check passes.
  StrCpy $PCTRequireCurrentVersion 1
  Call PCTValidatePoverelloRegistration
  ${If} $PCTNewValid != 1
    StrCpy $PCTRegistrationFailed 1
    Call PCTRestoreRegistrationOnFailure
    SetErrorLevel 1
    ${If} ${Silent}
      ${If} $PCTLegacyUpgradePending == 1
        ${If} $PCTRecoveryVerified == 1
          DetailPrint "Poverello registration verification failed. Registration recovery was verified; files may be partially installed and rerunning this installer is safe. The previous Anthro Bridge registration and shortcuts were retained."
        ${Else}
          DetailPrint "Poverello registration verification failed and registration recovery could not be verified. Files may be partially installed. Do not retry until the registration state is repaired manually."
        ${EndIf}
      ${ElseIf} $PCTHadExistingPoverello == 1
        ${If} $PCTRecoveryVerified == 1
          DetailPrint "Poverello registration verification failed. The previous Poverello registration was restored and verified; files may be partially installed and rerunning this installer is safe."
        ${Else}
          DetailPrint "Poverello registration verification failed and restoration of the previous registration could not be verified. Files may be partially installed. Do not retry until the registration state is repaired manually."
        ${EndIf}
      ${Else}
        ${If} $PCTRecoveryVerified == 1
          DetailPrint "Poverello registration verification failed. Removal of the incomplete registration was verified; files may be partially installed and rerunning this installer is safe."
        ${Else}
          DetailPrint "Poverello registration verification failed and removal of the incomplete registration could not be verified. Files may be partially installed. Do not retry until the registration state is repaired manually."
        ${EndIf}
      ${EndIf}
    ${Else}
      ${If} $PCTLegacyUpgradePending == 1
        ${If} $PCTRecoveryVerified == 1
          MessageBox MB_ICONSTOP|MB_OK "Poverello registration verification failed. The previous Anthro Bridge registration and shortcuts were retained and registration recovery was verified. Files may be partially installed; rerunning this installer is safe. No file rollback is claimed."
        ${Else}
          MessageBox MB_ICONSTOP|MB_OK "Poverello registration verification failed and registration recovery could not be verified. Files may be partially installed. Do not retry until the registry state is repaired manually. No file rollback is claimed."
        ${EndIf}
      ${ElseIf} $PCTHadExistingPoverello == 1
        ${If} $PCTRecoveryVerified == 1
          MessageBox MB_ICONSTOP|MB_OK "Poverello registration verification failed. The previous Poverello registration was restored and verified. Files may be partially installed; rerunning this installer is safe. No file rollback is claimed."
        ${Else}
          MessageBox MB_ICONSTOP|MB_OK "Poverello registration verification failed and the previous Poverello registration could not be verified after restoration. Files may be partially installed. Do not retry until the registry state is repaired manually. No file rollback is claimed."
        ${EndIf}
      ${Else}
        ${If} $PCTRecoveryVerified == 1
          MessageBox MB_ICONSTOP|MB_OK "Poverello registration verification failed. Removal of the incomplete registration was verified. Files may be partially installed; rerunning this installer is safe. No file rollback is claimed."
        ${Else}
          MessageBox MB_ICONSTOP|MB_OK "Poverello registration verification failed and removal of the incomplete registration could not be verified. Files may be partially installed. Do not retry until the registry state is repaired manually. No file rollback is claimed."
        ${EndIf}
      ${EndIf}
    ${EndIf}
    Quit
  ${EndIf}

  StrCpy $PCTRegistrationVerified 1
  ; Tauri's generated install-section calls are guarded until verification.
  ; Run the same shortcut handlers only after the new registration is durable.
  Call CreateOrUpdateStartMenuShortcut
  ${If} $PassiveMode == 1
  ${OrIf} ${Silent}
    Call CreateOrUpdateDesktopShortcut
  ${EndIf}

  ${If} $PCTLegacyUpgradePending == 1
    ; Finish registry migration before retiring any legacy shortcut or key.
    ; The durable pending marker makes partial cleanup recognizable on retry.
    Call PCTCompleteLegacyMigration
    ${If} $PCTLegacyCleanupFailed == 1
      SetErrorLevel 1
      ${If} ${Silent}
        DetailPrint "Poverello is installed and its registration was verified, but Anthro Bridge registration cleanup did not complete. The legacy registration was retained where possible. Rerun setup to retry cleanup."
      ${Else}
        MessageBox MB_ICONSTOP|MB_OK "Poverello is installed and its registration was verified, but Anthro Bridge registration cleanup did not complete. The legacy registration was retained where possible. Rerun setup to retry cleanup. No application-file rollback is claimed."
      ${EndIf}
      Quit
    ${EndIf}

    ; Only migrate shortcuts which still resolve to this exact installation.
    StrCpy $R9 0
    !insertmacro IsShortcutTarget "$SMPROGRAMS\Anthro Bridge.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $R5
    ${If} $R5 == 1
      Delete "$SMPROGRAMS\Anthro Bridge.lnk"
    ${EndIf}
    !insertmacro IsShortcutTarget "$DESKTOP\Anthro Bridge.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $R5
    ${If} $R5 == 1
      Delete "$DESKTOP\Anthro Bridge.lnk"
      StrCpy $R9 1
    ${EndIf}

    CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${If} $R9 == 1
      CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}

  ${EndIf}
!macroend

!macro PCT_HOOK_FUNCTIONS
!ifdef PCT_NSIS_TEST_BACKEND
Function PCTTestReadStateRegStr
  Pop $PCTTestRegistryValue
  Pop $PCTTestRegistryKey
  StrCpy $PCTTestRegistrySection ""
  ${If} $PCTTestRegistryKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloPublisher"
    ${If} $PCTTestRegistryValue == "LegacyUpgradePending"
      StrCpy $PCTTestRegistrySection "Migration"
    ${ElseIf} $PCTTestRegistryValue == "LegacyCleanupPending"
      StrCpy $PCTTestRegistrySection "Migration"
    ${ElseIf} $PCTTestRegistryValue == "LegacyUpgradePath"
      StrCpy $PCTTestRegistrySection "Migration"
    ${EndIf}
  ${ElseIf} $PCTTestRegistryKey == "${UNINSTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloUninstall"
  ${ElseIf} $PCTTestRegistryKey == "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
    StrCpy $PCTTestRegistrySection "LegacyUninstall"
  ${ElseIf} $PCTTestRegistryKey == "Software\Sohei IDE (soheidon)\Anthro Bridge"
    StrCpy $PCTTestRegistrySection "LegacyPublisher"
  ${EndIf}
  StrCpy $PCTTestRegistryIniValue $PCTTestRegistryValue
  ${If} $PCTTestRegistrySection == "PoverelloPublisher"
  ${AndIf} $PCTTestRegistryValue == ""
    StrCpy $PCTTestRegistryIniValue "Path"
  ${ElseIf} $PCTTestRegistrySection == "LegacyPublisher"
  ${AndIf} $PCTTestRegistryValue == ""
    StrCpy $PCTTestRegistryIniValue "Path"
  ${ElseIf} $PCTTestRegistrySection == "Migration"
    ${If} $PCTTestRegistryValue == "LegacyUpgradePending"
      StrCpy $PCTTestRegistryIniValue "Pending"
    ${ElseIf} $PCTTestRegistryValue == "LegacyCleanupPending"
      StrCpy $PCTTestRegistryIniValue "CleanupPending"
    ${Else}
      StrCpy $PCTTestRegistryIniValue "UpgradePath"
    ${EndIf}
  ${EndIf}
  ${If} $PCTTestRegistryStatePath != ""
    ClearErrors
    ReadINIStr $R0 "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "$PCTTestRegistryIniValue"
    ${If} ${Errors}
      ; A missing fake registry value models the normal RegQueryValueEx
      ; ERROR_FILE_NOT_FOUND result; only an unreadable/missing durable store
      ; is a harness failure.
      ${IfNot} ${FileExists} "$PCTTestRegistryStatePath"
        StrCpy $PCTTestRegistryReadFailed 1
      ${EndIf}
      ClearErrors
      StrCpy $R0 ""
    ${EndIf}
    ${If} $PCTTestRegistrySection == "LegacyUninstall"
      ReadINIStr $R1 "$PCTTestRegistryStatePath" "LegacyUninstall" "Present"
      ${If} $R1 != 1
        StrCpy $R0 ""
      ${EndIf}
    ${ElseIf} $PCTTestRegistrySection == "LegacyPublisher"
      ReadINIStr $R1 "$PCTTestRegistryStatePath" "LegacyPublisher" "Present"
      ${If} $R1 != 1
        StrCpy $R0 ""
      ${EndIf}
    ${EndIf}
    ${If} $R0 != ""
      ${If} $PCTTestRegistryValue == "InstallLocation"
      ${OrIf} $PCTTestRegistryValue == "UninstallString"
      ${OrIf} $PCTTestRegistryValue == "DisplayIcon"
        StrCpy $R0 "$\"$R0$\""
      ${EndIf}
    ${EndIf}
  ${ElseIf} $PCTTestRegistrySection == "LegacyUninstall"
    ${If} $PCTTestRegistryValue == "DisplayName"
      StrCpy $R0 $PCTFakeLegacyDisplayName
    ${ElseIf} $PCTTestRegistryValue == "Publisher"
      StrCpy $R0 $PCTFakeLegacyPublisher
    ${ElseIf} $PCTTestRegistryValue == "DisplayVersion"
      StrCpy $R0 $PCTFakeLegacyVersion
    ${ElseIf} $PCTTestRegistryValue == "InstallLocation"
      StrCpy $R0 $PCTFakeLegacyInstallLocation
    ${ElseIf} $PCTTestRegistryValue == "UninstallString"
      StrCpy $R0 $PCTFakeLegacyUninstallString
    ${ElseIf} $PCTTestRegistryValue == "MainBinaryName"
      StrCpy $R0 $PCTFakeLegacyMainBinaryName
    ${ElseIf} $PCTTestRegistryValue == "DisplayIcon"
      StrCpy $R0 $PCTFakeLegacyDisplayIcon
    ${Else}
      StrCpy $R0 ""
    ${EndIf}
  ${ElseIf} $PCTTestRegistrySection == "LegacyPublisher"
    StrCpy $R0 $PCTFakeLegacyPublisherPath
  ${ElseIf} $PCTTestRegistrySection == "PoverelloPublisher"
    StrCpy $R0 $PCTFakePublisherPath
  ${ElseIf} $PCTTestRegistrySection == "PoverelloUninstall"
    ${If} $PCTTestRegistryValue == "DisplayName"
      StrCpy $R0 $PCTFakeDisplayName
    ${ElseIf} $PCTTestRegistryValue == "Publisher"
      StrCpy $R0 $PCTFakePublisher
    ${ElseIf} $PCTTestRegistryValue == "DisplayVersion"
      StrCpy $R0 $PCTFakeDisplayVersion
    ${ElseIf} $PCTTestRegistryValue == "InstallLocation"
      StrCpy $R0 $PCTFakeInstallLocation
    ${ElseIf} $PCTTestRegistryValue == "UninstallString"
      StrCpy $R0 $PCTFakeUninstallString
    ${ElseIf} $PCTTestRegistryValue == "MainBinaryName"
      StrCpy $R0 $PCTFakeMainBinaryName
    ${ElseIf} $PCTTestRegistryValue == "DisplayIcon"
      StrCpy $R0 $PCTFakeDisplayIcon
    ${Else}
      StrCpy $R0 ""
    ${EndIf}
  ${Else}
    StrCpy $R0 ""
  ${EndIf}
  Push $R0
FunctionEnd

Function PCTTestDeleteStateRegValue
  Pop $PCTTestRegistryValue
  Pop $PCTTestRegistryKey
  ${If} $PCTTestRegistryKey == "${MANUPRODUCTKEY}"
  ${AndIf} $PCTTestRegistryStatePath != ""
    ${If} $PCTTestRegistryValue == "LegacyUpgradePending"
      WriteINIStr "$PCTTestRegistryStatePath" "Migration" "Pending" ""
      StrCpy $PCTPendingMarker 0
    ${ElseIf} $PCTTestRegistryValue == "LegacyCleanupPending"
      WriteINIStr "$PCTTestRegistryStatePath" "Migration" "CleanupPending" ""
      StrCpy $PCTFakeLegacyCleanupMarker 0
    ${ElseIf} $PCTTestRegistryValue == "LegacyUpgradeComplete"
      WriteINIStr "$PCTTestRegistryStatePath" "Migration" "Complete" ""
      StrCpy $PCTFakeLegacyCompleteMarker 0
    ${ElseIf} $PCTTestRegistryValue == "LegacyUpgradePath"
      WriteINIStr "$PCTTestRegistryStatePath" "Migration" "UpgradePath" ""
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.PCTTestReadStateRegStr
  Pop $PCTTestRegistryValue
  Pop $PCTTestRegistryKey
  ${If} $PCTTestRegistryKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloPublisher"
  ${ElseIf} $PCTTestRegistryKey == "${UNINSTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloUninstall"
  ${Else}
    StrCpy $PCTTestRegistrySection ""
  ${EndIf}
  StrCpy $PCTTestRegistryIniValue $PCTTestRegistryValue
  ${If} $PCTTestRegistryValue == ""
    StrCpy $PCTTestRegistryIniValue "Path"
  ${EndIf}
  ClearErrors
  ReadINIStr $PCTUninstallRawValue "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "$PCTTestRegistryIniValue"
  ${If} ${Errors}
    SetErrors
  ${EndIf}
  ${If} $PCTUninstallRawValue != ""
    ${If} $PCTTestRegistryValue == "InstallLocation"
    ${OrIf} $PCTTestRegistryValue == "UninstallString"
    ${OrIf} $PCTTestRegistryValue == "DisplayIcon"
      StrCpy $PCTUninstallRawValue "$\"$PCTUninstallRawValue$\""
    ${EndIf}
  ${EndIf}
  Push $PCTUninstallRawValue
FunctionEnd

Function un.PCTTestWriteStateRegStr
  Pop $PCTUninstallRawValue
  Pop $PCTTestRegistryValue
  Pop $PCTTestRegistryKey
  ${If} $PCTTestRegistryKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloPublisher"
  ${ElseIf} $PCTTestRegistryKey == "${UNINSTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloUninstall"
  ${Else}
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTTestFailOperation == "update-restore-write"
  ${AndIf} $PCTTestFailKey == $PCTTestRegistryKey
  ${AndIf} $PCTTestFailValue == $PCTTestRegistryValue
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  StrCpy $PCTTestRegistryIniValue $PCTTestRegistryValue
  ${If} $PCTTestRegistryValue == ""
    StrCpy $PCTTestRegistryIniValue "Path"
  ${EndIf}
  ClearErrors
  WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "Present" "1"
  WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "$PCTTestRegistryIniValue" "$PCTUninstallRawValue"
  ${If} ${Errors}
    SetErrors
  ${EndIf}
FunctionEnd

Function un.PCTTestDeleteStateRegKey
  Pop $PCTTestRegistryKey
  ${If} $PCTTestFailRegistrationDeleteKey != ""
  ${AndIf} $PCTTestFailRegistrationDeleteKey == $PCTTestRegistryKey
    StrCpy $PCTTestFailRegistrationDeleteKey ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTTestFailOperation == "registration-delete"
  ${AndIf} $PCTTestFailKey == $PCTTestRegistryKey
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTTestRegistryKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloPublisher"
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "Present" "0"
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "Path" ""
    WriteINIStr "$PCTTestRegistryStatePath" "Migration" "Pending" ""
    WriteINIStr "$PCTTestRegistryStatePath" "Migration" "CleanupPending" ""
    WriteINIStr "$PCTTestRegistryStatePath" "Migration" "UpgradePath" ""
    WriteINIStr "$PCTTestRegistryStatePath" "Migration" "Complete" ""
  ${ElseIf} $PCTTestRegistryKey == "${UNINSTKEY}"
    StrCpy $PCTTestRegistrySection "PoverelloUninstall"
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "Present" "0"
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "DisplayName" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "Publisher" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "DisplayVersion" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "InstallLocation" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "UninstallString" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "DisplayIcon" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "MainBinaryName" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "NoModify" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "NoRepair" ""
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "EstimatedSize" ""
  ${Else}
    SetErrors
    Return
  ${EndIf}
  ClearErrors
FunctionEnd

Function un.PCTTestProbeStateRegKey
  Pop $PCTTestRegistryKey
  ${If} $PCTTestRegistryKey == "${MANUPRODUCTKEY}"
    ReadINIStr $PCTUninstallRawValue "$PCTTestRegistryStatePath" "PoverelloPublisher" "Present"
  ${ElseIf} $PCTTestRegistryKey == "${UNINSTKEY}"
    ReadINIStr $PCTUninstallRawValue "$PCTTestRegistryStatePath" "PoverelloUninstall" "Present"
  ${Else}
    StrCpy $PCTUninstallKeyStatus 5
    Return
  ${EndIf}
  ${If} $PCTUninstallRawValue == "1"
    StrCpy $PCTUninstallKeyStatus 0
  ${Else}
    StrCpy $PCTUninstallKeyStatus 2
  ${EndIf}
FunctionEnd
!endif

Function un.PCTNormalizeInstallDirectory
  StrCpy $PCTNormalizeOutput ""
  StrCpy $PCTNormalizeWork $PCTNormalizeInput
  StrCpy $PCTPathFirst $PCTNormalizeWork 1
  StrLen $PCTPathLength $PCTNormalizeWork
  ${If} $PCTPathLength < 3
    Return
  ${EndIf}
  IntOp $PCTPathIndex $PCTPathLength - 1
  StrCpy $PCTPathLast $PCTNormalizeWork 1 $PCTPathIndex
  ${If} $PCTPathFirst == "$\""
    ${If} $PCTPathLast != "$\""
      Return
    ${EndIf}
    IntOp $PCTPathLength $PCTPathLength - 2
    StrCpy $PCTNormalizeWork $PCTNormalizeWork $PCTPathLength 1
  ${ElseIf} $PCTPathLast == "$\""
    Return
  ${EndIf}
  StrCpy $PCTPathIndex 0
un_pct_path_quote_scan:
  StrCpy $PCTPathFirst $PCTNormalizeWork 1 $PCTPathIndex
  ${If} $PCTPathFirst == ""
    Goto un_pct_path_quote_scan_done
  ${EndIf}
  ${If} $PCTPathFirst == "$\""
    StrCpy $PCTNormalizeOutput ""
    Return
  ${EndIf}
  IntOp $PCTPathIndex $PCTPathIndex + 1
  Goto un_pct_path_quote_scan
un_pct_path_quote_scan_done:
  StrCpy $PCTPathColon $PCTNormalizeWork 1 1
  StrCpy $PCTPathSlash $PCTNormalizeWork 1 2
  ${If} $PCTPathColon != ":"
  ${OrIf} $PCTPathSlash != "\"
    Return
  ${EndIf}
  ClearErrors
  GetFullPathName $PCTNormalizeOutput "$PCTNormalizeWork"
  ${If} ${Errors}
    StrCpy $PCTNormalizeOutput ""
    Return
  ${EndIf}
  StrLen $PCTPathLength $PCTNormalizeOutput
  ${If} $PCTPathLength > 3
    IntOp $PCTPathIndex $PCTPathLength - 1
    StrCpy $PCTPathLast $PCTNormalizeOutput 1 $PCTPathIndex
    ${If} $PCTPathLast == "\"
      StrCpy $PCTNormalizeOutput $PCTNormalizeOutput $PCTPathIndex
    ${EndIf}
  ${EndIf}
  ${UnStrCase} $PCTNormalizeOutput $PCTNormalizeOutput "L"
FunctionEnd

Function un.PCTSnapshotUpdateRegistration
  StrCpy $PCTUninstallSnapshotValid 0
  StrCpy $PCTUninstallRegistrationReadFailed 0
  StrCpy $PCTUninstallRegistrationOperationFailed 0
  StrCpy $PCTUninstallKeyStatus 5
  Push "${MANUPRODUCTKEY}"
  Call un.PCTProbePoverelloKey
  ${If} $PCTUninstallKeyStatus != 0
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  StrCpy $PCTUninstallKeyStatus 5
  Push "${UNINSTKEY}"
  Call un.PCTProbePoverelloKey
  ${If} $PCTUninstallKeyStatus != 0
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}

  !insertmacro PCT_UN_READ_REG_STR $PCTSavedPublisherPath "${MANUPRODUCTKEY}" ""
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedDisplayName "${UNINSTKEY}" "DisplayName"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedDisplayIcon "${UNINSTKEY}" "DisplayIcon"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedVersion "${UNINSTKEY}" "DisplayVersion"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedPublisher "${UNINSTKEY}" "Publisher"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedInstallLocation "${UNINSTKEY}" "InstallLocation"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedUninstallString "${UNINSTKEY}" "UninstallString"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTSavedMainBinaryName "${UNINSTKEY}" "MainBinaryName"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  ${If} $PCTUninstallRegistrationReadFailed == 1
  ${OrIf} $PCTSavedDisplayName != "${PRODUCTNAME}"
  ${OrIf} $PCTSavedPublisher != "${MANUFACTURER}"
  ${OrIf} $PCTSavedVersion == ""
  ${OrIf} $PCTSavedMainBinaryName != "${MAINBINARYNAME}.exe"
  ${OrIf} $PCTSavedDisplayIcon == ""
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTSavedPublisherPath
  Call un.PCTNormalizeInstallDirectory
  StrCpy $PCTExpectedPath $PCTNormalizeOutput
  StrCpy $PCTNormalizeInput $PCTSavedInstallLocation
  Call un.PCTNormalizeInstallDirectory
  ${If} $PCTNormalizeOutput == ""
  ${OrIf} $PCTNormalizeOutput != $PCTExpectedPath
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  StrCpy $PCTUninstallSnapshotPath $PCTExpectedPath
  ; Tauri stores a fully quoted executable command. Do not interpret trailing
  ; command-line arguments as part of an installation path.
  StrLen $PCTPathLength $PCTSavedUninstallString
  StrCpy $PCTPathFirst $PCTSavedUninstallString 1
  IntOp $PCTPathIndex $PCTPathLength - 1
  StrCpy $PCTPathLast $PCTSavedUninstallString 1 $PCTPathIndex
  ${If} $PCTPathLength < 3
  ${OrIf} $PCTPathFirst != "$\""
  ${OrIf} $PCTPathLast != "$\""
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  IntOp $PCTPathLength $PCTPathLength - 2
  StrCpy $PCTNormalizeInput $PCTSavedUninstallString $PCTPathLength 1
  Call un.PCTNormalizeInstallDirectory
  StrCpy $PCTUninstallRawValue $PCTNormalizeOutput
  StrCpy $PCTExpectedPath "$PCTUninstallSnapshotPath\uninstall.exe"
  StrCpy $PCTNormalizeInput $PCTExpectedPath
  Call un.PCTNormalizeInstallDirectory
  StrCpy $PCTExpectedPath $PCTNormalizeOutput
  ${If} $PCTUninstallRawValue != $PCTExpectedPath
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTSavedDisplayIcon
  StrLen $PCTPathLength $PCTNormalizeInput
  StrCpy $PCTPathFirst $PCTNormalizeInput 1
  IntOp $PCTPathIndex $PCTPathLength - 1
  StrCpy $PCTPathLast $PCTNormalizeInput 1 $PCTPathIndex
  ${If} $PCTPathLength < 3
  ${OrIf} $PCTPathFirst != "$\""
  ${OrIf} $PCTPathLast != "$\""
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  IntOp $PCTPathLength $PCTPathLength - 2
  StrCpy $PCTNormalizeInput $PCTSavedDisplayIcon $PCTPathLength 1
  Call un.PCTNormalizeInstallDirectory
  StrCpy $PCTUninstallRawValue $PCTNormalizeOutput
  StrCpy $PCTExpectedPath "$PCTUninstallSnapshotPath\${MAINBINARYNAME}.exe"
  StrCpy $PCTNormalizeInput $PCTExpectedPath
  Call un.PCTNormalizeInstallDirectory
  StrCpy $PCTExpectedPath $PCTNormalizeOutput
  ${If} $PCTUninstallRawValue != $PCTExpectedPath
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
!ifdef PCT_NSIS_TEST_BACKEND
  ; The fake backend tests registration semantics without touching a user's
  ; real install directory; the live uninstaller's caller already classified it.
!else
  ${IfNot} ${FileExists} "$PCTExpectedPath"
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTSavedPublisherPath
  Call un.PCTNormalizeInstallDirectory
  ${IfNot} ${FileExists} "$PCTNormalizeOutput\uninstall.exe"
    StrCpy $PCTUninstallRegistrationReadFailed 1
    Return
  ${EndIf}
!endif
  !insertmacro PCT_UN_READ_REG_DWORD $PCTSavedNoModify "${UNINSTKEY}" "NoModify"
  ${If} ${Errors}
    StrCpy $PCTSavedNoModifyExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTSavedNoModifyExists 1
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_DWORD $PCTSavedNoRepair "${UNINSTKEY}" "NoRepair"
  ${If} ${Errors}
    StrCpy $PCTSavedNoRepairExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTSavedNoRepairExists 1
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_DWORD $PCTSavedEstimatedSize "${UNINSTKEY}" "EstimatedSize"
  ${If} ${Errors}
    StrCpy $PCTSavedEstimatedSizeExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTSavedEstimatedSizeExists 1
  ${EndIf}
  StrCpy $PCTUninstallSnapshotValid 1
FunctionEnd

Function un.PCTRestoreUpdateRegistration
  StrCpy $PCTUninstallRegistrationVerified 0
  ${If} $PCTUninstallSnapshotValid != 1
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    Return
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${MANUPRODUCTKEY}" "" "$PCTSavedPublisherPath"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "DisplayName" "$PCTSavedDisplayName"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "DisplayIcon" "$PCTSavedDisplayIcon"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "DisplayVersion" "$PCTSavedVersion"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "Publisher" "$PCTSavedPublisher"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "InstallLocation" "$PCTSavedInstallLocation"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "UninstallString" "$PCTSavedUninstallString"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_WRITE_REG_STR "${UNINSTKEY}" "MainBinaryName" "$PCTSavedMainBinaryName"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  ${If} $PCTSavedNoModifyExists == 1
    !insertmacro PCT_UN_WRITE_REG_DWORD "${UNINSTKEY}" "NoModify" "$PCTSavedNoModify"
    ${If} ${Errors}
      StrCpy $PCTUninstallRegistrationOperationFailed 1
      ClearErrors
    ${EndIf}
  ${EndIf}
  ${If} $PCTSavedNoRepairExists == 1
    !insertmacro PCT_UN_WRITE_REG_DWORD "${UNINSTKEY}" "NoRepair" "$PCTSavedNoRepair"
    ${If} ${Errors}
      StrCpy $PCTUninstallRegistrationOperationFailed 1
      ClearErrors
    ${EndIf}
  ${EndIf}
  ${If} $PCTSavedEstimatedSizeExists == 1
    !insertmacro PCT_UN_WRITE_REG_DWORD "${UNINSTKEY}" "EstimatedSize" "$PCTSavedEstimatedSize"
    ${If} ${Errors}
      StrCpy $PCTUninstallRegistrationOperationFailed 1
      ClearErrors
    ${EndIf}
  ${EndIf}
  Call un.PCTVerifyUpdateRegistration
  ${If} $PCTUninstallRegistrationOperationFailed == 0
  ${AndIf} $PCTUninstallRegistrationReadFailed == 0
    StrCpy $PCTUninstallRegistrationVerified 1
  ${EndIf}
FunctionEnd

Function un.PCTVerifyUpdateRegistration
  StrCpy $PCTUninstallRegistrationReadFailed 0
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${MANUPRODUCTKEY}" ""
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedPublisherPath
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "DisplayName"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedDisplayName
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "DisplayIcon"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedDisplayIcon
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "DisplayVersion"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedVersion
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "Publisher"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedPublisher
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "InstallLocation"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedInstallLocation
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "UninstallString"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedUninstallString
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_READ_REG_STR $PCTUninstallRawValue "${UNINSTKEY}" "MainBinaryName"
  ${If} ${Errors}
  ${OrIf} $PCTUninstallRawValue != $PCTSavedMainBinaryName
    StrCpy $PCTUninstallRegistrationReadFailed 1
    ClearErrors
  ${EndIf}
  ${If} $PCTSavedNoModifyExists == 1
    ClearErrors
    !insertmacro PCT_UN_READ_REG_DWORD $PCTUninstallRawValue "${UNINSTKEY}" "NoModify"
    ${If} ${Errors}
    ${OrIf} $PCTUninstallRawValue != $PCTSavedNoModify
      StrCpy $PCTUninstallRegistrationReadFailed 1
      ClearErrors
    ${EndIf}
  ${EndIf}
  ${If} $PCTSavedNoRepairExists == 1
    ClearErrors
    !insertmacro PCT_UN_READ_REG_DWORD $PCTUninstallRawValue "${UNINSTKEY}" "NoRepair"
    ${If} ${Errors}
    ${OrIf} $PCTUninstallRawValue != $PCTSavedNoRepair
      StrCpy $PCTUninstallRegistrationReadFailed 1
      ClearErrors
    ${EndIf}
  ${EndIf}
  ${If} $PCTSavedEstimatedSizeExists == 1
    ClearErrors
    !insertmacro PCT_UN_READ_REG_DWORD $PCTUninstallRawValue "${UNINSTKEY}" "EstimatedSize"
    ${If} ${Errors}
    ${OrIf} $PCTUninstallRawValue != $PCTSavedEstimatedSize
      StrCpy $PCTUninstallRegistrationReadFailed 1
      ClearErrors
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.PCTProbePoverelloKey
  Pop $PCTCheckKey
!ifdef PCT_NSIS_TEST_BACKEND
  Push $PCTCheckKey
  Call un.PCTTestProbeStateRegKey
!else
  System::Call 'advapi32::RegOpenKeyExW(p 0x80000001, w "$PCTCheckKey", i 0, i 0x20019, *p .r0)i.r1'
  StrCpy $PCTUninstallKeyStatus $1
  ${If} $1 == 0
    System::Call 'advapi32::RegCloseKey(p r0)i.r1'
    ${If} $1 != 0
      StrCpy $PCTUninstallKeyStatus $1
    ${EndIf}
  ${EndIf}
!endif
FunctionEnd

Function un.PCTRemovePoverelloRegistration
  StrCpy $PCTUninstallRegistrationVerified 0
  StrCpy $PCTUninstallRegistrationOperationFailed 0
  StrCpy $PCTUninstallRegistrationReadFailed 0
  !insertmacro PCT_UN_DELETE_REG_KEY "${MANUPRODUCTKEY}"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  !insertmacro PCT_UN_DELETE_REG_KEY "${UNINSTKEY}"
  ${If} ${Errors}
    StrCpy $PCTUninstallRegistrationOperationFailed 1
    ClearErrors
  ${EndIf}
  StrCpy $PCTCheckKey "${MANUPRODUCTKEY}"
  Push $PCTCheckKey
  Call un.PCTProbePoverelloKey
  ${If} $PCTUninstallKeyStatus != 2
  ${AndIf} $PCTUninstallKeyStatus != 3
    StrCpy $PCTUninstallRegistrationReadFailed 1
  ${EndIf}
  StrCpy $PCTCheckKey "${UNINSTKEY}"
  Push $PCTCheckKey
  Call un.PCTProbePoverelloKey
  ${If} $PCTUninstallKeyStatus != 2
  ${AndIf} $PCTUninstallKeyStatus != 3
    StrCpy $PCTUninstallRegistrationReadFailed 1
  ${EndIf}
  ${If} $PCTUninstallRegistrationOperationFailed == 0
  ${AndIf} $PCTUninstallRegistrationReadFailed == 0
    StrCpy $PCTUninstallRegistrationVerified 1
  ${EndIf}
FunctionEnd

Function un.PCTReportDataCleanupFailure
!ifdef PCT_NSIS_TEST_BACKEND
  ReadEnvStr $8 "PCT_NSIS_TEST_RESULT_PATH"
  ${If} $8 != ""
    FileOpen $7 $8 a
    FileWrite $7 "status=failed|reason=data-cleanup|cleanup-failed=1|reparse=$PCTUninstallReparseFound|path=$PCTUninstallDataPath|registration-verified=$PCTUninstallRegistrationVerified|no-file-rollback-claimed=1$\n"
    FileClose $7
  ${EndIf}
!endif
  SetErrorLevel 1
  ${If} $PCTUninstallRegistrationVerified == 1
    ${If} ${Silent}
      DetailPrint "Poverello registration was removed, but user data at $PCTUninstallDataPath could not be safely or completely removed. Data may remain or be partially removed; no application-file rollback is claimed."
    ${Else}
      MessageBox MB_ICONEXCLAMATION|MB_OK "Poverello registration was removed, but user data at $PCTUninstallDataPath could not be safely or completely removed. Data may remain or be partially removed; no application-file rollback is claimed."
    ${EndIf}
  ${Else}
    ${If} ${Silent}
      DetailPrint "Poverello user data cleanup failed and registry registration removal could not be verified. Data may remain or be partially removed and registration may remain; no application-file rollback is claimed. Repair the installation before retrying."
    ${Else}
      MessageBox MB_ICONSTOP|MB_OK "Poverello user data cleanup failed and registry registration removal could not be verified. Data may remain or be partially removed and registration may remain; no application-file rollback is claimed. Repair the installation before retrying."
    ${EndIf}
  ${EndIf}
  Quit
FunctionEnd

Function PCTNormalizeAbsolutePath
  StrCpy $PCTNormalizeOutput ""
  StrCpy $PCTNormalizeWork $PCTNormalizeInput
  StrCpy $PCTPathFirst $PCTNormalizeWork 1
  StrLen $PCTPathLength $PCTNormalizeWork
  ${If} $PCTPathLength < 3
    Return
  ${EndIf}
  IntOp $PCTPathIndex $PCTPathLength - 1
  StrCpy $PCTPathLast $PCTNormalizeWork 1 $PCTPathIndex
  ${If} $PCTPathFirst == "$\""
    ${If} $PCTPathLast != "$\""
      Return
    ${EndIf}
    IntOp $PCTPathLength $PCTPathLength - 2
    StrCpy $PCTNormalizeWork $PCTNormalizeWork $PCTPathLength 1
  ${ElseIf} $PCTPathLast == "$\""
    Return
  ${EndIf}

  ; A quote is valid only as the complete outer delimiter pair. Any quote
  ; remaining inside the path indicates malformed quoting or appended text.
  StrCpy $PCTPathIndex 0
pct_normalize_absolute_quote_scan:
  StrCpy $PCTPathFirst $PCTNormalizeWork 1 $PCTPathIndex
  ${If} $PCTPathFirst == ""
    Goto pct_normalize_absolute_quote_scan_done
  ${EndIf}
  ${If} $PCTPathFirst == "$\""
    StrCpy $PCTNormalizeOutput ""
    Return
  ${EndIf}
  IntOp $PCTPathIndex $PCTPathIndex + 1
  Goto pct_normalize_absolute_quote_scan
pct_normalize_absolute_quote_scan_done:

  ; The supported product installs are local per-user paths. Reject relative,
  ; drive-relative, and UNC paths rather than resolving them against cwd.
  StrCpy $PCTPathDrive $PCTNormalizeWork 1
  StrCpy $PCTPathColon $PCTNormalizeWork 1 1
  StrCpy $PCTPathSlash $PCTNormalizeWork 1 2
  ${If} $PCTPathColon != ":"
    Return
  ${EndIf}
  ${If} $PCTPathSlash != "\"
    Return
  ${EndIf}
  ClearErrors
  GetFullPathName $PCTNormalizeOutput "$PCTNormalizeWork"
  ${If} ${Errors}
    StrCpy $PCTNormalizeOutput ""
    Return
  ${EndIf}
  StrLen $PCTPathLength $PCTNormalizeOutput
  ${If} $PCTPathLength > 3
    IntOp $PCTPathIndex $PCTPathLength - 1
    StrCpy $PCTPathLast $PCTNormalizeOutput 1 $PCTPathIndex
    ${If} $PCTPathLast == "\"
      StrCpy $PCTNormalizeOutput $PCTNormalizeOutput $PCTPathIndex
    ${EndIf}
  ${EndIf}
  ${StrCase} $PCTNormalizeOutput $PCTNormalizeOutput "L"
FunctionEnd

Function PCTNormalizeQuotedExecutable
  StrCpy $PCTNormalizeOutput ""
  StrCpy $PCTPathFirst $PCTNormalizeInput 1
  StrLen $PCTPathLength $PCTNormalizeInput
  ${If} $PCTPathLength < 3
    Return
  ${EndIf}
  IntOp $PCTPathIndex $PCTPathLength - 1
  StrCpy $PCTPathLast $PCTNormalizeInput 1 $PCTPathIndex
  ${If} $PCTPathFirst != "$\""
  ${OrIf} $PCTPathLast != "$\""
    Return
  ${EndIf}
  IntOp $PCTPathLength $PCTPathLength - 2
  StrCpy $PCTNormalizeWork $PCTNormalizeInput $PCTPathLength 1
  StrCpy $PCTNormalizeInput $PCTNormalizeWork
  Call PCTNormalizeAbsolutePath
FunctionEnd

Function PCTValidateLegacyRegistration
  StrCpy $PCTLegacyHasRegistry 0
  StrCpy $PCTLegacyOrphanExecutable 0
  StrCpy $PCTLegacyValid 0
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyDisplayName "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "DisplayName"
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyPublisher "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "Publisher"
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyVersion "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "DisplayVersion"
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyInstallLocation "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "InstallLocation"
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyUninstallString "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "UninstallString"
  ClearErrors
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyDisplayIcon "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "DisplayIcon"
  ${If} ${Errors}
    StrCpy $PCTLegacyDisplayIconExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTLegacyDisplayIconExists 1
  ${EndIf}
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyMainBinaryName "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge" "MainBinaryName"
  !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyPath "Software\Sohei IDE (soheidon)\Anthro Bridge" ""
  StrCpy $PCTLegacyRawPath $PCTLegacyPath

  ${If} $PCTLegacyDisplayName != ""
  ${OrIf} $PCTLegacyPublisher != ""
  ${OrIf} $PCTLegacyVersion != ""
  ${OrIf} $PCTLegacyInstallLocation != ""
  ${OrIf} $PCTLegacyUninstallString != ""
  ${OrIf} $PCTLegacyDisplayIcon != ""
  ${OrIf} $PCTLegacyMainBinaryName != ""
  ${OrIf} $PCTLegacyPath != ""
    StrCpy $PCTLegacyHasRegistry 1
  ${EndIf}
!ifdef PCT_NSIS_TEST_BACKEND
  ; Keep classifier tests isolated from the actual user's profile.
  ReadEnvStr $PCTTestLegacyOrphanPath "PCT_NSIS_TEST_LEGACY_ORPHAN"
  ${If} $PCTTestLegacyOrphanPath != ""
  ${AndIf} ${FileExists} "$PCTTestLegacyOrphanPath"
    StrCpy $PCTLegacyOrphanExecutable 1
  ${EndIf}
!else
  ${If} ${FileExists} "$LOCALAPPDATA\Anthro Bridge\${MAINBINARYNAME}.exe"
    StrCpy $PCTLegacyOrphanExecutable 1
  ${EndIf}
!endif
  ${If} $PCTLegacyHasRegistry != 1
    Return
  ${EndIf}

  ${If} $PCTLegacyDisplayName != "Anthro Bridge"
  ${OrIf} $PCTLegacyPublisher != "Sohei IDE (soheidon)"
  ${OrIf} $PCTLegacyVersion == ""
  ${OrIf} $PCTLegacyMainBinaryName != "${MAINBINARYNAME}.exe"
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTLegacyPath
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput == ""
    Return
  ${EndIf}
  StrCpy $PCTLegacyPath $PCTNormalizeOutput
  StrCpy $PCTNormalizeInput $PCTLegacyInstallLocation
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput != $PCTLegacyPath
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTLegacyUninstallString
  Call PCTNormalizeQuotedExecutable
  StrCpy $PCTExpectedPath "$PCTLegacyPath\uninstall.exe"
  StrCpy $PCTNormalizeInput $PCTExpectedPath
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput == ""
    Return
  ${EndIf}
  ${If} $PCTNormalizeOutput != $PCTExpectedPath
    StrCpy $PCTExpectedPath $PCTNormalizeOutput
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTLegacyUninstallString
  Call PCTNormalizeQuotedExecutable
  ${If} $PCTNormalizeOutput != $PCTExpectedPath
    Return
  ${EndIf}
  ${IfNot} ${FileExists} "$PCTLegacyPath\${MAINBINARYNAME}.exe"
    Return
  ${EndIf}
  ${IfNot} ${FileExists} "$PCTLegacyPath\uninstall.exe"
    Return
  ${EndIf}
  StrCpy $PCTLegacyValid 1
FunctionEnd

Function PCTLegacyCleanupDeleteKey
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTTestRegistryStatePath != ""
    ${If} $PCTLegacyCleanupKey == "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
      StrCpy $PCTTestRegistrySection "LegacyUninstall"
    ${Else}
      StrCpy $PCTTestRegistrySection "LegacyPublisher"
    ${EndIf}
    ReadINIStr $R0 "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "DeleteCalls"
    IntOp $R0 $R0 + 1
    WriteINIStr "$PCTTestRegistryStatePath" "$PCTTestRegistrySection" "DeleteCalls" "$R0"
  ${EndIf}
  ${If} $PCTTestFailOperation == "legacy-delete-key"
  ${AndIf} $PCTTestFailKey == $PCTLegacyCleanupKey
    StrCpy $PCTTestFailOperation ""
    SetErrors
  ${ElseIf} $PCTLegacyCleanupKey == "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
    StrCpy $PCTFakeLegacyUninstallKeyExists 0
    ${If} $PCTTestRegistryStatePath != ""
      WriteINIStr "$PCTTestRegistryStatePath" "LegacyUninstall" "Present" "0"
    ${EndIf}
  ${ElseIf} $PCTLegacyCleanupKey == "Software\Sohei IDE (soheidon)\Anthro Bridge"
    StrCpy $PCTFakeLegacyPublisherKeyExists 0
    ${If} $PCTTestRegistryStatePath != ""
      WriteINIStr "$PCTTestRegistryStatePath" "LegacyPublisher" "Present" "0"
    ${EndIf}
  ${Else}
    SetErrors
  ${EndIf}
!else
  DeleteRegKey HKCU "$PCTLegacyCleanupKey"
!endif
  ${If} ${Errors}
    StrCpy $PCTLegacyCleanupOperationFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTLegacyCleanupProbeKey
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTLegacyCleanupPhase == "readback"
  ${AndIf} $PCTTestFailOperation == "legacy-probe-readback"
  ${AndIf} $PCTTestFailKey == $PCTLegacyCleanupKey
    StrCpy $PCTTestFailOperation ""
    StrCpy $PCTLegacyCleanupStatus 5
    Return
  ${EndIf}
  ${If} $PCTTestFailOperation == "legacy-probe-key"
  ${AndIf} $PCTTestFailKey == $PCTLegacyCleanupKey
    StrCpy $PCTTestFailOperation ""
    StrCpy $PCTLegacyCleanupStatus 5
    Return
  ${EndIf}
  ${If} $PCTLegacyCleanupKey == "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
    ${If} $PCTTestRegistryStatePath != ""
      ReadINIStr $R0 "$PCTTestRegistryStatePath" "LegacyUninstall" "Present"
    ${Else}
      StrCpy $R0 $PCTFakeLegacyUninstallKeyExists
    ${EndIf}
    ${If} $R0 == 1
      StrCpy $PCTLegacyCleanupStatus 0
    ${Else}
      StrCpy $PCTLegacyCleanupStatus 2
    ${EndIf}
  ${ElseIf} $PCTLegacyCleanupKey == "Software\Sohei IDE (soheidon)\Anthro Bridge"
    ${If} $PCTTestRegistryStatePath != ""
      ReadINIStr $R0 "$PCTTestRegistryStatePath" "LegacyPublisher" "Present"
    ${Else}
      StrCpy $R0 $PCTFakeLegacyPublisherKeyExists
    ${EndIf}
    ${If} $R0 == 1
      StrCpy $PCTLegacyCleanupStatus 0
    ${Else}
      StrCpy $PCTLegacyCleanupStatus 2
    ${EndIf}
  ${Else}
    StrCpy $PCTLegacyCleanupStatus 5
  ${EndIf}
!else
  System::Call 'advapi32::RegOpenKeyExW(p 0x80000001, w "$PCTLegacyCleanupKey", i 0, i 0x20019, *p .r0)i.r1'
  StrCpy $PCTLegacyCleanupStatus $1
  ${If} $1 == 0
    System::Call 'advapi32::RegCloseKey(p r0)i.r1'
    ${If} $1 != 0
      StrCpy $PCTLegacyCleanupStatus $1
    ${EndIf}
  ${EndIf}
!endif
FunctionEnd

Function PCTLegacyCleanupWritePending
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTTestFailOperation == "legacy-write-pending"
    StrCpy $PCTTestFailOperation ""
    SetErrors
  ${Else}
    StrCpy $PCTFakeLegacyCleanupMarker 1
    ${If} $PCTTestRegistryStatePath != ""
      WriteINIStr "$PCTTestRegistryStatePath" "Migration" "CleanupPending" "1"
    ${EndIf}
  ${EndIf}
!else
  WriteRegDWORD HKCU "${MANUPRODUCTKEY}" "LegacyCleanupPending" 1
!endif
  ${If} ${Errors}
    StrCpy $PCTLegacyCleanupOperationFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTLegacyCleanupWriteComplete
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTTestFailOperation == "legacy-write-complete"
    StrCpy $PCTTestFailOperation ""
    SetErrors
  ${Else}
    StrCpy $PCTFakeLegacyCompleteMarker 1
    ${If} $PCTTestRegistryStatePath != ""
      WriteINIStr "$PCTTestRegistryStatePath" "Migration" "Complete" "1"
    ${EndIf}
  ${EndIf}
!else
  WriteRegDWORD HKCU "${MANUPRODUCTKEY}" "LegacyUpgradeComplete" 1
!endif
  ${If} ${Errors}
    StrCpy $PCTLegacyCleanupOperationFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTLegacyCleanupVerifyPending
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTFakeLegacyCleanupMarker != 1
    StrCpy $PCTLegacyCleanupReadbackFailed 1
  ${EndIf}
!else
  ClearErrors
  ReadRegDWORD $PCTRegistryDwordValue HKCU "${MANUPRODUCTKEY}" "LegacyCleanupPending"
  ${If} ${Errors}
  ${OrIf} $PCTRegistryDwordValue != 1
    StrCpy $PCTLegacyCleanupReadbackFailed 1
    ClearErrors
  ${EndIf}
!endif
FunctionEnd

Function PCTLegacyCleanupVerifyComplete
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTFakeLegacyCompleteMarker != 1
    StrCpy $PCTLegacyCleanupReadbackFailed 1
  ${EndIf}
!else
  ClearErrors
  ReadRegDWORD $PCTRegistryDwordValue HKCU "${MANUPRODUCTKEY}" "LegacyUpgradeComplete"
  ${If} ${Errors}
  ${OrIf} $PCTRegistryDwordValue != 1
    StrCpy $PCTLegacyCleanupReadbackFailed 1
    ClearErrors
  ${EndIf}
!endif
FunctionEnd

Function PCTCompleteLegacyMigration
  StrCpy $PCTLegacyUpgradeComplete 0
  StrCpy $PCTLegacyCleanupFailed 0
  StrCpy $PCTLegacyCleanupOperationFailed 0
  StrCpy $PCTLegacyCleanupReadbackFailed 0
  StrCpy $PCTLegacyCleanupPhase "write-pending"
  StrCpy $PCTLegacyCleanupKey "${MANUPRODUCTKEY}"
  Call PCTLegacyCleanupWritePending
  ${If} $PCTLegacyCleanupOperationFailed == 0
    Call PCTLegacyCleanupVerifyPending
  ${EndIf}
  ${If} $PCTLegacyCleanupOperationFailed == 0
  ${AndIf} $PCTLegacyCleanupReadbackFailed == 0
    StrCpy $PCTLegacyCleanupPhase "delete"
    StrCpy $PCTLegacyCleanupKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
    Call PCTLegacyCleanupProbeKey
    ${If} $PCTLegacyCleanupStatus == 0
      Call PCTLegacyCleanupDeleteKey
    ${ElseIf} $PCTLegacyCleanupStatus != 2
    ${AndIf} $PCTLegacyCleanupStatus != 3
      StrCpy $PCTLegacyCleanupReadbackFailed 1
    ${EndIf}

    StrCpy $PCTLegacyCleanupKey "Software\Sohei IDE (soheidon)\Anthro Bridge"
    Call PCTLegacyCleanupProbeKey
    ${If} $PCTLegacyCleanupStatus == 0
      Call PCTLegacyCleanupDeleteKey
    ${ElseIf} $PCTLegacyCleanupStatus != 2
    ${AndIf} $PCTLegacyCleanupStatus != 3
      StrCpy $PCTLegacyCleanupReadbackFailed 1
    ${EndIf}

    ; Confirm both legacy keys are absent; access errors are not absence.
    StrCpy $PCTLegacyCleanupPhase "readback"
    StrCpy $PCTLegacyCleanupKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
    Call PCTLegacyCleanupProbeKey
    ${If} $PCTLegacyCleanupStatus != 2
    ${AndIf} $PCTLegacyCleanupStatus != 3
      StrCpy $PCTLegacyCleanupReadbackFailed 1
    ${EndIf}
    StrCpy $PCTLegacyCleanupKey "Software\Sohei IDE (soheidon)\Anthro Bridge"
    Call PCTLegacyCleanupProbeKey
    ${If} $PCTLegacyCleanupStatus != 2
    ${AndIf} $PCTLegacyCleanupStatus != 3
      StrCpy $PCTLegacyCleanupReadbackFailed 1
    ${EndIf}

    ${If} $PCTLegacyCleanupOperationFailed == 0
    ${AndIf} $PCTLegacyCleanupReadbackFailed == 0
      Call PCTLegacyCleanupWriteComplete
      ${If} $PCTLegacyCleanupOperationFailed == 0
        Call PCTLegacyCleanupVerifyComplete
      ${EndIf}
    ${EndIf}
  ${EndIf}

  ${If} $PCTLegacyCleanupOperationFailed == 0
  ${AndIf} $PCTLegacyCleanupReadbackFailed == 0
    StrCpy $PCTLegacyUpgradeComplete 1
  ${Else}
    StrCpy $PCTLegacyCleanupFailed 1
  ${EndIf}
FunctionEnd

Function PCTValidateLegacyCleanupResidue
  ; A durable cleanup marker plus the verified pending install path allows a
  ; retry after either legacy key was already removed. Any remaining key must
  ; still identify that exact installation; arbitrary partial registrations
  ; remain fail-closed.
  StrCpy $PCTLegacyCleanupReadbackFailed 0
  StrCpy $PCTLegacyCleanupKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
  Call PCTLegacyCleanupProbeKey
  ${If} $PCTLegacyCleanupStatus != 0
  ${AndIf} $PCTLegacyCleanupStatus != 2
  ${AndIf} $PCTLegacyCleanupStatus != 3
    StrCpy $PCTLegacyCleanupReadbackFailed 1
    Return
  ${EndIf}
  ${If} $PCTLegacyCleanupStatus == 0
    ${If} $PCTLegacyDisplayName != "Anthro Bridge"
    ${OrIf} $PCTLegacyPublisher != "Sohei IDE (soheidon)"
    ${OrIf} $PCTLegacyVersion == ""
    ${OrIf} $PCTLegacyMainBinaryName != "${MAINBINARYNAME}.exe"
      StrCpy $PCTLegacyCleanupReadbackFailed 1
      Return
    ${EndIf}
    StrCpy $PCTNormalizeInput $PCTLegacyInstallLocation
    Call PCTNormalizeAbsolutePath
    ${If} $PCTNormalizeOutput != $PCTPendingPath
      StrCpy $PCTLegacyCleanupReadbackFailed 1
      Return
    ${EndIf}
    StrCpy $PCTNormalizeInput $PCTLegacyUninstallString
    Call PCTNormalizeQuotedExecutable
    StrCpy $PCTExpectedPath "$PCTPendingPath\uninstall.exe"
    StrCpy $PCTNormalizeInput $PCTExpectedPath
    Call PCTNormalizeAbsolutePath
    ${If} $PCTNormalizeOutput != $PCTExpectedPath
      StrCpy $PCTExpectedPath $PCTNormalizeOutput
    ${EndIf}
    StrCpy $PCTNormalizeInput $PCTLegacyUninstallString
    Call PCTNormalizeQuotedExecutable
    ${If} $PCTNormalizeOutput != $PCTExpectedPath
      StrCpy $PCTLegacyCleanupReadbackFailed 1
      Return
    ${EndIf}
  ${EndIf}

  StrCpy $PCTLegacyCleanupKey "Software\Sohei IDE (soheidon)\Anthro Bridge"
  Call PCTLegacyCleanupProbeKey
  ${If} $PCTLegacyCleanupStatus != 0
  ${AndIf} $PCTLegacyCleanupStatus != 2
  ${AndIf} $PCTLegacyCleanupStatus != 3
    StrCpy $PCTLegacyCleanupReadbackFailed 1
    Return
  ${EndIf}
  ${If} $PCTLegacyCleanupStatus == 0
!ifdef PCT_NSIS_TEST_BACKEND
    !insertmacro PCT_READ_STATE_REG_STR $PCTLegacyRawPath "$PCTLegacyCleanupKey" ""
!else
    ClearErrors
    ReadRegStr $PCTLegacyRawPath HKCU "$PCTLegacyCleanupKey" ""
    ${If} ${Errors}
      StrCpy $PCTLegacyCleanupReadbackFailed 1
      ClearErrors
      Return
    ${EndIf}
!endif
    StrCpy $PCTNormalizeInput $PCTLegacyRawPath
    Call PCTNormalizeAbsolutePath
    ${If} $PCTNormalizeOutput != $PCTPendingPath
      StrCpy $PCTLegacyCleanupReadbackFailed 1
    ${EndIf}
  ${EndIf}
FunctionEnd

Function PCTValidatePoverelloRegistration
  StrCpy $PCTNewValid 0
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewPath "${MANUPRODUCTKEY}" ""
  StrCpy $PCTNewRawPublisherPath $PCTNewPath
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewDisplayName "${UNINSTKEY}" "DisplayName"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewPublisher "${UNINSTKEY}" "Publisher"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewVersion "${UNINSTKEY}" "DisplayVersion"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewInstallLocation "${UNINSTKEY}" "InstallLocation"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewUninstallString "${UNINSTKEY}" "UninstallString"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewDisplayIcon "${UNINSTKEY}" "DisplayIcon"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewMainBinaryName "${UNINSTKEY}" "MainBinaryName"
  ${If} $PCTNewDisplayName == "${PRODUCTNAME}"
  ${AndIf} $PCTNewPublisher == "${MANUFACTURER}"
  ${AndIf} $PCTNewVersion != ""
  ${AndIf} $PCTNewInstallLocation != ""
  ${AndIf} $PCTNewUninstallString != ""
  ${AndIf} $PCTNewDisplayIcon != ""
  ${AndIf} $PCTNewMainBinaryName == "${MAINBINARYNAME}.exe"
  ${AndIf} $PCTNewPath != ""
  ${Else}
    Return
  ${EndIf}
  ${If} $PCTRequireCurrentVersion == 1
  ${AndIf} $PCTNewVersion != "${VERSION}"
    Return
  ${EndIf}

  StrCpy $PCTNormalizeInput $PCTNewPath
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput == ""
    Return
  ${EndIf}
  StrCpy $PCTNewPath $PCTNormalizeOutput
  StrCpy $PCTNormalizeInput $PCTNewInstallLocation
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput != $PCTNewPath
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTNewUninstallString
  Call PCTNormalizeQuotedExecutable
  StrCpy $PCTExpectedPath "$PCTNewPath\uninstall.exe"
  StrCpy $PCTNormalizeInput $PCTExpectedPath
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput != $PCTExpectedPath
    StrCpy $PCTExpectedPath $PCTNormalizeOutput
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTNewUninstallString
  Call PCTNormalizeQuotedExecutable
  ${If} $PCTNormalizeOutput != $PCTExpectedPath
    Return
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTNewDisplayIcon
  Call PCTNormalizeQuotedExecutable
  StrCpy $PCTExpectedPath "$PCTNewPath\${MAINBINARYNAME}.exe"
  StrCpy $PCTNormalizeInput $PCTExpectedPath
  Call PCTNormalizeAbsolutePath
  ${If} $PCTNormalizeOutput != $PCTExpectedPath
    StrCpy $PCTExpectedPath $PCTNormalizeOutput
  ${EndIf}
  StrCpy $PCTNormalizeInput $PCTNewDisplayIcon
  Call PCTNormalizeQuotedExecutable
  ${If} $PCTNormalizeOutput != $PCTExpectedPath
    Return
  ${EndIf}
  ${If} $PCTRequireCurrentVersion == 1
  ${AndIf} $PCTNewPath != $INSTDIR
    ; The resolved publisher path has already been canonicalized.
    StrCpy $PCTNormalizeInput $INSTDIR
    Call PCTNormalizeAbsolutePath
    ${If} $PCTNormalizeOutput != $PCTNewPath
      Return
    ${EndIf}
  ${EndIf}
  ${IfNot} ${FileExists} "$PCTNewPath\${MAINBINARYNAME}.exe"
    Return
  ${EndIf}
  ${IfNot} ${FileExists} "$PCTNewPath\uninstall.exe"
    Return
  ${EndIf}
  StrCpy $PCTNewValid 1
FunctionEnd

Function PCTSnapshotPoverelloRegistration
  StrCpy $PCTSavedPublisherPath $PCTNewRawPublisherPath
  StrCpy $PCTSavedDisplayName $PCTNewDisplayName
  ReadRegStr $PCTSavedDisplayIcon HKCU "${UNINSTKEY}" "DisplayIcon"
  StrCpy $PCTSavedVersion $PCTNewVersion
  StrCpy $PCTSavedPublisher $PCTNewPublisher
  StrCpy $PCTSavedInstallLocation $PCTNewInstallLocation
  StrCpy $PCTSavedUninstallString $PCTNewUninstallString
  StrCpy $PCTSavedMainBinaryName $PCTNewMainBinaryName
  ClearErrors
  ReadRegDWORD $PCTSavedNoModify HKCU "${UNINSTKEY}" "NoModify"
  ${If} ${Errors}
    StrCpy $PCTSavedNoModifyExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTSavedNoModifyExists 1
  ${EndIf}
  ClearErrors
  ReadRegDWORD $PCTSavedNoRepair HKCU "${UNINSTKEY}" "NoRepair"
  ${If} ${Errors}
    StrCpy $PCTSavedNoRepairExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTSavedNoRepairExists 1
  ${EndIf}
  ClearErrors
  ReadRegDWORD $PCTSavedEstimatedSize HKCU "${UNINSTKEY}" "EstimatedSize"
  ${If} ${Errors}
    StrCpy $PCTSavedEstimatedSizeExists 0
    ClearErrors
  ${Else}
    StrCpy $PCTSavedEstimatedSizeExists 1
  ${EndIf}
  StrCpy $PCTHadExistingPoverello 1
FunctionEnd

Function PCTRecoveryReadString
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryReadString
!else
  ReadRegStr $PCTRegistryStringValue HKCU "$PCTCheckKey" "$PCTCheckValue"
!endif
FunctionEnd

Function PCTRecoveryReadDword
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryReadDword
!else
  ReadRegDWORD $PCTRegistryDwordValue HKCU "$PCTCheckKey" "$PCTCheckValue"
!endif
FunctionEnd

Function PCTRecoveryWriteString
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryWriteString
!else
  WriteRegStr HKCU "$PCTCheckKey" "$PCTCheckValue" "$PCTCheckExpected"
!endif
  ${If} ${Errors}
    StrCpy $PCTRecoveryOperationFailed 1
    StrCpy $PCTRecoveryFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTRecoveryWriteDword
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryWriteDword
!else
  WriteRegDWORD HKCU "$PCTCheckKey" "$PCTCheckValue" $PCTCheckExpected
!endif
  ${If} ${Errors}
    StrCpy $PCTRecoveryOperationFailed 1
    StrCpy $PCTRecoveryFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTRecoveryDeleteValue
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryDeleteValue
!else
  DeleteRegValue HKCU "$PCTCheckKey" "$PCTCheckValue"
!endif
  ${If} ${Errors}
    StrCpy $PCTRecoveryOperationFailed 1
    StrCpy $PCTRecoveryFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTRecoveryProbeKey
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryProbeKey
!else
  System::Call 'advapi32::RegOpenKeyExW(p 0x80000001, w "$PCTCheckKey", i 0, i 0x20019, *p .r0)i.r1'
  StrCpy $PCTRegistryStatus $1
  ${If} $1 == 0
    System::Call 'advapi32::RegCloseKey(p r0)i.r1'
    ${If} $1 != 0
      StrCpy $PCTRegistryStatus $1
    ${EndIf}
  ${EndIf}
!endif
FunctionEnd

Function PCTRecoveryDeleteKey
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryDeleteKey
!else
  DeleteRegKey HKCU "$PCTCheckKey"
!endif
  ${If} ${Errors}
    StrCpy $PCTRecoveryOperationFailed 1
    StrCpy $PCTRecoveryFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

Function PCTRecoveryDeleteKeyIfEmpty
  ClearErrors
!ifdef PCT_NSIS_TEST_BACKEND
  Call PCTTestRegistryDeleteKeyIfEmpty
!else
  DeleteRegKey /ifempty HKCU "$PCTCheckKey"
!endif
  ${If} ${Errors}
    StrCpy $PCTRecoveryOperationFailed 1
    StrCpy $PCTRecoveryFailed 1
    ClearErrors
  ${EndIf}
FunctionEnd

!ifdef PCT_NSIS_TEST_BACKEND
Function PCTTestRegistryReadString
  ${If} $PCTTestFailOperation == "read"
  ${OrIf} $PCTTestFailOperation == "readback"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
  ${AndIf} $PCTTestFailValue == $PCTCheckValue
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckKey == "${MANUPRODUCTKEY}"
    ${If} $PCTFakePublisherKeyExists != 1
      SetErrors
      Return
    ${EndIf}
    StrCpy $PCTRegistryStringValue $PCTFakePublisherPath
    Return
  ${ElseIf} $PCTCheckKey == "${UNINSTKEY}"
    ${If} $PCTFakeUninstallKeyExists != 1
      SetErrors
      Return
    ${EndIf}
    ${If} $PCTCheckValue == "DisplayName"
      StrCpy $PCTRegistryStringValue $PCTFakeDisplayName
    ${ElseIf} $PCTCheckValue == "DisplayIcon"
      StrCpy $PCTRegistryStringValue $PCTFakeDisplayIcon
    ${ElseIf} $PCTCheckValue == "DisplayVersion"
      StrCpy $PCTRegistryStringValue $PCTFakeDisplayVersion
    ${ElseIf} $PCTCheckValue == "Publisher"
      StrCpy $PCTRegistryStringValue $PCTFakePublisher
    ${ElseIf} $PCTCheckValue == "InstallLocation"
      StrCpy $PCTRegistryStringValue $PCTFakeInstallLocation
    ${ElseIf} $PCTCheckValue == "UninstallString"
      StrCpy $PCTRegistryStringValue $PCTFakeUninstallString
    ${ElseIf} $PCTCheckValue == "MainBinaryName"
      StrCpy $PCTRegistryStringValue $PCTFakeMainBinaryName
    ${Else}
      SetErrors
    ${EndIf}
    Return
  ${ElseIf} $PCTCheckKey == "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
    ${If} $PCTFakeLegacyUninstallKeyExists != 1
      SetErrors
      Return
    ${EndIf}
    ${If} $PCTCheckValue == "DisplayName"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyDisplayName
    ${ElseIf} $PCTCheckValue == "Publisher"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyPublisher
    ${ElseIf} $PCTCheckValue == "DisplayVersion"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyVersion
    ${ElseIf} $PCTCheckValue == "InstallLocation"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyInstallLocation
    ${ElseIf} $PCTCheckValue == "UninstallString"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyUninstallString
    ${ElseIf} $PCTCheckValue == "DisplayIcon"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyDisplayIcon
    ${ElseIf} $PCTCheckValue == "MainBinaryName"
      StrCpy $PCTRegistryStringValue $PCTFakeLegacyMainBinaryName
    ${Else}
      SetErrors
    ${EndIf}
    Return
  ${ElseIf} $PCTCheckKey == "Software\Sohei IDE (soheidon)\Anthro Bridge"
    ${If} $PCTFakeLegacyPublisherKeyExists != 1
      SetErrors
      Return
    ${EndIf}
    StrCpy $PCTRegistryStringValue $PCTFakeLegacyPublisherPath
    Return
  ${EndIf}
  SetErrors
FunctionEnd

Function PCTTestRegistryReadDword
  ${If} $PCTTestFailOperation == "read"
  ${OrIf} $PCTTestFailOperation == "readback"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
  ${AndIf} $PCTTestFailValue == $PCTCheckValue
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckKey != "${UNINSTKEY}"
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTFakeUninstallKeyExists != 1
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckValue == "NoModify"
    ${If} $PCTFakeNoModifyExists != 1
      SetErrors
      Return
    ${EndIf}
    StrCpy $PCTRegistryDwordValue $PCTFakeNoModify
  ${ElseIf} $PCTCheckValue == "NoRepair"
    ${If} $PCTFakeNoRepairExists != 1
      SetErrors
      Return
    ${EndIf}
    StrCpy $PCTRegistryDwordValue $PCTFakeNoRepair
  ${ElseIf} $PCTCheckValue == "EstimatedSize"
    ${If} $PCTFakeEstimatedSizeExists != 1
      SetErrors
      Return
    ${EndIf}
    StrCpy $PCTRegistryDwordValue $PCTFakeEstimatedSize
  ${Else}
    SetErrors
  ${EndIf}
FunctionEnd

Function PCTTestRegistryWriteString
  ${If} $PCTTestFailOperation == "write"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
  ${AndIf} $PCTTestFailValue == $PCTCheckValue
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTFakePublisherKeyExists 1
    StrCpy $PCTFakePublisherPath $PCTCheckExpected
    Return
  ${ElseIf} $PCTCheckKey == "${UNINSTKEY}"
    StrCpy $PCTFakeUninstallKeyExists 1
    ${If} $PCTCheckValue == "DisplayName"
      StrCpy $PCTFakeDisplayName $PCTCheckExpected
    ${ElseIf} $PCTCheckValue == "DisplayIcon"
      StrCpy $PCTFakeDisplayIcon $PCTCheckExpected
    ${ElseIf} $PCTCheckValue == "DisplayVersion"
      StrCpy $PCTFakeDisplayVersion $PCTCheckExpected
    ${ElseIf} $PCTCheckValue == "Publisher"
      StrCpy $PCTFakePublisher $PCTCheckExpected
    ${ElseIf} $PCTCheckValue == "InstallLocation"
      StrCpy $PCTFakeInstallLocation $PCTCheckExpected
    ${ElseIf} $PCTCheckValue == "UninstallString"
      StrCpy $PCTFakeUninstallString $PCTCheckExpected
    ${ElseIf} $PCTCheckValue == "MainBinaryName"
      StrCpy $PCTFakeMainBinaryName $PCTCheckExpected
    ${Else}
      SetErrors
    ${EndIf}
    Return
  ${EndIf}
  SetErrors
FunctionEnd

Function PCTTestRegistryWriteDword
  ${If} $PCTTestFailOperation == "write"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
  ${AndIf} $PCTTestFailValue == $PCTCheckValue
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckKey != "${UNINSTKEY}"
    SetErrors
    Return
  ${EndIf}
  StrCpy $PCTFakeUninstallKeyExists 1
  ${If} $PCTCheckValue == "NoModify"
    StrCpy $PCTFakeNoModifyExists 1
    StrCpy $PCTFakeNoModify $PCTCheckExpected
  ${ElseIf} $PCTCheckValue == "NoRepair"
    StrCpy $PCTFakeNoRepairExists 1
    StrCpy $PCTFakeNoRepair $PCTCheckExpected
  ${ElseIf} $PCTCheckValue == "EstimatedSize"
    StrCpy $PCTFakeEstimatedSizeExists 1
    StrCpy $PCTFakeEstimatedSize $PCTCheckExpected
  ${Else}
    SetErrors
  ${EndIf}
FunctionEnd

Function PCTTestRegistryDeleteValue
  ${If} $PCTTestFailOperation == "delete-value"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
  ${AndIf} $PCTTestFailValue == $PCTCheckValue
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckKey != "${UNINSTKEY}"
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckValue == "NoModify"
    StrCpy $PCTFakeNoModifyExists 0
    StrCpy $PCTFakeNoModify 0
  ${ElseIf} $PCTCheckValue == "NoRepair"
    StrCpy $PCTFakeNoRepairExists 0
    StrCpy $PCTFakeNoRepair 0
  ${ElseIf} $PCTCheckValue == "EstimatedSize"
    StrCpy $PCTFakeEstimatedSizeExists 0
    StrCpy $PCTFakeEstimatedSize 0
  ${Else}
    SetErrors
  ${EndIf}
FunctionEnd

Function PCTTestRegistryProbeKey
  ${If} $PCTTestFailOperation == "probe"
  ${OrIf} $PCTTestFailOperation == "access"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
    StrCpy $PCTTestFailOperation ""
    StrCpy $PCTRegistryStatus 5
    Return
  ${EndIf}
  ${If} $PCTCheckKey == "${UNINSTKEY}"
    StrCpy $PCTRegistryStatus $PCTFakeUninstallKeyExists
  ${ElseIf} $PCTCheckKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTRegistryStatus $PCTFakePublisherKeyExists
  ${Else}
    StrCpy $PCTRegistryStatus 5
    Return
  ${EndIf}
  ${If} $PCTRegistryStatus == 1
    StrCpy $PCTRegistryStatus 0
  ${Else}
    StrCpy $PCTRegistryStatus 2
  ${EndIf}
FunctionEnd

Function PCTTestRegistryDeleteKey
  ${If} $PCTTestFailOperation == "delete-key"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
  ${If} $PCTCheckKey == "${UNINSTKEY}"
    StrCpy $PCTFakeUninstallKeyExists 0
  ${ElseIf} $PCTCheckKey == "${MANUPRODUCTKEY}"
    StrCpy $PCTFakePublisherKeyExists 0
  ${Else}
    SetErrors
  ${EndIf}
FunctionEnd

Function PCTTestRegistryDeleteKeyIfEmpty
  ; The fake store models the product-key cleanup as a harmless empty-key
  ; removal. It deliberately performs no operating-system registry access.
  ${If} $PCTTestFailOperation == "delete-key"
  ${AndIf} $PCTTestFailKey == $PCTCheckKey
    StrCpy $PCTTestFailOperation ""
    StrCpy $PCTTestFailOperation ""
    SetErrors
    Return
  ${EndIf}
FunctionEnd
!endif

Function PCTVerifyRegStr
  Call PCTRecoveryReadString
  ${If} ${Errors}
    StrCpy $PCTRecoveryFailed 1
    ClearErrors
  ${ElseIf} $PCTRegistryStringValue != $PCTCheckExpected
    StrCpy $PCTRecoveryFailed 1
  ${EndIf}
FunctionEnd

Function PCTVerifySavedDword
  StrCpy $PCTCheckKey "${UNINSTKEY}"
  Call PCTRecoveryReadDword
  ${If} $PCTCheckExpectedExists == 1
    ${If} ${Errors}
      StrCpy $PCTRecoveryFailed 1
      ClearErrors
    ${ElseIf} $PCTRegistryDwordValue != $PCTCheckExpected
      StrCpy $PCTRecoveryFailed 1
    ${EndIf}
  ${ElseIf} ${Errors}
    ClearErrors
  ${Else}
    StrCpy $PCTRecoveryFailed 1
  ${EndIf}
FunctionEnd

Function PCTDeleteRegistrationKeyIfPresent
  Call PCTRecoveryProbeKey
  ${If} $PCTRegistryStatus == 0
    Call PCTRecoveryDeleteKey
    ${If} ${Errors}
      StrCpy $PCTRecoveryFailed 1
      ClearErrors
    ${EndIf}
  ${ElseIf} $PCTRegistryStatus != 2
  ${AndIf} $PCTRegistryStatus != 3
    ; ERROR_ACCESS_DENIED and all unexpected API outcomes fail closed.
    StrCpy $PCTRecoveryFailed 1
  ${EndIf}
FunctionEnd

Function PCTVerifyRegistryKeyAbsent
  Call PCTRecoveryProbeKey
  ${If} $PCTRegistryStatus == 0
    StrCpy $PCTRecoveryFailed 1
  ${ElseIf} $PCTRegistryStatus != 2
  ${AndIf} $PCTRegistryStatus != 3
    StrCpy $PCTRecoveryFailed 1
  ${EndIf}
FunctionEnd

Function PCTVerifySavedPoverelloRegistration
  StrCpy $PCTCheckKey "${MANUPRODUCTKEY}"
  StrCpy $PCTCheckValue ""
  StrCpy $PCTCheckExpected "$PCTSavedPublisherPath"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckKey "${UNINSTKEY}"
  StrCpy $PCTCheckValue "DisplayName"
  StrCpy $PCTCheckExpected "$PCTSavedDisplayName"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "DisplayIcon"
  StrCpy $PCTCheckExpected "$PCTSavedDisplayIcon"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "DisplayVersion"
  StrCpy $PCTCheckExpected "$PCTSavedVersion"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "Publisher"
  StrCpy $PCTCheckExpected "$PCTSavedPublisher"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "InstallLocation"
  StrCpy $PCTCheckExpected "$PCTSavedInstallLocation"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "UninstallString"
  StrCpy $PCTCheckExpected "$PCTSavedUninstallString"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "MainBinaryName"
  StrCpy $PCTCheckExpected "$PCTSavedMainBinaryName"
  Call PCTVerifyRegStr

  StrCpy $PCTCheckValue "NoModify"
  StrCpy $PCTCheckExpected "$PCTSavedNoModify"
  StrCpy $PCTCheckExpectedExists $PCTSavedNoModifyExists
  Call PCTVerifySavedDword
  StrCpy $PCTCheckValue "NoRepair"
  StrCpy $PCTCheckExpected "$PCTSavedNoRepair"
  StrCpy $PCTCheckExpectedExists $PCTSavedNoRepairExists
  Call PCTVerifySavedDword
  StrCpy $PCTCheckValue "EstimatedSize"
  StrCpy $PCTCheckExpected "$PCTSavedEstimatedSize"
  StrCpy $PCTCheckExpectedExists $PCTSavedEstimatedSizeExists
  Call PCTVerifySavedDword
FunctionEnd

Function PCTVerifyNewRegistrationRemoved
  StrCpy $PCTCheckKey "${UNINSTKEY}"
  Call PCTDeleteRegistrationKeyIfPresent
  Call PCTVerifyRegistryKeyAbsent
  StrCpy $PCTCheckKey "${MANUPRODUCTKEY}"
  Call PCTDeleteRegistrationKeyIfPresent
  Call PCTVerifyRegistryKeyAbsent
FunctionEnd

Function PCTVerifyLegacyRegistrationUnchanged
  StrCpy $PCTCheckKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\Anthro Bridge"
  StrCpy $PCTCheckValue "DisplayName"
  StrCpy $PCTCheckExpected "$PCTLegacyDisplayName"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "Publisher"
  StrCpy $PCTCheckExpected "$PCTLegacyPublisher"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "DisplayVersion"
  StrCpy $PCTCheckExpected "$PCTLegacyVersion"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "InstallLocation"
  StrCpy $PCTCheckExpected "$PCTLegacyInstallLocation"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "UninstallString"
  StrCpy $PCTCheckExpected "$PCTLegacyUninstallString"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "MainBinaryName"
  StrCpy $PCTCheckExpected "$PCTLegacyMainBinaryName"
  Call PCTVerifyRegStr
  StrCpy $PCTCheckValue "DisplayIcon"
  Call PCTRecoveryReadString
  ${If} $PCTLegacyDisplayIconExists == 1
    ${If} ${Errors}
      StrCpy $PCTRecoveryFailed 1
      ClearErrors
    ${ElseIf} $PCTRegistryStringValue != $PCTLegacyDisplayIcon
      StrCpy $PCTRecoveryFailed 1
    ${EndIf}
  ${ElseIf} ${Errors}
    ClearErrors
  ${Else}
    StrCpy $PCTRecoveryFailed 1
  ${EndIf}
  StrCpy $PCTCheckKey "Software\Sohei IDE (soheidon)\Anthro Bridge"
  StrCpy $PCTCheckValue ""
  StrCpy $PCTCheckExpected "$PCTLegacyRawPath"
  Call PCTVerifyRegStr
FunctionEnd

Function PCTRestoreRegistrationOnFailure
  ${If} $PCTCleanupDone == 1
    StrCpy $PCTRecoveryVerified 1
    Return
  ${EndIf}
  ${If} $PCTRecoveryAttempted == 1
    Return
  ${EndIf}
  StrCpy $PCTRecoveryAttempted 1
  StrCpy $PCTRecoveryVerified 0
  StrCpy $PCTRecoveryFailed 0
  StrCpy $PCTRecoveryOperationFailed 0
  ${If} $PCTHadExistingPoverello == 1
    StrCpy $PCTCheckKey "${MANUPRODUCTKEY}"
    StrCpy $PCTCheckValue ""
    StrCpy $PCTCheckExpected "$PCTSavedPublisherPath"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckKey "${UNINSTKEY}"
    StrCpy $PCTCheckValue "DisplayName"
    StrCpy $PCTCheckExpected "$PCTSavedDisplayName"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckValue "DisplayIcon"
    StrCpy $PCTCheckExpected "$PCTSavedDisplayIcon"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckValue "DisplayVersion"
    StrCpy $PCTCheckExpected "$PCTSavedVersion"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckValue "Publisher"
    StrCpy $PCTCheckExpected "$PCTSavedPublisher"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckValue "InstallLocation"
    StrCpy $PCTCheckExpected "$PCTSavedInstallLocation"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckValue "UninstallString"
    StrCpy $PCTCheckExpected "$PCTSavedUninstallString"
    Call PCTRecoveryWriteString
    StrCpy $PCTCheckValue "MainBinaryName"
    StrCpy $PCTCheckExpected "$PCTSavedMainBinaryName"
    Call PCTRecoveryWriteString
    ${If} $PCTSavedNoModifyExists == 1
      StrCpy $PCTCheckValue "NoModify"
      StrCpy $PCTCheckExpected "$PCTSavedNoModify"
      Call PCTRecoveryWriteDword
    ${Else}
      StrCpy $PCTCheckKey "${UNINSTKEY}"
      StrCpy $PCTCheckValue "NoModify"
      Call PCTRecoveryDeleteValue
    ${EndIf}
    ${If} $PCTSavedNoRepairExists == 1
      StrCpy $PCTCheckValue "NoRepair"
      StrCpy $PCTCheckExpected "$PCTSavedNoRepair"
      Call PCTRecoveryWriteDword
    ${Else}
      StrCpy $PCTCheckKey "${UNINSTKEY}"
      StrCpy $PCTCheckValue "NoRepair"
      Call PCTRecoveryDeleteValue
    ${EndIf}
    ${If} $PCTSavedEstimatedSizeExists == 1
      StrCpy $PCTCheckValue "EstimatedSize"
      StrCpy $PCTCheckExpected "$PCTSavedEstimatedSize"
      Call PCTRecoveryWriteDword
    ${Else}
      StrCpy $PCTCheckKey "${UNINSTKEY}"
      StrCpy $PCTCheckValue "EstimatedSize"
      Call PCTRecoveryDeleteValue
    ${EndIf}
    Call PCTVerifySavedPoverelloRegistration
  ${Else}
    ; Remove only the Poverello keys created by this attempt, then verify the
    ; product keys are absent. The old Anthro Bridge keys are never deleted.
    Call PCTVerifyNewRegistrationRemoved
    StrCpy $PCTCheckKey "${MANUKEY}"
    Call PCTRecoveryDeleteKeyIfEmpty
    ${If} ${Errors}
      StrCpy $PCTRecoveryFailed 1
      ClearErrors
    ${EndIf}
    ${If} $PCTLegacyUpgradePending == 1
      Call PCTVerifyLegacyRegistrationUnchanged
    ${EndIf}
  ${EndIf}
  ${If} $PCTRecoveryFailed == 0
  ${AndIf} $PCTRecoveryOperationFailed == 0
    StrCpy $PCTRecoveryVerified 1
    StrCpy $PCTCleanupDone 1
  ${EndIf}
FunctionEnd

Function PCTSeedLegacyRegistration
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "$PCTLegacyVersion"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" "$PCTLegacyUninstallString"
  WriteRegStr HKCU "${UNINSTKEY}" "MainBinaryName" "${MAINBINARYNAME}.exe"
  WriteRegStr HKCU "${MANUPRODUCTKEY}" "" "$PCTLegacyPath"
  WriteRegStr HKCU "${MANUPRODUCTKEY}" "LegacyUpgradePath" "$PCTLegacyPath"
  WriteRegDWORD HKCU "${MANUPRODUCTKEY}" "LegacyUpgradePending" 1
  StrCpy $PCTLegacyUpgradePending 1
  StrCpy $INSTDIR $PCTLegacyPath
  StrCpy $UpdateMode 1
FunctionEnd

Function PCTRejectInstallState
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTTestRegistryStatePath != ""
    FileOpen $R8 "$PCTTestRegistryStatePath.rejected" w
    FileWrite $R8 "classifier-rejected$\n"
    FileClose $R8
  ${EndIf}
!endif
  ${If} ${Silent}
    DetailPrint "Anthro Bridge/Poverello installer state is incomplete or conflicting. No install files were intentionally changed. Repair the registered installation or remove the conflict, then rerun setup."
  ${Else}
    MessageBox MB_ICONSTOP|MB_OK "Anthro Bridge/Poverello installer state is incomplete or conflicting. No install files were intentionally changed. Repair the registered installation or remove the conflict, then rerun setup."
  ${EndIf}
  SetErrorLevel 1
  Quit
FunctionEnd

Function PCTClassifyInstallState
  StrCpy $PCTAttemptStarted 0
  StrCpy $PCTRegistrationVerified 0
  StrCpy $PCTRegistrationFailed 0
  StrCpy $PCTCleanupDone 0
  StrCpy $PCTRecoveryAttempted 0
  StrCpy $PCTRecoveryVerified 0
  StrCpy $PCTRecoveryFailed 0
  StrCpy $PCTLegacyDisplayIconExists 0
  StrCpy $PCTHadExistingPoverello 0
  StrCpy $PCTLegacyUpgradePending 0
  StrCpy $PCTLegacyUpgradeComplete 0
  StrCpy $PCTLegacyCleanupMarker 0
  StrCpy $PCTRequireCurrentVersion 0

  !insertmacro PCT_READ_STATE_REG_DWORD $PCTPendingMarker "${MANUPRODUCTKEY}" "LegacyUpgradePending"
  !insertmacro PCT_READ_STATE_REG_DWORD $PCTLegacyCleanupMarker "${MANUPRODUCTKEY}" "LegacyCleanupPending"
  !insertmacro PCT_READ_STATE_REG_STR $PCTPendingPath "${MANUPRODUCTKEY}" "LegacyUpgradePath"
  Call PCTValidateLegacyRegistration
  StrCpy $PCTNewHasRegistry 0
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewPath "${MANUPRODUCTKEY}" ""
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewDisplayName "${UNINSTKEY}" "DisplayName"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewPublisher "${UNINSTKEY}" "Publisher"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewVersion "${UNINSTKEY}" "DisplayVersion"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewInstallLocation "${UNINSTKEY}" "InstallLocation"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewUninstallString "${UNINSTKEY}" "UninstallString"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewDisplayIcon "${UNINSTKEY}" "DisplayIcon"
  !insertmacro PCT_READ_STATE_REG_STR $PCTNewMainBinaryName "${UNINSTKEY}" "MainBinaryName"
!ifdef PCT_NSIS_TEST_BACKEND
  ${If} $PCTTestRegistryReadFailed == 1
    Call PCTRejectInstallState
  ${EndIf}
!endif
  ${If} $PCTPendingMarker == 1
  ${OrIf} $PCTNewPath != ""
  ${OrIf} $PCTNewDisplayName != ""
  ${OrIf} $PCTNewPublisher != ""
  ${OrIf} $PCTNewVersion != ""
  ${OrIf} $PCTNewInstallLocation != ""
  ${OrIf} $PCTNewUninstallString != ""
  ${OrIf} $PCTNewDisplayIcon != ""
  ${OrIf} $PCTNewMainBinaryName != ""
    StrCpy $PCTNewHasRegistry 1
  ${EndIf}
  Call PCTValidatePoverelloRegistration

  ${If} $PCTNewValid == 1
    ${If} $PCTLegacyHasRegistry == 1
      ${If} $PCTPendingMarker != 1
        Call PCTRejectInstallState
      ${EndIf}
      StrCpy $PCTNormalizeInput $PCTPendingPath
      Call PCTNormalizeAbsolutePath
      ${If} $PCTNormalizeOutput == ""
      ${OrIf} $PCTNormalizeOutput != $PCTNewPath
        Call PCTRejectInstallState
      ${EndIf}
      ${If} $PCTLegacyCleanupMarker == 1
        Call PCTValidateLegacyCleanupResidue
        ${If} $PCTLegacyCleanupReadbackFailed == 1
          Call PCTRejectInstallState
        ${EndIf}
        StrCpy $PCTLegacyPath $PCTNewPath
      ${Else}
        ${If} $PCTLegacyValid != 1
        ${OrIf} $PCTNormalizeOutput != $PCTLegacyPath
          Call PCTRejectInstallState
        ${EndIf}
      ${EndIf}
      StrCpy $PCTLegacyUpgradePending 1
      StrCpy $INSTDIR $PCTLegacyPath
      StrCpy $UpdateMode 1
    ${ElseIf} $PCTPendingMarker == 1
      StrCpy $PCTNormalizeInput $PCTPendingPath
      Call PCTNormalizeAbsolutePath
      ${If} $PCTNormalizeOutput != $PCTNewPath
        Call PCTRejectInstallState
      ${EndIf}
      ; Recovery after the new registration was verified and the old keys
      ; removed, but before the pending marker was cleared.
      StrCpy $PCTLegacyUpgradePending 1
      StrCpy $INSTDIR $PCTNewPath
      StrCpy $UpdateMode 1
      Call PCTSnapshotPoverelloRegistration
    ${Else}
      StrCpy $INSTDIR $PCTNewPath
      Call PCTSnapshotPoverelloRegistration
    ${EndIf}
    StrCpy $PCTAttemptStarted 1
    Return
  ${EndIf}

  ${If} $PCTNewHasRegistry == 1
    ; Resume a migration only when the pending marker, every already-seeded
    ; value, and the independently validated old registration agree.
    ${If} $PCTPendingMarker != 1
    ${OrIf} $PCTLegacyValid != 1
      Call PCTRejectInstallState
    ${EndIf}
    StrCpy $PCTNormalizeInput $PCTPendingPath
    Call PCTNormalizeAbsolutePath
    ${If} $PCTNormalizeOutput != $PCTLegacyPath
      Call PCTRejectInstallState
    ${EndIf}
    ${If} $PCTNewPath != ""
    ${AndIf} $PCTNewPath != $PCTLegacyPath
      Call PCTRejectInstallState
    ${EndIf}
    ${If} $PCTNewDisplayName != ""
    ${AndIf} $PCTNewDisplayName != "${PRODUCTNAME}"
      Call PCTRejectInstallState
    ${EndIf}
    ${If} $PCTNewPublisher != ""
    ${AndIf} $PCTNewPublisher != "${MANUFACTURER}"
      Call PCTRejectInstallState
    ${EndIf}
    ${If} $PCTNewVersion != ""
    ${AndIf} $PCTNewVersion != $PCTLegacyVersion
      Call PCTRejectInstallState
    ${EndIf}
    ${If} $PCTNewMainBinaryName != ""
    ${AndIf} $PCTNewMainBinaryName != "${MAINBINARYNAME}.exe"
      Call PCTRejectInstallState
    ${EndIf}
    ${If} $PCTNewUninstallString != ""
    ${AndIf} $PCTNewUninstallString != $PCTLegacyUninstallString
      Call PCTRejectInstallState
    ${EndIf}
    StrCpy $PCTLegacyUpgradePending 1
    Call PCTSeedLegacyRegistration
    StrCpy $PCTAttemptStarted 1
    Return
  ${EndIf}

  ${If} $PCTLegacyHasRegistry == 1
    ${If} $PCTLegacyValid != 1
      Call PCTRejectInstallState
    ${EndIf}
    StrCpy $PCTLegacyUpgradePending 1
    Call PCTSeedLegacyRegistration
    StrCpy $PCTAttemptStarted 1
    Return
  ${EndIf}

  ${If} $PCTLegacyOrphanExecutable == 1
    Call PCTRejectInstallState
  ${EndIf}
  ; No old registration, no orphaned legacy executable, and no Poverello
  ; registration: this is a clean install.
  StrCpy $PCTAttemptStarted 1
FunctionEnd

Function ValidateLegacyUpgradeDir
  ${If} $PCTLegacyUpgradePending == 1
    StrCpy $PCTNormalizeInput $INSTDIR
    Call PCTNormalizeAbsolutePath
    ${If} $PCTNormalizeOutput != $PCTLegacyPath
      ${If} ${Silent}
        DetailPrint "In-place migration requires the verified existing Anthro Bridge installation directory."
      ${Else}
        MessageBox MB_ICONSTOP|MB_OK "For an in-place upgrade, Poverello must use the verified existing Anthro Bridge folder:$\r$\n$PCTLegacyPath"
      ${EndIf}
      Abort
    ${EndIf}
  ${EndIf}
FunctionEnd

Function .onGUIEnd
  ${If} $PCTAttemptStarted == 1
  ${AndIf} $PCTRegistrationVerified != 1
  ${AndIf} $PCTCleanupDone != 1
    Call PCTRestoreRegistrationOnFailure
  ${EndIf}
  ${If} $PCTRegistrationVerified == 1
  ${AndIf} $PCTLegacyUpgradePending == 1
  ${AndIf} $PCTLegacyUpgradeComplete == 1
    !insertmacro PCT_DELETE_STATE_REG_VALUE "${MANUPRODUCTKEY}" "LegacyUpgradeComplete"
    !insertmacro PCT_DELETE_STATE_REG_VALUE "${MANUPRODUCTKEY}" "LegacyCleanupPending"
    !insertmacro PCT_DELETE_STATE_REG_VALUE "${MANUPRODUCTKEY}" "LegacyUpgradePath"
    ; Keep the transaction pending marker until every other marker cleanup is
    ; attempted, so interrupted marker cleanup can be recognized on retry.
    !insertmacro PCT_DELETE_STATE_REG_VALUE "${MANUPRODUCTKEY}" "LegacyUpgradePending"
  ${EndIf}
FunctionEnd

Function un.PCTScanForReparsePoint
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  StrCpy $2 $PCTUninstallScanPath
  System::Call 'kernel32::GetFileAttributesW(w "$2")i.r3'
  ${If} $3 == -1
  ${OrIf} $3 == 4294967295
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_scan_done
  ${EndIf}
  IntOp $4 $3 & 0x400
  ${If} $4 != 0
    StrCpy $PCTUninstallReparseFound 1
    Goto pct_un_scan_done
  ${EndIf}
  IntOp $4 $3 & 0x10
  ${If} $4 == 0
    Goto pct_un_scan_done
  ${EndIf}
  FindFirst $0 $1 "$2\*"
  ${If} $0 == -1
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_scan_done
  ${EndIf}
pct_un_scan_next:
  ${If} $1 == ""
    Goto pct_un_scan_close
  ${EndIf}
  ${If} $1 != "."
  ${AndIf} $1 != ".."
    StrCpy $PCTUninstallScanPath "$2\$1"
    Call un.PCTScanForReparsePoint
    ${If} $PCTUninstallDataFailed == 1
    ${OrIf} $PCTUninstallReparseFound == 1
      Goto pct_un_scan_close
    ${EndIf}
  ${EndIf}
  FindNext $0 $1
  Goto pct_un_scan_next
pct_un_scan_close:
  FindClose $0
pct_un_scan_done:
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

Function un.PCTRemoveDataNode
  ; Recheck every node immediately before descending/deleting it. Do not use
  ; NSIS's recursive RmDir here: it can follow a reparse point created after
  ; the preflight scan and escape the exact approved data root.
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  StrCpy $2 $PCTUninstallScanPath
  System::Call 'kernel32::GetFileAttributesW(w "$2")i.r3'
  ${If} $3 == -1
  ${OrIf} $3 == 4294967295
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_remove_node_done
  ${EndIf}
  IntOp $4 $3 & 0x400
  ${If} $4 != 0
    StrCpy $PCTUninstallReparseFound 1
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_remove_node_done
  ${EndIf}
  IntOp $4 $3 & 0x10
  ${If} $4 == 0
    ClearErrors
    Delete "$2"
    ${If} ${Errors}
      StrCpy $PCTUninstallDataFailed 1
    ${EndIf}
    Goto pct_un_remove_node_done
  ${EndIf}
  FindFirst $0 $1 "$2\*"
  ${If} $0 == -1
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_remove_node_done
  ${EndIf}
pct_un_remove_node_next:
  ${If} $1 == ""
    Goto pct_un_remove_node_close
  ${EndIf}
  ${If} $1 != "."
  ${AndIf} $1 != ".."
    StrCpy $PCTUninstallScanPath "$2\$1"
    Call un.PCTRemoveDataNode
    ${If} $PCTUninstallDataFailed == 1
      Goto pct_un_remove_node_close
    ${EndIf}
  ${EndIf}
  FindNext $0 $1
  Goto pct_un_remove_node_next
pct_un_remove_node_close:
  FindClose $0
  ${If} $PCTUninstallDataFailed == 0
    ClearErrors
    RmDir "$2"
    ${If} ${Errors}
      StrCpy $PCTUninstallDataFailed 1
    ${EndIf}
  ${EndIf}
pct_un_remove_node_done:
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

Function un.PCTDeleteStableUserData
  StrCpy $PCTUninstallDataFailed 0
  StrCpy $PCTUninstallReparseFound 0
!ifdef PCT_NSIS_TEST_BACKEND
  ; Only the compile-time-only test harness may select a disposable fixture.
  ReadEnvStr $PCTUninstallDataPath "PCT_NSIS_TEST_DATA_PATH"
  ${If} $PCTTestFailOperation == "data-delete"
    StrCpy $PCTTestFailOperation ""
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_data_report
  ${EndIf}
!else
  StrCpy $PCTUninstallDataPath "$APPDATA\Poverello"
!endif
  ${If} $PCTUninstallDataPath == ""
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_data_report
  ${EndIf}
  StrCpy $PCTUninstallScanPath $PCTUninstallDataPath
  ; System::Call may render INVALID_FILE_ATTRIBUTES as signed -1 or unsigned
  ; 4294967295. Handle both before using the directory attribute bit.
  System::Call 'kernel32::GetFileAttributesW(w "$PCTUninstallDataPath")i.r0'
  ${If} $0 == -1
  ${OrIf} $0 == 4294967295
    System::Call 'kernel32::GetLastError()i.r1'
    ${If} $1 == 2
    ${OrIf} $1 == 3
      Return
    ${Else}
      StrCpy $PCTUninstallDataFailed 1
      Goto pct_un_data_report
    ${EndIf}
  ${EndIf}
  IntOp $1 $0 & 0x10
  ${If} $1 == 0
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_data_report
  ${EndIf}
  Call un.PCTScanForReparsePoint
  ${If} $PCTUninstallDataFailed == 1
  ${OrIf} $PCTUninstallReparseFound == 1
    StrCpy $PCTUninstallDataFailed 1
    Goto pct_un_data_report
  ${EndIf}
  ; The preflight walker uses this global for recursion. Restore the exact
  ; approved root before starting the guarded deletion walk.
  StrCpy $PCTUninstallScanPath $PCTUninstallDataPath
  Call un.PCTRemoveDataNode
pct_un_data_report:
  ${If} $PCTUninstallDataFailed == 1
!ifdef PCT_NSIS_TEST_BACKEND
    ; The post-hook writes the terminal status only after registration cleanup.
    ReadEnvStr $8 "PCT_NSIS_TEST_RESULT_PATH"
    ${If} $8 != ""
      FileOpen $7 $8 a
      FileWrite $7 "data-cleanup-failed=1|reparse=$PCTUninstallReparseFound|path=$PCTUninstallDataPath$\n"
      FileClose $7
    ${EndIf}
!endif
  ${EndIf}
FunctionEnd
!macroend
