use std::path::Path;

use serde::{Deserialize, Serialize};

/// Classification of where a declaration originated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceOrigin {
    /// From one of the user's entry headers
    Entry,
    /// Included from entry headers but not a system header
    UserInclude,
    /// System header (flag 3 in preprocessor line markers)
    System,
    /// Origin could not be determined
    Unknown,
}

/// File/line/column location derived from preprocessor line markers where available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub column: Option<usize>,
}

/// Controls which declarations to keep based on origin.
#[derive(Debug, Clone)]
pub struct OriginFilter {
    pub include_entry: bool,
    pub include_user: bool,
    pub include_system: bool,
}

impl Default for OriginFilter {
    fn default() -> Self {
        Self {
            include_entry: true,
            include_user: true,
            include_system: false,
        }
    }
}

impl OriginFilter {
    pub fn accepts(&self, origin: &SourceOrigin) -> bool {
        match origin {
            SourceOrigin::Entry => self.include_entry,
            SourceOrigin::UserInclude => self.include_user,
            SourceOrigin::System => self.include_system,
            SourceOrigin::Unknown => true, // Don't filter unknown origins
        }
    }
}

/// A parsed line marker entry.
#[derive(Debug, Clone)]
struct LineMarker {
    file: String,
    is_system: bool,
    line_num: usize,
}

/// Tracks which file is active at each byte range in preprocessed source.
#[derive(Debug, Clone)]
pub struct FileOriginMap {
    /// Sorted by byte_offset: (start_offset, file_path, is_system, line_num)
    ranges: Vec<(usize, String, bool, usize)>,
    /// Set of entry header paths (normalized)
    entry_headers: Vec<String>,
    source: String,
}

impl FileOriginMap {
    /// Build a file-origin map by parsing preprocessor line markers from source.
    pub fn parse(source: &str, entry_headers: &[impl AsRef<Path>]) -> Self {
        let entry_headers: Vec<String> = entry_headers
            .iter()
            .map(|p| normalize_path(p.as_ref()))
            .collect();

        let mut ranges = Vec::new();
        let mut offset = 0;

        for line in source.split('\n') {
            let line_start = offset;
            offset += line.len() + 1; // +1 for newline

            if let Some(marker) = parse_line_marker(line) {
                ranges.push((line_start, marker.file, marker.is_system, marker.line_num));
            }
        }

        FileOriginMap {
            ranges,
            entry_headers,
            source: source.to_string(),
        }
    }

    /// Determine the origin of a declaration at the given byte offset.
    pub fn origin_at(&self, byte_offset: usize) -> SourceOrigin {
        // Find the last range that starts before this offset
        let active = self
            .ranges
            .iter()
            .rev()
            .find(|(start, _, _, _)| *start <= byte_offset);

        match active {
            Some((_, file, is_system, _)) => {
                if *is_system {
                    SourceOrigin::System
                } else if self.is_entry_header(file) {
                    SourceOrigin::Entry
                } else {
                    SourceOrigin::UserInclude
                }
            }
            None => SourceOrigin::Unknown,
        }
    }

    /// Determine the source location of a declaration at the given byte offset.
    pub fn location_at(&self, byte_offset: usize) -> Option<SourceLocation> {
        let active = self
            .ranges
            .iter()
            .rev()
            .find(|(start, _, _, _)| *start <= byte_offset)?;
        let (marker_offset, file, _, line_num) = active;
        let prefix = self.source.get(*marker_offset..byte_offset)?;
        let newline_count = prefix.bytes().filter(|b| *b == b'\n').count();
        let line = line_num + newline_count.saturating_sub(1);
        let column = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.chars().count() + 1)
            .unwrap_or(1);

        Some(SourceLocation {
            file: file.clone(),
            line: Some(line),
            column: Some(column),
        })
    }

    fn is_entry_header(&self, file: &str) -> bool {
        let norm = normalize_path(Path::new(file));
        self.entry_headers.iter().any(|e| {
            // Exact match or filename match (preprocessor may use relative paths)
            *e == norm || path_ends_with(&norm, e) || path_ends_with(e, &norm)
        })
    }
}

/// Parse a single preprocessor line marker.
/// Format: `# <line> "<file>" [flags]`
fn parse_line_marker(line: &str) -> Option<LineMarker> {
    let line = line.trim();
    if !line.starts_with("# ") {
        return None;
    }
    let rest = &line[2..];

    // Parse line number
    let space_idx = rest.find(' ')?;
    let line_num: usize = rest[..space_idx].parse().ok()?;

    // Parse quoted filename
    let after_space = &rest[space_idx + 1..];
    if !after_space.starts_with('"') {
        return None;
    }
    let end_quote = after_space[1..].find('"')?;
    let file = after_space[1..1 + end_quote].to_string();

    // Parse flags after the closing quote
    let after_file = &after_space[1 + end_quote + 1..];
    let flags: Vec<u8> = after_file
        .split_whitespace()
        .filter_map(|f| f.parse::<u8>().ok())
        .collect();

    // Flag 3 = system header
    let is_system = flags.contains(&3);

    Some(LineMarker {
        file,
        is_system,
        line_num,
    })
}

