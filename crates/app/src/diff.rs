//! Unified diffs (what coding agents post: `git diff` output, `.patch`
//! files) parsed into files, hunks and numbered lines for the review dialog.

/// What a diff line does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub kind: LineKind,
    /// Line number in the old / new file (absent on the side it isn't on).
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hunk {
    /// The `@@ -a,b +c,d @@ context` line.
    pub header: String,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileDiff {
    /// The new path (the old one for a deleted file).
    pub path: String,
    pub hunks: Vec<Hunk>,
    pub added: usize,
    pub removed: usize,
}

/// `-a,b` / `+c,d` → the start line.
fn range_start(part: &str) -> u32 {
    part[1..].split(',').next().and_then(|n| n.parse().ok()).unwrap_or(1)
}

/// A path from `--- a/x` / `+++ b/x` / `diff --git a/x b/x` (prefix dropped).
fn clean_path(p: &str) -> Option<String> {
    let p = p.split('\t').next().unwrap_or(p).trim();
    if p == "/dev/null" || p.is_empty() {
        return None;
    }
    Some(p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p).to_string())
}

/// Parse a unified diff. Anything before the first file header is ignored;
/// so is a hunk line that can't be a diff line.
pub fn parse(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_no = 0u32;
    let mut new_no = 0u32;
    let mut old_path: Option<String> = None;
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // `a/x b/y`: the new path, until --- / +++ say better.
            let path = rest.rsplit_once(" b/").map(|(_, b)| b.to_string()).unwrap_or_else(|| rest.to_string());
            files.push(FileDiff { path, ..Default::default() });
            old_path = None;
        } else if let Some(rest) = line.strip_prefix("--- ") {
            if lines.peek().is_some_and(|n| n.starts_with("+++ ")) {
                old_path = clean_path(rest);
            }
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            let path = clean_path(rest).or_else(|| old_path.clone()).unwrap_or_default();
            // A new file unless `diff --git` just opened one.
            match files.last_mut() {
                Some(f) if f.hunks.is_empty() && f.added == 0 && f.removed == 0 && !path.is_empty() => f.path = path,
                _ => files.push(FileDiff { path, ..Default::default() }),
            }
        } else if line.starts_with("@@") {
            let mut parts = line.split_whitespace().skip(1);
            old_no = parts.next().map(range_start).unwrap_or(1);
            new_no = parts.next().map(range_start).unwrap_or(1);
            if files.is_empty() {
                files.push(FileDiff::default());
            }
            if let Some(f) = files.last_mut() {
                f.hunks.push(Hunk { header: line.to_string(), lines: Vec::new() });
            }
        } else if let Some(hunk) = files.last_mut().and_then(|f| f.hunks.last_mut()) {
            let (kind, body) = match line.chars().next() {
                Some('+') => (LineKind::Added, &line[1..]),
                Some('-') => (LineKind::Removed, &line[1..]),
                Some(' ') => (LineKind::Context, &line[1..]),
                None => (LineKind::Context, ""),
                _ => continue, // "\ No newline at end of file", trailing prose
            };
            let (old, new) = match kind {
                LineKind::Added => { new_no += 1; (None, Some(new_no - 1)) }
                LineKind::Removed => { old_no += 1; (Some(old_no - 1), None) }
                LineKind::Context => { old_no += 1; new_no += 1; (Some(old_no - 1), Some(new_no - 1)) }
            };
            hunk.lines.push(Line { kind, old, new, text: body.to_string() });
            if let Some(f) = files.last_mut() {
                match kind {
                    LineKind::Added => f.added += 1,
                    LineKind::Removed => f.removed += 1,
                    LineKind::Context => {}
                }
            }
        }
    }
    files.retain(|f| !f.hunks.is_empty());
    files
}

/// The diffs in a message: its code blocks that are diffs (formatted or
/// ``` fenced in the body).
pub fn diffs_in(body: &str, formatted: Option<&str>) -> Vec<String> {
    let blocks: Vec<String> = match formatted.filter(|f| f.contains("<pre")) {
        Some(html) => crate::view_messages::code_blocks_of(html),
        None => body.split("```").skip(1).step_by(2)
            // Drop the language tag line (```diff).
            .map(|b| b.split_once('\n').map(|(first, rest)| if first.trim().contains(' ') { b } else { rest }).unwrap_or(b).to_string())
            .collect(),
    };
    blocks.into_iter().filter(|b| crate::html::looks_like_diff(b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT: &str = "diff --git a/src/lib.rs b/src/lib.rs\nindex 1..2 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -10,4 +10,5 @@ fn main() {\n     let a = 1;\n-    let b = 2;\n+    let b = 3;\n+    let c = 4;\n     println!(\"{a}\");\n\\ No newline at end of file\ndiff --git a/README.md b/README.md\nnew file mode 100644\n--- /dev/null\n+++ b/README.md\n@@ -0,0 +1,2 @@\n+# Title\n+text\n";

    #[test]
    fn git_diffs_parse_into_files_hunks_and_numbers() {
        let files = parse(GIT);
        assert_eq!(files.len(), 2);
        let lib = &files[0];
        assert_eq!((lib.path.as_str(), lib.added, lib.removed), ("src/lib.rs", 2, 1));
        let lines = &lib.hunks[0].lines;
        assert_eq!(lines[0], Line { kind: LineKind::Context, old: Some(10), new: Some(10), text: "    let a = 1;".into() });
        assert_eq!((lines[1].kind, lines[1].old, lines[1].new), (LineKind::Removed, Some(11), None));
        assert_eq!((lines[2].kind, lines[2].old, lines[2].new), (LineKind::Added, None, Some(11)));
        assert_eq!((lines[4].old, lines[4].new), (Some(12), Some(13)));
        assert_eq!(lines.len(), 5, "the no-newline marker isn't a line");
        assert_eq!((files[1].path.as_str(), files[1].added), ("README.md", 2));
    }

    #[test]
    fn plain_unified_diffs_and_fenced_ones_in_messages() {
        let plain = "--- old.txt\n+++ new.txt\n@@ -1 +1 @@\n-a\n+b\n";
        assert_eq!(parse(plain)[0].path, "new.txt");
        let body = "Here's the fix:\n\n```diff\n--- a/x.py\n+++ b/x.py\n@@ -1 +1 @@\n-print(1)\n+print(2)\n```\nand some code:\n```python\nprint(3)\n```";
        let diffs = diffs_in(body, None);
        assert_eq!(diffs.len(), 1);
        assert_eq!(parse(&diffs[0])[0].path, "x.py");
        assert!(parse("just some text").is_empty());
    }
}
