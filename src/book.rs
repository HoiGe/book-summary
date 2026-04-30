use crate::sort::{compare_paths, parse_filename};
use std::path::Path;
use std::str::FromStr;
use titlecase::titlecase;

#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    Md(char),
    Git(char),
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "md" => Ok(Format::Md('-')),
            "git" => Ok(Format::Git('*')),
            _ => Err(format!("不支持的格式：{}", s)),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Chapter {
    pub name: String,
    pub files: Vec<String>,
    pub chapter: Vec<Chapter>,
    pub mdheader: bool,
}

impl Chapter {
    pub fn new(name: String, entries: &[String], mdheader: bool) -> Chapter {
        let mut chapter = Chapter {
            name,
            files: vec![],
            chapter: vec![],
            mdheader,
        };

        for entry in entries {
            chapter.add_entry(entry.split('/').collect::<Vec<_>>(), "");
        }

        chapter.sort_contents();
        chapter
    }

    fn add_entry(&mut self, entry: Vec<&str>, root: &str) {
        let new_root = match root {
            "" => entry[0].to_string(),
            _ => format!("{}/{}", root, entry[0]),
        };

        if entry.len() > 1 {
            if let Some(chapter) = self.chapter.iter_mut().find(|c| c.name == entry[0]) {
                chapter.add_entry(entry[1..].to_owned(), &new_root)
            } else {
                let mut chapter = Chapter {
                    name: entry[0].to_string(),
                    files: vec![],
                    chapter: vec![],
                    mdheader: false,
                };
                chapter.add_entry(entry[1..].to_owned(), &new_root);
                self.chapter.push(chapter);
            }
        } else {
            self.files.push(new_root)
        }
    }

    fn sort_contents(&mut self) {
        self.files.sort_by(|a, b| compare_paths(a, b));

        self.chapter.sort_by(|a, b| a.name.cmp(&b.name));

        for child in &mut self.chapter {
            child.sort_contents();
        }
    }

    pub fn get_summary_file(
        &self,
        format: &Format,
        prefered_chapter: &Option<Vec<String>>,
        mdheader: bool,
    ) -> String {
        let indent_level = 0;
        let mut summary: String = "".to_string();
        summary.push_str(&format!("# {}\n\n", self.name));
        let list_char = match format {
            Format::Md(list_char) | Format::Git(list_char) => list_char,
        };
        let linked_files = self.linked_child_files(format);
        summary += &print_files_excluding(
            &self.files,
            &linked_files,
            list_char,
            indent_level,
            mdheader,
        );

        // first prefered chapters (sort)
        if let Some(chapter_names) = prefered_chapter {
            for chapter_name in chapter_names {
                if let Some(chapter) = self
                    .chapter
                    .iter()
                    .find(|c| c.name.to_lowercase() == chapter_name.to_lowercase())
                {
                    let link = self.link_file_for_child(chapter, format);
                    summary +=
                        &chapter.create_tree_for_summary(format, indent_level, mdheader, link);
                }
            }
        }

        for c in &self.chapter {
            if is_preferred_chapter(c, prefered_chapter) {
                continue;
            }

            let link = self.link_file_for_child(c, format);
            summary += &c.create_tree_for_summary(format, indent_level, mdheader, link);
        }
        summary
    }

    fn create_tree_for_summary(
        &self,
        format: &Format,
        indent: usize,
        mdheader: bool,
        link_override: Option<&str>,
    ) -> String {
        let mut summary: String = " ".repeat(4 * indent);
        let list_char = match format {
            Format::Md(c) => c,
            Format::Git(c) => c,
        };

        if let Some(link) = link_override.or_else(|| {
            self.files
                .iter()
                .find(|f| f.to_lowercase().ends_with("/readme.md"))
                .map(String::as_str)
        }) {
            summary += &format!(
                "{} [{}]({})\n",
                list_char,
                titlecase(&self.name),
                percent_encode_path(link)
            )
        } else {
            match format {
                Format::Md(_) => {
                    let link = match self.infer_chapter_link() {
                        Some(link) => link,
                        None => format!("{}.md", titlecase(&self.name)),
                    };
                    summary.push_str(&format!(
                        "{} [{}]({})\n",
                        list_char,
                        titlecase(&self.name),
                        percent_encode_path(&link)
                    ))
                }
                Format::Git(_) => {
                    summary.push_str(&format!("{} {}\n", list_char, titlecase(&self.name)))
                }
            }
        }

        let linked_files = self.linked_child_files(format);
        summary +=
            &print_files_excluding(&self.files, &linked_files, list_char, indent + 1, mdheader);

        for c in &self.chapter {
            let link = self.link_file_for_child(c, format);
            summary += &c.create_tree_for_summary(format, indent + 1, mdheader, link);
        }
        summary
    }

    fn link_file_for_child(&self, child: &Chapter, format: &Format) -> Option<&str> {
        if !matches!(format, Format::Md(_)) {
            return None;
        }

        self.files
            .iter()
            .find(|file| match file_stem(file) {
                Some(stem) => stem.eq_ignore_ascii_case(&child.name),
                None => false,
            })
            .map(String::as_str)
    }

    fn linked_child_files(&self, format: &Format) -> Vec<&str> {
        self.chapter
            .iter()
            .filter_map(|child| self.link_file_for_child(child, format))
            .collect()
    }

    fn infer_chapter_link(&self) -> Option<String> {
        let first_path = self.first_content_path()?;
        let components: Vec<&str> = first_path.split('/').collect();

        components
            .iter()
            .position(|component| *component == self.name)
            .map(|index| format!("{}.md", components[..=index].join("/")))
    }

