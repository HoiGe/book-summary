use clap::{ArgAction, Parser};
use serde_json::Value as jsonValue;
use std::env;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use toml::Table;
use walkdir::{DirEntry, WalkDir};

mod book;
mod sort;
use book::Chapter;
use book::Format;

#[derive(Parser, Debug)]
struct Opt {
    /// Activate debug mode
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,

    /// Title from md file header?
    #[arg(short = 'm', long = "mdheader")]
    mdheader: bool,

    /// Format md/git book
    #[arg(short = 'f', long = "format", default_value = "md")]
    format: Format,

    /// Title for summary
    #[arg(short = 't', long = "title", default_value = "Summary")]
    title: String,

    /// Start with following chapters (space seperate)
    #[arg(short = 's', long = "sort", num_args = 1..)]
    sort: Option<Vec<String>>,

    /// Output file
    #[arg(short = 'o', long = "outputfile", default_value = "SUMMARY.md")]
    outputfile: String,

    /// Notes dir where to parse all your notes from
    #[arg(short = 'n', long = "notesdir", default_value = ".")]
    dir: PathBuf,

    /// Overwrite existing SUMMARY.md file
    #[arg(short = 'y', long = "overwrite")]
    yes: bool,

    /// Only update content between marker comments, keeping other sections intact
    #[arg(short = 'k', long = "marker")]
    marker: Option<String>,
}

fn main() {
    let mut opt = Opt::parse();

    // print opt in verbose level 3
    if opt.verbose > 2 {
        println!("{:?}", opt);
        match env::current_dir() {
            Ok(dir) => println!("{:?}", dir.display()),
            Err(err) => eprintln!("错误：无法获取当前目录：{}", err),
        }
    }

    // parse book.js OR book.toml
    let config_result = match opt.format {
        Format::Md(_) => {
            parse_config_file(&format!("{}{}", opt.dir.display(), "/book.toml"), &mut opt)
        }
        Format::Git(_) => {
            parse_config_file(&format!("{}{}", opt.dir.display(), "/book.json"), &mut opt).and_then(
                |_| parse_config_file(&format!("{}{}", opt.dir.display(), "/book.js"), &mut opt),
            )
        }
    };

    if let Err(err) = config_result {
        eprintln!("错误：{}", err);
        std::process::exit(1);
    }

    if opt.dir == Path::new("./") {
        opt.dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("错误：无法获取当前目录：{}", err);
                std::process::exit(1)
            }
        };
    }

    if !opt.dir.is_dir() {
        eprintln!("错误：路径不存在：{}", opt.dir.display());
        std::process::exit(1)
    }

    let entries = get_dir(&opt.dir, &opt.outputfile);

    // SUMMARY.md file check if exists
    if Path::new(&format!("{}/{}", &opt.dir.display(), &opt.outputfile)).exists() && !opt.yes {
        loop {
            println!("文件 {} 已存在，是否覆盖？[Y/n]", &opt.outputfile);
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) if &input == "y\n" || &input == "Y\n" || &input == "\n" => break,
                Ok(_) if &input == "n\n" || &input == "N\n" => return,
                _ => {}
            }
        }
    }

    if opt.verbose > 2 {
        dbg!(&entries);
    }

    let book = Chapter::new(opt.title, &entries, opt.mdheader);

    if let Err(err) = create_file(
        &opt.dir,
        &opt.outputfile,
        // &book.get_summary_file(&opt.format),
        &book.get_summary_file(&opt.format, &opt.sort, opt.mdheader),
        opt.marker.as_deref(),
    ) {
        eprintln!("错误：{}", err);
        std::process::exit(1)
    }

    if opt.verbose > 2 {
        dbg!(&book);
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    match entry.file_name().to_str() {
        Some(name) => name.starts_with('.'),
        None => false,
    }
}

