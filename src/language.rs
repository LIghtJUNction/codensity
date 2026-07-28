use std::path::Path;

/// One canonical language and its recognized extensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageSpec {
    /// Stable serialized language name.
    pub name: &'static str,
    /// Case-sensitive filename extensions without a leading dot.
    pub extensions: &'static [&'static str],
}

/// Central protocol-v1 language table in canonical output order.
pub const LANGUAGES: &[LanguageSpec] = &[
    LanguageSpec {
        name: "Rust",
        extensions: &["rs"],
    },
    LanguageSpec {
        name: "C",
        extensions: &["c"],
    },
    LanguageSpec {
        name: "C Header",
        extensions: &["h"],
    },
    LanguageSpec {
        name: "C++",
        extensions: &["cc", "cpp", "cxx"],
    },
    LanguageSpec {
        name: "C++ Header",
        extensions: &["hh", "hpp", "hxx"],
    },
    LanguageSpec {
        name: "Assembly",
        extensions: &["S", "s", "asm"],
    },
    LanguageSpec {
        name: "Python",
        extensions: &["py"],
    },
    LanguageSpec {
        name: "Go",
        extensions: &["go"],
    },
    LanguageSpec {
        name: "JavaScript",
        extensions: &["js", "mjs", "cjs"],
    },
    LanguageSpec {
        name: "JSX",
        extensions: &["jsx"],
    },
    LanguageSpec {
        name: "TypeScript",
        extensions: &["ts", "mts", "cts"],
    },
    LanguageSpec {
        name: "TSX",
        extensions: &["tsx"],
    },
    LanguageSpec {
        name: "Java",
        extensions: &["java"],
    },
    LanguageSpec {
        name: "Kotlin",
        extensions: &["kt", "kts"],
    },
    LanguageSpec {
        name: "Swift",
        extensions: &["swift"],
    },
    LanguageSpec {
        name: "Objective-C",
        extensions: &["m"],
    },
    LanguageSpec {
        name: "Objective-C++",
        extensions: &["mm"],
    },
    LanguageSpec {
        name: "C#",
        extensions: &["cs"],
    },
    LanguageSpec {
        name: "Ruby",
        extensions: &["rb"],
    },
    LanguageSpec {
        name: "PHP",
        extensions: &["php"],
    },
    LanguageSpec {
        name: "Shell",
        extensions: &["sh", "bash", "zsh"],
    },
    LanguageSpec {
        name: "Lua",
        extensions: &["lua"],
    },
    LanguageSpec {
        name: "Zig",
        extensions: &["zig"],
    },
    LanguageSpec {
        name: "Scala",
        extensions: &["scala", "sc"],
    },
    LanguageSpec {
        name: "Haskell",
        extensions: &["hs", "lhs"],
    },
];

/// Returns the canonical language index for a recognized source path.
///
/// Extension matching is case-sensitive because changing filename semantics
/// changes protocol comparability.
#[must_use]
pub fn language_for_path(path: &Path) -> Option<usize> {
    let extension = path.extension()?.to_str()?;
    LANGUAGES
        .iter()
        .position(|language| language.extensions.contains(&extension))
}
