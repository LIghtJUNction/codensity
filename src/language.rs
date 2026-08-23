use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// One canonical language and its recognized path keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageSpec {
    /// Stable serialized language name.
    pub name: &'static str,
    /// Case-sensitive filename extensions without a leading dot.
    pub extensions: &'static [&'static str],
    /// Case-sensitive exact basenames, including extensionless build files.
    pub filenames: &'static [&'static str],
}

macro_rules! language {
    ($name:literal, [$($ext:literal),* $(,)?]) => {
        LanguageSpec {
            name: $name,
            extensions: &[$($ext),*],
            filenames: &[],
        }
    };
    ($name:literal, [$($ext:literal),* $(,)?], [$($file:literal),* $(,)?]) => {
        LanguageSpec {
            name: $name,
            extensions: &[$($ext),*],
            filenames: &[$($file),*],
        }
    };
}

/// Central protocol-v2 language table in canonical output order.
///
/// The first 25 entries keep their protocol-v1 names and relative order. Later
/// entries are the v2 expansion. Extension matching stays case-sensitive.
///
/// Ambiguous extensions keep a single owner so the ledger stays deterministic:
/// `.m` is Objective-C (not MATLAB), `.sc` is Scala (not Scheme), and `.v` is
/// Verilog (not Coq or V).
pub const LANGUAGES: &[LanguageSpec] = &[
    language!("Rust", ["rs"]),
    language!("C", ["c"]),
    language!("C Header", ["h"]),
    language!("C++", ["cc", "cpp", "cxx", "c++", "cppm", "ixx", "ccm"]),
    language!("C++ Header", ["hh", "hpp", "hxx", "h++", "ipp", "tpp"]),
    language!("Assembly", ["S", "s", "asm", "nasm"]),
    language!("Python", ["py", "pyi", "pyw"]),
    language!("Go", ["go"]),
    language!("JavaScript", ["js", "mjs", "cjs"]),
    language!("JSX", ["jsx"]),
    language!("TypeScript", ["ts", "mts", "cts"]),
    language!("TSX", ["tsx"]),
    language!("Java", ["java"]),
    language!("Kotlin", ["kt", "kts"]),
    language!("Swift", ["swift"]),
    language!("Objective-C", ["m"]),
    language!("Objective-C++", ["mm"]),
    language!("C#", ["cs", "csx"]),
    language!(
        "Ruby",
        ["rb", "rake"],
        ["Gemfile", "Rakefile", "Guardfile", "Capfile"]
    ),
    language!("PHP", ["php", "phps"]),
    language!(
        "Shell",
        ["sh", "bash", "zsh", "ksh"],
        [".bashrc", ".bash_profile", ".zshrc"]
    ),
    language!("Lua", ["lua"]),
    language!("Zig", ["zig"]),
    language!("Scala", ["scala", "sc"]),
    language!("Haskell", ["hs", "lhs"]),
    language!("Ada", ["adb", "ads"]),
    language!("Agda", ["agda"]),
    language!("AppleScript", ["applescript"]),
    language!("AutoHotkey", ["ahk"]),
    language!("AWK", ["awk"]),
    language!("Batch", ["bat", "cmd"]),
    language!("C3", ["c3"]),
    language!("CMake", ["cmake"], ["CMakeLists.txt"]),
    language!("COBOL", ["cob", "cbl"]),
    language!("CSS", ["css"]),
    language!("CUDA", ["cu", "cuh"]),
    language!("Clojure", ["clj", "cljs", "cljc"]),
    language!("CoffeeScript", ["coffee"]),
    language!("Common Lisp", ["lisp", "lsp"]),
    language!("Crystal", ["cr"]),
    language!("Cython", ["pyx", "pxd", "pxi"]),
    language!("D", ["d"]),
    language!("Dart", ["dart"]),
    language!(
        "Dockerfile",
        ["dockerfile"],
        ["Dockerfile", "Containerfile", "dockerfile"]
    ),
    language!("Elixir", ["ex", "exs"], ["mix.exs"]),
    language!("Elm", ["elm"]),
    language!("Elvish", ["elv"]),
    language!("Emacs Lisp", ["el"]),
    language!("Erlang", ["erl", "hrl"], ["rebar.config"]),
    language!("F#", ["fs", "fsx", "fsi"]),
    language!("F*", ["fst", "fsti"]),
    language!("Fish", ["fish"]),
    language!("Forth", ["fth", "4th"]),
    language!(
        "Fortran",
        [
            "f", "for", "f90", "f95", "f03", "f08", "F", "F90", "F95", "F03", "F08"
        ]
    ),
    language!("GDScript", ["gd"]),
    language!(
        "GLSL",
        ["glsl", "vert", "frag", "comp", "geom", "tesc", "tese"]
    ),
    language!("Gleam", ["gleam"]),
    language!("GraphQL", ["graphql", "gql"]),
    language!("Groovy", ["groovy", "gvy"], ["Jenkinsfile"]),
    language!("HCL", ["hcl", "tf", "tfvars"]),
    language!("Haxe", ["hx"]),
    language!("Idris", ["idr"]),
    language!("Janet", ["janet"]),
    language!("Julia", ["jl"]),
    language!("Just", ["just"], ["Justfile", "justfile"]),
    language!("Lean", ["lean"]),
    language!("Less", ["less"]),
    language!("LLVM IR", ["ll"]),
    language!(
        "Makefile",
        ["mak", "mk"],
        ["Makefile", "makefile", "GNUmakefile"]
    ),
    language!("Meson", [], ["meson.build", "meson.options"]),
    language!("Nim", ["nim", "nims"]),
    language!("Nix", ["nix"]),
    language!("OCaml", ["ml", "mli"]),
    language!("Odin", ["odin"]),
    language!("Pascal", ["pas", "pp"]),
    language!("Perl", ["pl", "pm", "t"]),
    language!("Pony", ["pony"]),
    language!("PowerShell", ["ps1", "psm1", "psd1"]),
    language!("Prolog", ["pro", "prolog"]),
    language!("Protocol Buffers", ["proto"]),
    language!("PureScript", ["purs"]),
    language!("R", ["r", "R"]),
    language!("Racket", ["rkt"]),
    language!("Raku", ["raku", "p6", "rakumod"]),
    language!("Razor", ["cshtml", "razor"]),
    language!("ReScript", ["res", "resi"]),
    language!("Roc", ["roc"]),
    language!("SCSS", ["scss"]),
    language!("SQL", ["sql"]),
    language!("Sass", ["sass"]),
    language!("Scheme", ["scm", "ss"]),
    language!("Smalltalk", ["st"]),
    language!("SML", ["sml"]),
    language!("Solidity", ["sol"]),
    language!(
        "Starlark",
        ["bzl", "star"],
        ["BUILD", "BUILD.bazel", "WORKSPACE", "WORKSPACE.bazel"]
    ),
    language!("Stylus", ["styl"]),
    language!("Svelte", ["svelte"]),
    language!("SystemVerilog", ["sv", "svh"]),
    language!("Tcl", ["tcl"]),
    language!("Thrift", ["thrift"]),
    language!("VB.NET", ["vb"]),
    language!("Vala", ["vala", "vapi"]),
    language!("Verilog", ["v", "vh"]),
    language!("VHDL", ["vhd", "vhdl"]),
    language!("Vim Script", ["vim"]),
    language!("Vue", ["vue"]),
    language!("WebAssembly", ["wat"]),
    language!("Wolfram", ["wl", "wls"]),
];

