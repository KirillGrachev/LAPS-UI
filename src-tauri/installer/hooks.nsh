/*
 * Хуки NSIS-шаблона Tauri: закрываем запущенное приложение до установки
 * и удаления. Без этого exe и файлы кэша залочены живым процессом, и
 * повторная установка падает ошибкой «файл используется».
 *
 * Сначала мягкое завершение (WM_CLOSE): приложение успевает сбросить
 * отложенные операции (запись конфига, flush журнала аудита).
 * Затем страховочное жёсткое — если окно не закрылось само. Обе команды
 * безопасны при незапущенном приложении: ненулевой код игнорируется.
 */

!macro NSIS_HOOK_PREINIT
  /*
   * Вместо стоковой строки футера «Nullsoft Install System v3.11» —
   * наш нейминг: футер страниц установщика показывает продукт, а не NSIS.
   */
  BrandingText "KMAruda LAPS · (c) КМАруда, 2026"
  /*
   * Компания в version-info файла (вкладка «Подробно» в свойствах).
   * На поле «Издатель» в UAC не влияет: то берётся только из подписи.
   */
  VIAddVersionKey "CompanyName" "КМАруда"
  VIAddVersionKey "ProductName" "KMAruda LAPS"
  VIAddVersionKey "ProductVersion" "${VERSION}"
  VIAddVersionKey "FileDescription" "Чтение паролей LAPS из Active Directory для службы поддержки"
!macroend

!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'taskkill /IM "KMAruda LAPS.exe"'
  Pop $0
  Sleep 1500
  nsExec::ExecToLog 'taskkill /F /IM "KMAruda LAPS.exe"'
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'taskkill /IM "KMAruda LAPS.exe"'
  Pop $0
  Sleep 1500
  nsExec::ExecToLog 'taskkill /F /IM "KMAruda LAPS.exe"'
  Pop $0
!macroend
