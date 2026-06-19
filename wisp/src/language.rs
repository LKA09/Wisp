#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    English,
    Korean,
}

/// Detect user language from input text.
/// Korean Hangul characters → Korean; otherwise English.
pub fn detect(text: &str) -> Language {
    let has_hangul = text.chars().any(|c| {
        matches!(c,
            '\u{AC00}'..='\u{D7A3}' | // Hangul syllables
            '\u{1100}'..='\u{11FF}' | // Hangul jamo
            '\u{3130}'..='\u{318F}'   // Hangul compatibility jamo
        )
    });

    if has_hangul {
        Language::Korean
    } else {
        Language::English
    }
}

/// Return English or Korean message depending on the detected language.
pub fn msg(lang: &Language, en: &str, ko: &str) -> String {
    match lang {
        Language::English => en.to_string(),
        Language::Korean => ko.to_string(),
    }
}
