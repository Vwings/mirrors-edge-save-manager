pub const ENGLISH: &str = "en";
pub const SIMPLIFIED_CHINESE: &str = "zh-CN";

pub fn supported(language: &str) -> bool {
    matches!(language, ENGLISH | SIMPLIFIED_CHINESE)
}

pub fn initial_language() -> &'static str {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

        let mut buffer = [0u16; 85];
        let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
        if length > 0 {
            let locale = String::from_utf16_lossy(&buffer[..length as usize - 1]);
            if locale.eq_ignore_ascii_case("zh-CN") || locale.starts_with("zh-") {
                return SIMPLIFIED_CHINESE;
            }
        }
    }
    ENGLISH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_supported_languages() {
        assert!(supported(ENGLISH));
        assert!(supported(SIMPLIFIED_CHINESE));
        assert!(!supported("fr"));
    }
}
