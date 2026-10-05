//! Запасной уровень дешифровки Windows LAPS: системный модуль LAPS.
//!
//! Полноценная нативная дешифровка `msLAPS-EncryptedPassword` требует
//! клиентского вызова RPC MS-GKDI (`GetKey`) к контроллеру домена с
//! последующим DPAPI-NG-снятием защиты полученным seed-ключом — отдельный
//! большой проект (NDR/SD-маршаллинг), верифицируемый только на живом
//! домене. Пока его нет, второй уровень — системный модуль Windows LAPS
//! (cmdlet `Get-LapsADPassword`), выполняющий ровно этот маршрут изнутри.
//!
//! Вызов повторяет проверенный первой версией приложения контур:
//! * параметр `-Server` передаётся ТОЛЬКО если cmdlet его поддерживает
//!   (проверка по `(Get-Command).Parameters`) — иначе биндинг-ошибка;
//! * Identity — имя компьютера (CN/sAMAccountName без `$`);
//! * пароль проходит через SecureString и затирается внутри скрипта
//!   (`ZeroFreeBSTR`), наружу уходит один JSON в stdout;
//! * окно процесса скрыто; stderr не теряется: причина отказа попадает
//!   в tracing-лог и в подробности аудита, но не в UI.
//!
//! Если модуль не установлен или прав нет — молча `None`: поле пароля
//! покажет статус «не расшифрован».

use std::os::windows::process::CommandExt;
use std::process::Command;

use tracing::warn;

use super::parse::{parse_windows_plain_json, WindowsPlainPassword};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Скрипт-оракул: один JSON `{"n":...,"p":...}` в stdout, причины отказов —
/// в stderr, код выхода всегда 0 (ошибки различаем по пустому stdout).
const PS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
try {
  $id = $env:LAPS_IDENTITY
  $srv = $env:LAPS_SERVER
  $p = @{ Identity = $id; ErrorAction = 'Stop' }
  $cp = (Get-Command Get-LapsADPassword -ErrorAction Stop).Parameters
  if ($srv -and $cp.ContainsKey('Server')) { $p['Server'] = $srv }
  $d = Get-LapsADPassword @p
  if ($null -eq $d -or $null -eq $d.Password) { exit 0 }
  $b = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($d.Password)
  $pw = [System.Runtime.InteropServices.Marshal]::PtrToStringBSTR($b)
  [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($b)
  [Console]::Out.Write((@{ n = $d.AccountName; p = $pw } | ConvertTo-Json -Compress))
} catch {
  [Console]::Error.Write($_.Exception.Message)
}
"#;

/// Пытается получить расшифрованный пароль системным модулем Windows LAPS.
///
/// Блокирующая операция (process spawn) — вызывающий код оборачивает
/// её в `spawn_blocking`.
/// Имя компьютера уже прошло валидацию charset'а на входе команды.
/// Сервер извлекаем из LDAP URL и дополнительно ограничиваем набором.
/// `-EncodedCommand`: никаких проблем с кавычками и кодовыми страницами.
pub fn decrypt_via_laps_module(
    computer_name: &str,
    ldap_url: &str,
) -> Option<WindowsPlainPassword> {
    let server = ldap_url
        .split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(rest))
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_string();
    let server = if server.is_empty()
        || server
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
    {
        String::new()
    } else {
        server
    };

    let script =
        format!("$env:LAPS_IDENTITY='{computer_name}';$env:LAPS_SERVER='{server}';{PS_SCRIPT}");
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64_encode(&utf16);

    let output = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-EncodedCommand")
        .arg(encoded)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        warn!(
            reason = %stderr.trim(),
            "Get-LapsADPassword: отказ (модуль не установлен, нет прав или нет маршрута к DC)"
        );
    }
    if !output.status.success() {
        warn!(
            "Get-LapsADPassword: процесс завершился с кодом {:?}",
            output.status.code()
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|line| line.trim().starts_with('{'))?;
    parse_windows_plain_json(line.trim())
}

/// Base64 для `-EncodedCommand` (UTF-16LE скрипта).
fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