fn normalize_path(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

fn path_ends_with(haystack: &str, needle: &str) -> bool {
    let h = Path::new(haystack);
    let n = Path::new(needle);
    h.ends_with(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_line_marker() {
        let m = parse_line_marker("# 1 \"zlib.h\"").unwrap();
        assert_eq!(m.file, "zlib.h");
        assert!(!m.is_system);
        assert_eq!(m.line_num, 1);
    }

    #[test]
    fn parse_system_header_marker() {
        let m = parse_line_marker("# 42 \"/usr/include/stdio.h\" 1 3 4").unwrap();
        assert_eq!(m.file, "/usr/include/stdio.h");
        assert!(m.is_system);
    }

    #[test]
    fn parse_non_system_include() {
        let m = parse_line_marker("# 1 \"mylib.h\" 1").unwrap();
        assert_eq!(m.file, "mylib.h");
        assert!(!m.is_system);
    }

    #[test]
    fn parse_return_marker() {
        let m = parse_line_marker("# 10 \"zlib.h\" 2").unwrap();
        assert_eq!(m.file, "zlib.h");
        assert!(!m.is_system);
    }

    #[test]
    fn non_marker_lines_ignored() {
        assert!(parse_line_marker("int foo(void);").is_none());
        assert!(parse_line_marker("// comment").is_none());
        assert!(parse_line_marker("").is_none());
    }

    #[test]
    fn file_origin_map_basic() {
        let source = concat!(
            "# 1 \"zlib.h\"\n",
            "int deflate(void);\n",
            "# 1 \"/usr/include/stdio.h\" 1 3 4\n",
            "int printf(const char *fmt, ...);\n",
            "# 5 \"zlib.h\" 2\n",
            "int inflate(void);\n",
        );
        let map = FileOriginMap::parse(source, &["zlib.h"]);

        // deflate is in zlib.h (entry)
        let offset_deflate = source.find("int deflate").unwrap();
        assert_eq!(map.origin_at(offset_deflate), SourceOrigin::Entry);

        // printf is in stdio.h (system)
        let offset_printf = source.find("int printf").unwrap();
        assert_eq!(map.origin_at(offset_printf), SourceOrigin::System);

        // inflate is back in zlib.h (entry)
        let offset_inflate = source.find("int inflate").unwrap();
        assert_eq!(map.origin_at(offset_inflate), SourceOrigin::Entry);
    }

    #[test]
    fn file_origin_map_reports_source_locations() {
        let source = concat!(
            "# 7 \"mylib.h\"\n",
            "int foo(void);\n",
            "int bar(void);\n",
        );
        let map = FileOriginMap::parse(source, &["mylib.h"]);

        let offset_foo = source.find("foo").unwrap();
        let foo = map.location_at(offset_foo).unwrap();
        assert_eq!(foo.file, "mylib.h");
        assert_eq!(foo.line, Some(7));
        assert_eq!(foo.column, Some(5));

        let offset_bar = source.find("bar").unwrap();
        let bar = map.location_at(offset_bar).unwrap();
        assert_eq!(bar.line, Some(8));
        assert_eq!(bar.column, Some(5));
    }

    #[test]
    fn file_origin_map_user_include() {
        let source = concat!(
            "# 1 \"mylib.h\"\n",
            "int foo(void);\n",
            "# 1 \"helper.h\" 1\n",
            "int bar(void);\n",
            "# 3 \"mylib.h\" 2\n",
            "int baz(void);\n",
        );
        let map = FileOriginMap::parse(source, &["mylib.h"]);

        let offset_foo = source.find("int foo").unwrap();
        assert_eq!(map.origin_at(offset_foo), SourceOrigin::Entry);

        let offset_bar = source.find("int bar").unwrap();
        assert_eq!(map.origin_at(offset_bar), SourceOrigin::UserInclude);

        let offset_baz = source.find("int baz").unwrap();
        assert_eq!(map.origin_at(offset_baz), SourceOrigin::Entry);
    }

    #[test]
    fn origin_filter_default() {
        let filter = OriginFilter::default();
        assert!(filter.accepts(&SourceOrigin::Entry));
        assert!(filter.accepts(&SourceOrigin::UserInclude));
        assert!(!filter.accepts(&SourceOrigin::System));
        assert!(filter.accepts(&SourceOrigin::Unknown));
    }

    #[test]
    fn origin_filter_system_included() {
        let filter = OriginFilter {
            include_entry: true,
            include_user: true,
            include_system: true,
        };
        assert!(filter.accepts(&SourceOrigin::System));
    }

    #[test]
    fn unknown_offset_returns_unknown() {
        let map = FileOriginMap::parse("", &["test.h"]);
        assert_eq!(map.origin_at(0), SourceOrigin::Unknown);
    }
}