    fn first_content_path(&self) -> Option<&str> {
        self.files
            .first()
            .map(String::as_str)
            .or_else(|| self.chapter.iter().find_map(Chapter::first_content_path))
    }
}

fn is_preferred_chapter(chapter: &Chapter, preferred_chapter: &Option<Vec<String>>) -> bool {
    match preferred_chapter {
        Some(names) => names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&chapter.name)),
        None => false,
    }
}

fn print_files_excluding(
    files: &[String],
    excluded_files: &[&str],
    list_char: &char,
    indent: usize,
    mdheader: bool,
) -> String {
    files
        .iter()
        .filter(|f| !f.to_lowercase().ends_with("/readme.md"))
        .filter(|f| !excluded_files.contains(&f.as_str()))
        .map(|f| {
            let title = if mdheader {
                match get_first_header(f) {
                    Some(header) => header,
                    None => get_display_title(f),
                }
            } else {
                get_display_title(f)
            };

            format!(
                "{}{} [{}]({})\n",
                " ".repeat(4 * indent),
                list_char,
                title,
                percent_encode_path(f)
            )
        })
        .collect::<Vec<String>>()
        .join("")
}

fn get_display_title(file_path: &str) -> String {
    let stem = match file_stem(file_path) {
        Some(stem) => stem,
        None => file_path.to_string(),
    };

    if parse_filename(file_path).is_some() {
        stem
    } else {
        titlecase(&stem.replace('_', " "))
    }
}

fn file_stem(file_path: &str) -> Option<String> {
    Path::new(file_path)
        .file_stem()?
        .to_str()
        .map(str::to_string)
}

fn get_first_header(file_path: &str) -> Option<String> {
    let full_path = Path::new(file_path);
    let content = std::fs::read_to_string(full_path).ok()?;
    content
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").trim().to_string())
}

fn percent_encode_path(path: &str) -> String {
    let needs_encoding = path
        .chars()
        .any(|c| c.is_whitespace() || c == '#' || c == '[' || c == ']' || c == '<' || c == '>');

    if needs_encoding {
        format!("<{}>", path)
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| path.to_string()).collect()
    }

    #[test]
    fn format_from_str_rejects_unknown_format() {
        assert_eq!(Ok(Format::Md('-')), "md".parse::<Format>());
        assert_eq!(Ok(Format::Git('*')), "git".parse::<Format>());
        assert_eq!(
            Err("不支持的格式：html".to_string()),
            "html".parse::<Format>()
        );
    }

    #[test]
    fn print_files_test() {
        let files = files(&["0.1.ABcdc.md", "0.2.BcDAc.md", "0.10.bbdac.md"]);
        let expected = r#"- [0.1.ABcdc](0.1.ABcdc.md)
- [0.2.BcDAc](0.2.BcDAc.md)
- [0.10.bbdac](0.10.bbdac.md)
"#;
        assert_eq!(expected, print_files_excluding(&files, &[], &'-', 0, false));
    }

    #[test]
    fn percent_encode_path_test() {
        let cases = [
            ("normal/path/file.md", "normal/path/file.md"),
            ("path with spaces/file.md", "<path with spaces/file.md>"),
            ("path#hash/file.md", "<path#hash/file.md>"),
            ("path [special]/file.md", "<path [special]/file.md>"),
        ];

        for (path, expected) in cases {
            assert_eq!(expected, percent_encode_path(path));
        }
    }

    #[test]
    fn md_chapter_links_keep_nested_directory_path() {
        let input = files(&["A/A1/page.md"]);
        let book = Chapter::new("Summary".to_string(), &input, false);

        let expected = r#"# Summary

- [A](A.md)
    - [A1](A/A1.md)
        - [Page](A/A1/page.md)
"#;

        assert_eq!(
            expected,
            book.get_summary_file(&Format::Md('-'), &None, false)
        );
    }

    #[test]
    fn md_markdown_named_chapter_uses_regular_link() {
        let input = files(&["Markdown/intro.md"]);
        let book = Chapter::new("Summary".to_string(), &input, false);

        let expected = r#"# Summary

- [Markdown](Markdown.md)
    - [Intro](Markdown/intro.md)
"#;

        assert_eq!(
            expected,
            book.get_summary_file(&Format::Md('-'), &None, false)
        );
    }

    #[test]
    fn md_same_named_file_is_used_as_child_chapter_link_only_once() {
        let input = files(&[
            "volume/chapter/readme.md",
            "volume/chapter/markdown.md",
            "volume/chapter/markdown/intro.md",
        ]);
        let book = Chapter::new("Summary".to_string(), &input, false);

        let expected = r#"# Summary

- [Volume](volume.md)
    - [Chapter](volume/chapter/readme.md)
        - [Markdown](volume/chapter/markdown.md)
            - [Intro](volume/chapter/markdown/intro.md)
"#;

        assert_eq!(
            expected,
            book.get_summary_file(&Format::Md('-'), &None, false)
        );
    }

    #[test]
    fn print_files_can_use_first_markdown_header() -> std::io::Result<()> {
        let path = std::env::temp_dir().join(format!(
            "book-summary-header-{}-{}.md",
            std::process::id(),
            "chapter"
        ));
        std::fs::write(&path, "# Header Title\n\nBody")?;

        let path = path.to_string_lossy().to_string();
        let expected = format!("- [Header Title]({})\n", path);

        assert_eq!(expected, print_files_excluding(&[path], &[], &'-', 0, true));
        Ok(())
    }
}
