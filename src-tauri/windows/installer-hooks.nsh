; Funo Studio installer choices. These are intentionally explicit rather than
; silently changing PATH or guessing whether the learner is a beginner.
!macro NSIS_HOOK_POSTINSTALL
  IfSilent funo_choices_done

  MessageBox MB_YESNO|MB_ICONQUESTION \
    "Добавить команду 'funo' в пользовательский PATH?$\r$\n$\r$\nПосле этого Funo можно запускать из нового окна Terminal, PowerShell или cmd." \
    IDYES funo_path_yes IDNO funo_path_done
  funo_path_yes:
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --install-path'
  funo_path_done:

  MessageBox MB_YESNO|MB_ICONQUESTION \
    "Я новичок — включить пошаговое обучение при первом запуске?$\r$\n$\r$\nВыбор можно изменить позже в настройках Studio." \
    IDYES funo_beginner_yes IDNO funo_beginner_no
  funo_beginner_yes:
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --installer-beginner=true'
    Goto funo_choices_done
  funo_beginner_no:
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --installer-beginner=false'

  funo_choices_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 funo_uninstall_done
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-path'
  funo_uninstall_done:
!macroend