fn extension_index(extension: &str) -> Option<usize> {
    static MAP: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    MAP.get_or_init(extension_map).get(extension).copied()
}

fn filename_index(filename: &str) -> Option<usize> {
    static MAP: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    MAP.get_or_init(filename_map).get(filename).copied()
}

fn extension_map() -> HashMap<&'static str, usize> {
    let mut map = HashMap::new();
    for (index, language) in LANGUAGES.iter().enumerate() {
        for extension in language.extensions {
            map.insert(*extension, index);
        }
    }
    map
}

fn filename_map() -> HashMap<&'static str, usize> {
    let mut map = HashMap::new();
    for (index, language) in LANGUAGES.iter().enumerate() {
        for filename in language.filenames {
            map.insert(*filename, index);
        }
    }
    map
}

/// Returns the canonical language index for a recognized source path.
///
/// Exact basenames win over extensions so `CMakeLists.txt` is CMake rather than
/// an unrecognized `.txt` file. Extension matching is case-sensitive because
/// changing filename semantics changes protocol comparability.
#[must_use]
pub fn language_for_path(path: &Path) -> Option<usize> {
    if let Some(filename) = path.file_name().and_then(|name| name.to_str())
        && let Some(index) = filename_index(filename)
    {
        return Some(index);
    }
    extension_index(path.extension()?.to_str()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    const PROTOCOL_V1_NAMES: &[&str] = &[
        "Rust",
        "C",
        "C Header",
        "C++",
        "C++ Header",
        "Assembly",
        "Python",
        "Go",
        "JavaScript",
        "JSX",
        "TypeScript",
        "TSX",
        "Java",
        "Kotlin",
        "Swift",
        "Objective-C",
        "Objective-C++",
        "C#",
        "Ruby",
        "PHP",
        "Shell",
        "Lua",
        "Zig",
        "Scala",
        "Haskell",
    ];

    #[test]
    fn language_table_should_keep_protocol_v1_prefix_in_canonical_order() {
        assert!(LANGUAGES.len() > PROTOCOL_V1_NAMES.len());
        for (index, name) in PROTOCOL_V1_NAMES.iter().enumerate() {
            assert_eq!(LANGUAGES[index].name, *name);
        }
    }

    #[test]
    fn language_table_should_keep_unique_names_extensions_and_filenames() {
        let mut names = HashSet::new();
        let mut extensions = HashSet::new();
        let mut filenames = HashSet::new();
        for language in LANGUAGES {
            assert!(
                names.insert(language.name),
                "duplicate language name {}",
                language.name
            );
            assert!(
                !language.extensions.is_empty() || !language.filenames.is_empty(),
                "{} has no path keys",
                language.name
            );
            for extension in language.extensions {
                assert!(
                    extensions.insert(*extension),
                    "duplicate extension .{extension}"
                );
            }
            for filename in language.filenames {
                assert!(filenames.insert(*filename), "duplicate filename {filename}");
            }
        }
    }

    #[test]
    fn language_for_path_should_map_every_declared_key() {
        for (expected_index, language) in LANGUAGES.iter().enumerate() {
            for extension in language.extensions {
                assert_eq!(
                    language_for_path(&Path::new("dir").join(format!("source.{extension}"))),
                    Some(expected_index),
                    "extension {extension} did not map to {}",
                    language.name
                );
            }
            for filename in language.filenames {
                assert_eq!(
                    language_for_path(&Path::new("dir").join(filename)),
                    Some(expected_index),
                    "filename {filename} did not map to {}",
                    language.name
                );
            }
        }
    }

    #[test]
    fn language_for_path_should_prefer_exact_filenames_over_unknown_extensions() {
        assert_eq!(
            language_for_path(Path::new("CMakeLists.txt")).map(|index| LANGUAGES[index].name),
            Some("CMake")
        );
    }

    #[test]
    fn language_for_path_should_recognize_stylesheets_and_makefiles() {
        assert_eq!(
            language_for_path(Path::new("web/src/styles.css")).map(|index| LANGUAGES[index].name),
            Some("CSS")
        );
        assert_eq!(
            language_for_path(Path::new("theme.scss")).map(|index| LANGUAGES[index].name),
            Some("SCSS")
        );
        assert_eq!(
            language_for_path(Path::new("theme.sass")).map(|index| LANGUAGES[index].name),
            Some("Sass")
        );
        assert_eq!(
            language_for_path(Path::new("theme.less")).map(|index| LANGUAGES[index].name),
            Some("Less")
        );
        assert_eq!(
            language_for_path(Path::new("Makefile")).map(|index| LANGUAGES[index].name),
            Some("Makefile")
        );
    }

    #[test]
    fn language_for_path_should_ignore_unknown_and_data_files() {
        assert_eq!(language_for_path(Path::new("notes.md")), None);
        assert_eq!(language_for_path(Path::new("data.json")), None);
        assert_eq!(language_for_path(Path::new("blob.bin")), None);
    }
}
