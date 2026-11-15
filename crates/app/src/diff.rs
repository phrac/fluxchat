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
    // `get`: agents write headers like `@@ … @@`, where byte 1 isn't a boundary.
    part.get(1..).and_then(|s| s.split(',').next()).and_then(|n| n.parse().ok()).unwrap_or(1)
}

/// A path from `--- a/x` / `+++ b/x` / `diff --git a/x b/x` (prefix dropped).
fn clean_path(p: &str) -> Option<String> {
    let p = p.split('\t').next().unwrap_or(p).trim();
    if p == "/dev/null" || p.is_empty() {
        return None;
    }
    Some(p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p).to_string())
}

/// A file header's path: the file `diff --git` just opened takes it (nothing
/// is in it yet), else it opens a new one.
fn open_file(files: &mut Vec<FileDiff>, path: String) {
    match files.last_mut() {
        Some(f) if f.hunks.is_empty() && f.added == 0 && f.removed == 0 && !path.is_empty() => f.path = path,
        _ => files.push(FileDiff { path, ..Default::default() }),
    }
}

/// Parse a unified diff. Anything before the first file header is ignored;
/// so is a hunk line that can't be a diff line.
pub fn parse(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_no = 0u32;
    let mut new_no = 0u32;
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        // Inside a hunk, `--- x` is a removed `-- x` and `+++ x` an added
        // `++ x` (SQL, Lua and Haskell comments start that way). Read as file
        // headers they were dropped from the hunk, or opened a file that
        // isn't one, and every line number after them was off. There they
        // are headers only when a hunk follows them.
        let in_hunk = files.last().is_some_and(|f| !f.hunks.is_empty());
        let hunk_at = |n: usize| lines.get(n).is_some_and(|l| l.starts_with("@@"));
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // `a/x b/y`: the new path, until --- / +++ say better.
            let path = rest.rsplit_once(" b/").map(|(_, b)| b.to_string()).unwrap_or_else(|| rest.to_string());
            files.push(FileDiff { path, ..Default::default() });
            continue;
        }
        if let Some(old) = line.strip_prefix("--- ") {
            let new = lines.get(i).and_then(|n| n.strip_prefix("+++ ")).filter(|_| !in_hunk || hunk_at(i + 1));
            if let Some(new) = new {
                i += 1;
                // The new path (the old one for a deleted file).
                open_file(&mut files, clean_path(new).or_else(|| clean_path(old)).unwrap_or_default());
                continue;
            }
            if !in_hunk {
                continue;
            }
        } else if let Some(new) = line.strip_prefix("+++ ") {
            if !in_hunk || hunk_at(i) {
                open_file(&mut files, clean_path(new).unwrap_or_default());
                continue;
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
            continue;
        }
        let Some(file) = files.last_mut() else { continue };
        let Some(hunk) = file.hunks.last_mut() else { continue };
        let (kind, body) = match line.chars().next() {
            Some('+') => (LineKind::Added, &line[1..]),
            Some('-') => (LineKind::Removed, &line[1..]),
            Some(' ') => (LineKind::Context, &line[1..]),
            None => (LineKind::Context, ""),
            _ => continue, // "\ No newline at end of file", trailing prose
        };
        let (old, new) = match kind {
            LineKind::Added => (None, Some(new_no)),
            LineKind::Removed => (Some(old_no), None),
            LineKind::Context => (Some(old_no), Some(new_no)),
        };
        // Saturating: the header's numbers are whatever the agent wrote.
        if old.is_some() {
            old_no = old_no.saturating_add(1);
        }
        if new.is_some() {
            new_no = new_no.saturating_add(1);
        }
        hunk.lines.push(Line { kind, old, new, text: body.to_string() });
        match kind {
            LineKind::Added => file.added += 1,
            LineKind::Removed => file.removed += 1,
            LineKind::Context => {}
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

    #[test]
    fn comment_lines_in_a_hunk_are_changes_not_file_headers() {
        // A removed SQL comment is `--- old comment` in a diff.
        let sql = "--- a/q.sql\n+++ b/q.sql\n@@ -1,3 +1,3 @@\n select 1;\n--- old comment\n+-- new comment\n select 2;\n";
        let files = parse(sql);
        assert_eq!(files.len(), 1);
        assert_eq!((files[0].path.as_str(), files[0].added, files[0].removed), ("q.sql", 1, 1));
        let lines = &files[0].hunks[0].lines;
        assert_eq!(lines[1], Line { kind: LineKind::Removed, old: Some(2), new: None, text: "-- old comment".into() });
        assert_eq!((lines[3].old, lines[3].new), (Some(3), Some(3)), "the numbers after it are right");
        // A removed `-- a` right before an added `++ b` is still the hunk's…
        let files = parse("--- a/x.lua\n+++ b/x.lua\n@@ -1,2 +1,2 @@\n--- a\n+++ b\n keep\n");
        assert_eq!(files.len(), 1, "no file called b");
        assert_eq!((files[0].added, files[0].removed, files[0].hunks[0].lines.len()), (1, 1, 3));
        // …and the next file's header, which a hunk follows, still opens it.
        let files = parse("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-1\n+2\n--- a/y\n+++ b/y\n@@ -1 +1 @@\n-3\n+4\n");
        assert_eq!(files.iter().map(|f| (f.path.as_str(), f.added, f.removed)).collect::<Vec<_>>(), [("x", 1, 1), ("y", 1, 1)]);
        // A deleted file keeps its old path.
        assert_eq!(parse("--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n")[0].path, "gone.txt");
    }

    #[test]
    fn line_numbers_past_the_end_do_not_overflow() {
        let files = parse("--- a/x\n+++ b/x\n@@ -4294967295,2 +4294967295,2 @@\n-a\n-b\n+c\n+d\n e\n");
        assert_eq!((files[0].added, files[0].removed), (2, 2));
    }

    /// Agents elide or prettify hunk headers; this is parsed on every render
    /// of the message, so it must not panic.
    #[test]
    fn odd_hunk_headers_do_not_panic() {
        for header in ["@@ \u{2026} @@", "@@ \u{2212}1,3 +1,3 @@", "@@ @@", "@@"] {
            let files = parse(&format!("--- a/x\n+++ b/x\n{header}\n-old\n+new\n"));
            assert_eq!(files.len(), 1, "{header}");
            assert_eq!((files[0].added, files[0].removed), (1, 1), "{header}");
        }
    }
}
