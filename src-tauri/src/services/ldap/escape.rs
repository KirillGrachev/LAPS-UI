//! Экранирование значений LDAP-фильтров (RFC 4515, §3).
//!
//! Любой пользовательский ввод (имя компьютера, FQDN), попадающий в фильтр
//! поиска, обязан быть экранирован: символы `* ( ) \ NUL` меняют смысл
//! фильтра и позволяют устроить LDAP-инъекцию (например, `*)(|(cn=*`
//! превращает точечный поиск в дамп каталога).
//!
//! В дополнение к экранированию вызывающий код валидирует сам набор
//! допустимых символов имени — это второй эшелон защиты.

/// Экранирует значение для подстановки в LDAP-фильтр.
pub fn escape_filter_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '*' => out.push_str("\\2a"),
            '(' => out.push_str("\\28"),
            ')' => out.push_str("\\29"),
            '\\' => out.push_str("\\5c"),
            '\0' => out.push_str("\\00"),
            '/' => out.push_str("\\2f"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape_filter_value;

    #[test]
    /// `|` по RFC 4515 не экранируется: внутри значения ассерции он
    /// литерален (парсер значения завершает только незакрытая `)`).
    fn escapes_rfc4515_specials() {
        assert_eq!(escape_filter_value("ws01"), "ws01");
        assert_eq!(escape_filter_value("KMA-W01"), "KMA-W01");
        assert_eq!(escape_filter_value("*)(|(cn=*"), "\\2a\\29\\28|\\28cn=\\2a");
        assert_eq!(escape_filter_value("a\\b"), "a\\5cb");
        assert_eq!(escape_filter_value("host/pc"), "host\\2fpc");
        assert_eq!(escape_filter_value("nul\0here"), "nul\\00here");
    }

    #[test]
    /// После экранирования «сырых» звёздочек не остаётся: все — за \2a.
    fn escaped_value_has_no_unescaped_wildcards() {
        let escaped = escape_filter_value("ws*01(x)");
        let raw: Vec<char> = escaped.chars().collect();
        for (i, ch) in raw.iter().enumerate() {
            if *ch == '*' {
                panic!("неэкранированная звёздочка на позиции {i}: {escaped}");
            }
        }
    }
}