fn get_dir(dir: &PathBuf, outputfile: &str) -> Vec<String> {
    let mut entries: Vec<String> = vec![];
    for direntry in WalkDir::new(dir)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok())
    {
        // entry without:
        // - given root folder
        // - plain dirnames
        // - not md files
        // - not SUMMARY.md file
        if let Some(entry) = direntry
            .path()
            .strip_prefix(dir)
            .ok()
            .and_then(Path::to_str)
        {
            if !entry.is_empty()
                && !entry.eq(outputfile)
                && !entry.to_lowercase().eq("readme.md")
                && entry.to_lowercase().ends_with(".md")
            {
                entries.push(entry.to_owned());
            }
        }
    }
    entries
}

fn parse_config_file(path: &str, opt: &mut Opt) -> std::result::Result<(), String> {
    let path = Path::new(path);

    if !path.exists() {
        if opt.verbose > 2 {
            eprintln!("未找到书籍配置文件：{}", path.display());
        }
        return Ok(());
    }

    let mut file = match File::open(path) {
        Err(why) => return Err(format!("无法打开 {}：{}", path.display(), why)),
        Ok(file) => file,
    };

    let mut content = String::new();

    if let Err(why) = file.read_to_string(&mut content) {
        return Err(format!("无法读取 {}：{}", path.display(), why));
    }

    if opt.verbose > 2 {
        println!("Found book config file: {}", path.display());
    }

    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return Ok(());
    };

    match ext {
        "toml" => {
            let values = match content.parse::<Table>() {
                Ok(values) => values,
                Err(err) => return Err(format!("无法解析 {}：{}", path.display(), err)),
            };
            if opt.dir.to_str().eq(&Some(".")) {
                if let Some(src) = values
                    .get("book")
                    .and_then(|book| book.get("src"))
                    .and_then(|src| src.as_str())
                {
                    if opt.verbose > 2 {
                        println!("Found `src` in book.toml: {}", src);
                    }
                    if !src.is_empty() {
                        opt.dir = PathBuf::from(src);
                    }
                }
            }

            if opt.title.eq("Summary") {
                if let Some(title) = values
                    .get("book")
                    .and_then(|book| book.get("title"))
                    .and_then(|title| title.as_str())
                {
                    if opt.verbose > 2 {
                        println!("Found `title` in book.toml: {}", title);
                    }
                    opt.title = title.to_string();
                }
            }
        }
        "js" | "json" => {
            let values: jsonValue = match serde_json::from_str(&content) {
                Ok(values) => values,
                Err(err) => return Err(format!("无法解析 {}：{}", path.display(), err)),
            };
            if opt.dir.to_str().eq(&Some(".")) {
                if let Some(src) = values["root"].as_str() {
                    if opt.verbose > 2 {
                        println!("Found `root` in book.{}: {}", ext, src);
                    }
                    if !src.is_empty() {
                        opt.dir = PathBuf::from(src);
                    }
                }
            }

            if opt.title.eq("Summary") {
                if let Some(title) = values["title"].as_str() {
                    if opt.verbose > 2 {
                        println!("Found `title` in book.{}: {}", ext, title);
                    }
                    opt.title = title.to_string();
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn create_file(path: &Path, filename: &str, content: &str, marker: Option<&str>) -> io::Result<()> {
    let path = path.join(filename);
    let display = path.display();

    let final_content = if let Some(_marker) = marker {
        // If marker is specified and file exists, do partial update
        if path.exists() {
            let existing = std::fs::read_to_string(&path).map_err(|err| {
                io::Error::new(err.kind(), format!("无法读取已有文件 {}：{}", display, err))
            })?;
            update_partial_summary(&existing, content, _marker)
        } else {
            // No existing file, create with marker wrapper
            wrap_with_marker(content, _marker)
        }
    } else {
        content.to_string()
    };

    // Open a file in write-only mode, returns `io::Result<File>`
    let mut file = File::create(&path)
        .map_err(|err| io::Error::new(err.kind(), format!("无法创建文件 {}：{}", display, err)))?;

    // Write the content to `file`, returns `io::Result<()>`
    file.write_all(final_content.as_bytes())
        .map_err(|err| io::Error::new(err.kind(), format!("无法写入文件 {}：{}", display, err)))?;
    println!("已成功创建 {}", display);
    Ok(())
}

/// Wraps content with marker comments
fn wrap_with_marker(content: &str, marker: &str) -> String {
    format!(
        "<!-- book-summary-start-{marker} -->\n{}\n<!-- book-summary-end-{marker} -->",
        content
    )
}

/// Updates only the content between marker comments in an existing summary
fn update_partial_summary(existing: &str, new_content: &str, marker: &str) -> String {
    let start_marker = format!("<!-- book-summary-start-{} -->", marker);
    let end_marker = format!("<!-- book-summary-end-{} -->", marker);

    let start_idx = existing.find(&start_marker);
    let end_idx = existing.find(&end_marker);

    if let (Some(start), Some(end)) = (start_idx, end_idx) {
        let content_start = start + start_marker.len();
        let before_end = existing[..end]
            .rfind('\n')
            .map_or(content_start, |index| index + 1);
        let new_content = if new_content.ends_with('\n') {
            new_content.to_string()
        } else {
            format!("{new_content}\n")
        };

        // Replace only the content between markers
        format!(
            "{}{}{}",
            &existing[..content_start],
            new_content,
            &existing[before_end..]
        )
    } else {
        // No existing markers found, append wrapped content
        format!("{}\n\n{}", existing, wrap_with_marker(new_content, marker))
    }
}

/* ------------------------- TEST --------------------------------- */
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TITLE: &str = "Summary";
    const FORMAT: Format = Format::Git('*');
    const LIST_CHAR: char = '*';

    fn files(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| path.to_string()).collect()
    }

    fn book(paths: &[&str]) -> Chapter {
        Chapter::new(TITLE.to_string(), &files(paths), false)
    }

    fn test_dir(name: &str) -> io::Result<PathBuf> {
        let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        let path = std::env::temp_dir().join(format!(
            "book-summary-{}-{}-{}",
            name,
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    fn default_opt() -> Opt {
        Opt {
            debug: false,
            verbose: 0,
            mdheader: false,
            format: FORMAT,
            title: "Summary".to_string(),
            sort: None,
            outputfile: "SUMMARY.md".to_string(),
            dir: PathBuf::from("."),
            yes: true,
            marker: None,
        }
    }

    #[test]
    fn cli_accepts_repeated_verbose_and_multi_value_sort() -> Result<(), clap::Error> {
        let opt = Opt::try_parse_from([
            "book-summary",
            "-vvv",
            "--sort",
            "tech",
            "personal",
            "--format",
            "git",
        ])?;

        assert_eq!(3, opt.verbose);
        assert_eq!(
            Some(vec!["tech".to_string(), "personal".to_string()]),
            opt.sort
        );
        assert_eq!(Format::Git('*'), opt.format);
        Ok(())
    }

    // # get file list: no hidden files, filepaths from given folder as root
    #[test]
    fn get_file_list_test() {
        let expected = vec![
            "about.md".to_string(),
            "chapter1/FILE.md".to_string(),
            "chapter1/file1.md".to_string(),
            "chapter2/FILE1.md".to_string(),
            "chapter2/README.md".to_string(),
            "chapter2/file2.md".to_string(),
            "chapter2/subchap/info.md".to_string(),
            "chapter3/file1.md".to_string(),
            "chapter3/file2.md".to_string(),
            "chapter3/file3.md".to_string(),
        ];
        assert_eq!(
            expected,
            get_dir(&PathBuf::from(r"./examples/gitbook/book"), "SUMMARY.md")
        );
    }

    #[test]
    fn get_dir_only_collects_markdown_files() -> io::Result<()> {
        let dir = test_dir("markdown-filter")?;
        std::fs::write(dir.join("chapter.md"), "")?;
        std::fs::write(dir.join("draft.md.bak"), "")?;
        std::fs::write(dir.join("README.md"), "")?;
        std::fs::write(dir.join("SUMMARY.md"), "")?;

        assert_eq!(vec!["chapter.md".to_string()], get_dir(&dir, "SUMMARY.md"));
        Ok(())
    }

    // 2. Markdown output for entry in chapter
    //      - format (md/git)
    //      - titlecase for entry
    //      - remove pre numbers in entry
    #[test]
    fn md_output_onefile_test() {
        // only one file
        let expected: &str = &format!("# {}\n\n{} [File1](file1.md)\n", TITLE, LIST_CHAR);

        assert_eq!(
            expected,
            book(&["file1.md"]).get_summary_file(&FORMAT, &None, false)
        );
    }

    #[test]
    fn md_output_empty_book_test() {
        assert_eq!(
            "# Summary\n\n",
            book(&[]).get_summary_file(&FORMAT, &None, false)
        );
    }

    #[test]
    fn md_output_onechapter_test() {
        // only one file
        let expected: &str = &format!(
            "# {0}\n\n{1} [File1](file1.md)\n{1} Chapter1\n    {1} [File1](chapter1/file1.md)\n",
            TITLE, LIST_CHAR
        );

        assert_eq!(
            expected,
            book(&["file1.md", "chapter1/file1.md"]).get_summary_file(&FORMAT, &None, false)
        );
    }

    #[test]
    fn md_output_subchapter_test() {
        // only one file
        let expected: &str = &format!(
            "# {0}\n\n{1} Chapter1\n    {1} [File1](chapter1/file1.md)\n    {1} Subchap\n        {1} [File1](chapter1/subchap/file1.md)\n",
            TITLE, LIST_CHAR
        );

        assert_eq!(
            expected,
            book(&["chapter1/file1.md", "chapter1/subchap/file1.md"])
                .get_summary_file(&FORMAT, &None, false)
        );
    }

    #[test]
    fn md_simple_structure_test() {
        let input = files(&[
            "part1/README.md",
            "part1/WritingIsGood.md",
            "part1/GitbookIsNice.md",
            "part2/README.md",
            "part2/First_part_of_part_2.md",
            "part2/Second_part_of_part_2.md",
        ]);

        let expected = r#"# Summary

* [Part1](part1/README.md)
    * [WritingIsGood](part1/WritingIsGood.md)
    * [GitbookIsNice](part1/GitbookIsNice.md)
* [Part2](part2/README.md)
    * [First Part of Part 2](part2/First_part_of_part_2.md)
    * [Second Part of Part 2](part2/Second_part_of_part_2.md)
"#;

        let book = Chapter::new(TITLE.to_string(), &input, false);

        assert_eq!(expected, book.get_summary_file(&FORMAT, &None, false));
    }

    #[test]
    fn parse_config_test() {
        let bookjson = "./examples/gitbook/book.json";
        let booktoml = "./examples/mdbook/book.toml";

        // opt with default values
        let mut opt = default_opt();
        opt.verbose = 3;

        assert!(parse_config_file(booktoml, &mut opt).is_ok());

        assert_eq!("src", format!("{}", opt.dir.display()));
        assert_eq!("MyMDBook", opt.title);

        opt.dir = PathBuf::from(".");
        opt.title = "Summary".to_string();

        assert!(parse_config_file(bookjson, &mut opt).is_ok());

        assert_eq!("book", format!("{}", opt.dir.display()));
        assert_eq!("My title", opt.title);
    }

    #[test]
    fn parse_config_accepts_mdbook_config_with_empty_src() -> io::Result<()> {
        let dir = test_dir("empty-src-mdbook-config")?;
        let booktoml = dir.join("book.toml");
        std::fs::write(
            &booktoml,
            r#"[book]
authors = ["Example Author"]
language = "zh-CN"
src = ""
title = "Example Book"
site-url = "/example-book/"

[output.html]
no-section-label = true
git-repository-url = "https://example.com/example/book"

[output.html.print]
enable = false

[output.html.fold]
enable = true
level = 10

[preprocessor.chapter-list]
ignored-files = ["chapters/volume-0/markdown.md"]
"#,
        )?;

        let mut opt = default_opt();
        assert!(parse_config_file(&booktoml.to_string_lossy(), &mut opt).is_ok());

        assert_eq!(PathBuf::from("."), opt.dir);
        assert_eq!("Example Book", opt.title);
        Ok(())
    }

    #[test]
    fn parse_config_missing_file_is_ok() {
        let mut opt = default_opt();

        assert!(parse_config_file("./missing-book-config.toml", &mut opt).is_ok());
        assert_eq!(PathBuf::from("."), opt.dir);
        assert_eq!("Summary", opt.title);
    }

    #[test]
    fn parse_config_accepts_toml_without_book_section() -> io::Result<()> {
        let dir = test_dir("toml-without-book-section")?;
        let booktoml = dir.join("book.toml");
        std::fs::write(
            &booktoml,
            r#"[output.html]
no-section-label = true
"#,
        )?;

        let mut opt = default_opt();
        assert!(parse_config_file(&booktoml.to_string_lossy(), &mut opt).is_ok());
        assert_eq!(PathBuf::from("."), opt.dir);
        assert_eq!("Summary", opt.title);
        Ok(())
    }

    #[test]
    fn parse_config_reports_invalid_content() -> io::Result<()> {
        let dir = test_dir("invalid-config")?;
        let invalid_toml = dir.join("book.toml");
        let invalid_json = dir.join("book.json");
        std::fs::write(&invalid_toml, "book = [")?;
        std::fs::write(&invalid_json, "{")?;

        let mut opt = default_opt();
        let toml_err = parse_config_file(&invalid_toml.to_string_lossy(), &mut opt);
        let json_err = parse_config_file(&invalid_json.to_string_lossy(), &mut opt);

        assert!(toml_err.is_err());
        assert!(json_err.is_err());
        Ok(())
    }

    #[test]
    fn marker_helpers_wrap_append_and_replace_content() {
        let wrapped = wrap_with_marker("# Generated", "auto");
        assert_eq!(
            "<!-- book-summary-start-auto -->\n# Generated\n<!-- book-summary-end-auto -->",
            wrapped
        );

        let without_markers = "# Manual";
        assert_eq!(
            "# Manual\n\n<!-- book-summary-start-auto -->\n# Generated\n<!-- book-summary-end-auto -->",
            update_partial_summary(without_markers, "# Generated", "auto")
        );

        let existing = "# Manual\n<!-- book-summary-start-auto -->\nold\n<!-- book-summary-end-auto -->\n# End";
        assert_eq!(
            "# Manual\n<!-- book-summary-start-auto --># Generated\n<!-- book-summary-end-auto -->\n# End",
            update_partial_summary(existing, "# Generated", "auto")
        );
    }

    #[test]
    fn create_file_writes_and_updates_marker_content() -> io::Result<()> {
        let dir = test_dir("create-file")?;

        create_file(&dir, "SUMMARY.md", "# First", None)?;
        assert_eq!("# First", std::fs::read_to_string(dir.join("SUMMARY.md"))?);

        create_file(&dir, "MARKED.md", "# Generated", Some("auto"))?;
        assert_eq!(
            "<!-- book-summary-start-auto -->\n# Generated\n<!-- book-summary-end-auto -->",
            std::fs::read_to_string(dir.join("MARKED.md"))?
        );

        create_file(&dir, "MARKED.md", "# Updated", Some("auto"))?;
        assert_eq!(
            "<!-- book-summary-start-auto --># Updated\n<!-- book-summary-end-auto -->",
            std::fs::read_to_string(dir.join("MARKED.md"))?
        );

        Ok(())
    }

    #[test]
    fn sort_chapter_test() {
        let input = files(&[
            "part1/README.md",
            "part1/WritingIsGood.md",
            "part2/GitbookIsNice.md",
            "part2/README.md",
            "part3/file.md",
            "part4/file.md",
        ]);

        let expected = r#"# Summary

* Part4
    * [File](part4/file.md)
* Part3
    * [File](part3/file.md)
* [Part1](part1/README.md)
    * [WritingIsGood](part1/WritingIsGood.md)
* [Part2](part2/README.md)
    * [GitbookIsNice](part2/GitbookIsNice.md)
"#;

        let book = Chapter::new(TITLE.to_string(), &input, false);

        assert_eq!(
            expected,
            book.get_summary_file(
                &FORMAT,
                &Some(vec![
                    "PART4".to_string(),
                    "part5".to_string(),
                    "part3".to_string()
                ]),
                false
            )
        );
    }
}
