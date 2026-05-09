//! Cross-platform helpers for matching agent CLI processes from sysinfo.
//!
//! Windows ships executables with `.exe`/`.cmd`/`.bat` suffixes and uses
//! backslashes as path separators. The original `matches_process`
//! implementations baked in Unix conventions (`/bin/claude`, `/.claude/local/`,
//! exact `claude` string match), which silently rejected every running agent
//! when the user ran the dashboard on Windows — the Live Processes panel was
//! always empty even with real `claude.exe`/`codex.cmd`/etc. processes
//! sampled by sysinfo.
//!
//! These helpers normalize separators and strip executable suffixes before
//! comparing, so the same pattern strings work on macOS, Linux, and Windows.

/// Returns `true` if `arg` is a *bare* binary invocation — no path separator
/// — whose name (with any executable suffix stripped) equals `name`.
///
/// This is the cross-platform replacement for `arg == "claude"`. It also
/// matches `claude.exe`, `claude.cmd`, `claude.bat`, `claude.ps1`, but
/// deliberately rejects paths like `/usr/local/bin/claude` (use
/// `path_ends_with` for those). Restricting to bare names is what keeps
/// pathological hits — e.g., the VSCode chatgpt extension's bundled
/// `bin/macos-aarch64/codex` "app-server" — out of Live Processes.
pub(crate) fn bare_name_matches(arg: &str, name: &str) -> bool {
    if arg.contains('/') || arg.contains('\\') {
        return false;
    }
    let stem = strip_exe_suffix(arg);
    stem == name
}

/// Returns `true` if `arg` ends with `suffix` after path-separator
/// normalization and exe-suffix stripping on the last segment.
///
/// Useful for matching things like `/bin/claude` or
/// `Codex.app/Contents/Resources/codex` regardless of which slash the host OS
/// reports. Strips a trailing `.exe`/`.cmd`/`.bat`/`.ps1` from `arg` so a
/// Windows arg like `C:\...\bin\claude.exe` matches the Unix-shaped suffix
/// `/bin/claude`.
pub(crate) fn path_ends_with(arg: &str, suffix: &str) -> bool {
    let arg_norm = normalize_separators(arg);
    let suffix_norm = normalize_separators(suffix);
    let arg_stripped = strip_exe_suffix(&arg_norm);
    arg_stripped.ends_with(&suffix_norm)
}

/// Returns `true` if `arg` contains `needle` after path-separator
/// normalization. Useful for substrings like `/.claude/local/`,
/// `/@anthropic-ai/claude-code`, `/@openai/codex-`, `/codex-cli/` —
/// patterns that show up when an npm-installed CLI runs as
/// `node.exe path\to\node_modules\<pkg>\cli.js` on Windows.
pub(crate) fn path_contains(arg: &str, needle: &str) -> bool {
    let arg_norm = normalize_separators(arg);
    let needle_norm = normalize_separators(needle);
    arg_norm.contains(&needle_norm)
}

/// Replace backslashes with forward slashes so a single string-comparison
/// works for paths from either OS. Returns an owned `String` only when a
/// substitution is needed; the borrow checker forces a clone here regardless
/// because callers want to pass the result to `ends_with` / `contains`.
fn normalize_separators(s: &str) -> String {
    if s.contains('\\') {
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
}

/// If `s` ends with one of `.exe / .cmd / .bat / .ps1` (case-insensitive),
/// return the stem; otherwise return `s` unchanged.
fn strip_exe_suffix(s: &str) -> &str {
    const EXTS: &[&str] = &[".exe", ".cmd", ".bat", ".ps1"];
    for ext in EXTS {
        if s.len() >= ext.len() {
            let tail = &s[s.len() - ext.len()..];
            if tail.eq_ignore_ascii_case(ext) {
                return &s[..s.len() - ext.len()];
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_name_matches_unix_and_windows_forms() {
        // Bare invocations.
        assert!(bare_name_matches("claude", "claude"));
        assert!(bare_name_matches("claude.exe", "claude"));
        assert!(bare_name_matches("claude.cmd", "claude"));
        assert!(bare_name_matches("claude.bat", "claude"));
        assert!(bare_name_matches("claude.ps1", "claude"));
        // Case-insensitive extension only.
        assert!(bare_name_matches("CLAUDE.EXE", "CLAUDE"));
        assert!(bare_name_matches("claude.EXE", "claude"));
    }

    #[test]
    fn bare_name_matches_rejects_anything_with_a_path() {
        // Even if the basename matches, a path-qualified arg must NOT match
        // here — that's the regression that broke the VSCode codex test
        // when basename matching was too permissive.
        assert!(!bare_name_matches("/usr/local/bin/claude", "claude"));
        assert!(!bare_name_matches("C:\\Users\\u\\claude.exe", "claude"));
        // No accidental matches on substrings.
        assert!(!bare_name_matches("claudette", "claude"));
        assert!(!bare_name_matches("claude.exetra", "claude"));
        assert!(!bare_name_matches("claude.js", "claude"));
    }

    #[test]
    fn path_ends_with_handles_separators_and_extensions() {
        // Unix paths still work.
        assert!(path_ends_with("/usr/local/bin/claude", "/bin/claude"));
        // Windows path with backslashes — `.exe` stripped before comparison.
        assert!(path_ends_with(
            "C:\\Program Files\\Anthropic\\bin\\claude.exe",
            "/bin/claude"
        ));
        // Strict suffix match — vscode codex bundle must NOT match
        // `/bin/codex` because the immediate parent isn't `bin`.
        assert!(!path_ends_with(
            "/Users/test/.vscode/extensions/foo/bin/macos-aarch64/codex",
            "/bin/codex"
        ));
        // Should not match unrelated suffixes.
        assert!(!path_ends_with("/usr/local/bin/claude", "/bin/codex"));
    }

    #[test]
    fn path_contains_normalizes_separators() {
        // The classic `/.claude/local/` pattern must match on Windows too.
        assert!(path_contains(
            "C:\\Users\\yjw\\.claude\\local\\node_modules\\.bin\\claude.cmd",
            "/.claude/local/"
        ));
        // npm-installed CLI running as `node.exe path\to\@anthropic-ai\claude-code\cli.js`.
        assert!(path_contains(
            "C:\\Users\\yjw\\AppData\\Roaming\\npm\\node_modules\\@anthropic-ai\\claude-code\\cli.js",
            "/@anthropic-ai/claude-code"
        ));
        // And of course on Unix.
        assert!(path_contains(
            "/Users/yjw/.claude/local/node_modules/.bin/claude",
            "/.claude/local/"
        ));
        // Negative case.
        assert!(!path_contains(
            "C:\\Users\\yjw\\.claude\\projects\\foo",
            "/.claude/local/"
        ));
    }
}
