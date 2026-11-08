//! What an item looks like, for display and filtering (design handoff section 9).
//!
//! Stored and synced kinds stay text, rich and image (SPEC 6.2, D-021). This module
//! derives a finer [`Look`] from the preview with cheap heuristics, so the list can
//! show the right icon, set code in mono, and offer Code and Links filters.

use crate::model::ClipKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Look {
    Text,
    Rich,
    Image,
    Code,
    Link,
    Colour,
}

impl Look {
    pub fn as_str(self) -> &'static str {
        match self {
            Look::Text => "text",
            Look::Rich => "rich",
            Look::Image => "image",
            Look::Code => "code",
            Look::Link => "link",
            Look::Colour => "colour",
        }
    }
}

/// The look of a clip, and whether its text reads better in a monospace font (code,
/// colours, IP addresses).
pub fn classify(kind: ClipKind, preview: &str) -> (Look, bool) {
    match kind {
        ClipKind::Image => return (Look::Image, false),
        ClipKind::Rich => return (Look::Rich, false),
        ClipKind::Text => {}
    }
    let text = preview.trim();
    if is_link(text) {
        (Look::Link, false)
    } else if is_colour(text) {
        (Look::Colour, true)
    } else if is_code(text) {
        (Look::Code, true)
    } else {
        (Look::Text, is_ip(text))
    }
}

/// The address to open for a clip that looks like a link: web and mail links only,
/// with `https://` added to bare hosts like `www.example.org`.
pub fn link_target(text: &str) -> Option<String> {
    let text = text.trim();
    if !is_link(text) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|p| lower.starts_with(p))
    {
        Some(text.to_owned())
    } else if lower.contains("://") {
        None
    } else {
        Some(format!("https://{text}"))
    }
}

fn single_word(text: &str) -> bool {
    !text.is_empty() && !text.contains(char::is_whitespace)
}

fn is_link(text: &str) -> bool {
    if !single_word(text) {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if ["http://", "https://", "ftp://", "mailto:", "www."]
        .iter()
        .any(|p| lower.starts_with(p))
    {
        return true;
    }
    // host.tld/path, like login.tailscale.com/admin/machines
    let Some((host, _)) = lower.split_once('/') else {
        return false;
    };
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels
            .last()
            .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

fn is_colour(text: &str) -> bool {
    if let Some(hex) = text.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let lower = text.to_ascii_lowercase();
    ["rgb(", "rgba(", "hsl(", "hsla(", "oklch("]
        .iter()
        .any(|p| lower.starts_with(p))
        && lower.ends_with(')')
        && !lower.contains('\n')
}

fn is_ip(text: &str) -> bool {
    single_word(text) && text.parse::<std::net::IpAddr>().is_ok()
}

/// Commands people paste into terminals.
const COMMANDS: &[&str] = &[
    "apt",
    "brew",
    "cargo",
    "cat",
    "cd",
    "chmod",
    "chown",
    "cp",
    "curl",
    "docker",
    "dnf",
    "echo",
    "export",
    "find",
    "gh",
    "git",
    "go",
    "grep",
    "helm",
    "journalctl",
    "kubectl",
    "ls",
    "make",
    "mkdir",
    "mv",
    "node",
    "npm",
    "npx",
    "pip",
    "pnpm",
    "python",
    "python3",
    "rm",
    "rustup",
    "scp",
    "sed",
    "ssh",
    "sudo",
    "systemctl",
    "tailscale",
    "tar",
    "terraform",
    "wget",
    "yarn",
];

fn is_code(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let Some(first) = lines.first() else {
        return false;
    };
    if lines.len() == 1 {
        let line = first.trim_start_matches("$ ").trim();
        let command = line.split_whitespace().next().unwrap_or("");
        if COMMANDS.contains(&command) && line.split_whitespace().count() > 1 {
            return true;
        }
        if line
            .split_whitespace()
            .any(|w| w.starts_with("--") && w.len() > 2)
        {
            return true;
        }
    }
    let indented = lines
        .iter()
        .filter(|l| l.starts_with("  ") || l.starts_with('\t'))
        .count();
    if lines.len() >= 2 && indented * 3 >= lines.len() {
        return true;
    }
    let symbols = text
        .chars()
        .filter(|c| matches!(c, '{' | '}' | ';' | '=' | '(' | ')' | '<' | '>' | '|' | '$'))
        .count();
    let words = text.split_whitespace().count().max(1);
    symbols >= 2 && symbols * 4 >= words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(text: &str) -> (Look, bool) {
        classify(ClipKind::Text, text)
    }

    #[test]
    fn links() {
        for s in [
            "https://tailscale.com/kb/1537/grants-app-capabilities",
            "login.tailscale.com/admin/machines",
            "www.example.org",
            "mailto:will@example.com",
        ] {
            assert_eq!(look(s).0, Look::Link, "{s}");
        }
        for s in ["see https://x.com", "1.5/2", "a.b/c d", "file.txt"] {
            assert_ne!(look(s).0, Look::Link, "{s}");
        }
    }

    #[test]
    fn link_targets() {
        let target = |s: &str| link_target(s);
        assert_eq!(
            target(" https://tailscale.com/kb\n").as_deref(),
            Some("https://tailscale.com/kb")
        );
        assert_eq!(
            target("www.example.org").as_deref(),
            Some("https://www.example.org")
        );
        assert_eq!(
            target("login.tailscale.com/admin").as_deref(),
            Some("https://login.tailscale.com/admin")
        );
        assert_eq!(
            target("mailto:a@example.com").as_deref(),
            Some("mailto:a@example.com")
        );
        assert_eq!(target("ftp://example.org/x"), None);
        assert_eq!(target("see https://x.com"), None);
        assert_eq!(target("file.txt"), None);
    }

    #[test]
    fn colours() {
        for s in [
            "#7B52F0",
            "#fff",
            "#7b52f0cc",
            "rgb(12, 34, 56)",
            "hsl(260 80% 60%)",
        ] {
            assert_eq!(look(s), (Look::Colour, true), "{s}");
        }
        assert_ne!(look("#hashtag").0, Look::Colour);
        assert_ne!(look("#12345").0, Look::Colour);
    }

    #[test]
    fn code() {
        for s in [
            "docker compose -f stack.yml up -d",
            "tailscale up --ssh --accept-routes",
            "$ git rebase -i HEAD~4",
            "SELECT id FROM clips WHERE pinned = 1;",
            "fn main() {\n    println!(\"hi\");\n}",
            "cargo clippy --workspace --all-targets -- -D warnings",
        ] {
            assert_eq!(look(s), (Look::Code, true), "{s}");
        }
        for s in [
            "Ok, I've pushed the fix to main. Can you pull and retry the build?",
            "Meeting moved to Thursday 3pm",
            "git",
            "I",
        ] {
            assert_eq!(look(s).0, Look::Text, "{s}");
        }
    }

    #[test]
    fn ip_addresses_are_mono_text() {
        assert_eq!(look("100.88.12.34"), (Look::Text, true));
        assert_eq!(look("fd7a:115c:a1e0::1"), (Look::Text, true));
        assert_eq!(look("hello"), (Look::Text, false));
    }

    #[test]
    fn other_kinds_keep_their_look() {
        assert_eq!(
            classify(ClipKind::Image, "image 10×10"),
            (Look::Image, false)
        );
        assert_eq!(classify(ClipKind::Rich, "docker ps"), (Look::Rich, false));
    }
}
