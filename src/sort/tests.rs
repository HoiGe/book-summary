use super::*;

fn assert_sorted(input: &[&str], expected: &[&str]) {
    let mut paths: Vec<String> = input.iter().map(|path| path.to_string()).collect();
    paths.sort_by(|a, b| compare_paths(a, b));
    assert_eq!(
        expected
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>(),
        paths
    );
}

#[test]
fn parse_structured_filename() {
    let filename = "0.1.131313.md";
    assert_eq!(
        Some((0, 1, "131313".to_string(), filename.to_string())),
        parse_filename(filename)
    );
}

#[test]
fn file_sort_cases() {
    let cases = [
        (
            &[
                "chapter10.md",
                "0.10.structured.md",
                "chapter2.md",
                "0.2.structured.md",
            ][..],
            &[
                "0.2.structured.md",
                "0.10.structured.md",
                "chapter2.md",
                "chapter10.md",
            ][..],
        ),
        (
            &[
                "第二十章.md",
                "第十章.md",
                "第2章.md",
                "第二章.md",
                "第11章.md",
            ][..],
            &[
                "第2章.md",
                "第二章.md",
                "第十章.md",
                "第11章.md",
                "第二十章.md",
            ][..],
        ),
        (
            &["第壹佰章.md", "第贰拾章.md", "第壹章.md", "第壹拾章.md"][..],
            &["第壹章.md", "第壹拾章.md", "第贰拾章.md", "第壹佰章.md"][..],
        ),
        (
            &[
                "第一万章.md",
                "第一千零二章.md",
                "第一百零一章.md",
                "第九十九章.md",
                "第二章.md",
                "第十一章.md",
            ][..],
            &[
                "第二章.md",
                "第十一章.md",
                "第九十九章.md",
                "第一百零一章.md",
                "第一千零二章.md",
                "第一万章.md",
            ][..],
        ),
        (
            &["第〇一〇章.md", "第九章.md", "第一二章.md", "第一〇一章.md"][..],
            &["第九章.md", "第〇一〇章.md", "第一二章.md", "第一〇一章.md"][..],
        ),
        (
            &[
                "番外10.md",
                "番外第二话.md",
                "番外第十一话.md",
                "番外3.md",
                "番外第壹佰话.md",
            ][..],
            &[
                "番外第二话.md",
                "番外3.md",
                "番外10.md",
                "番外第十一话.md",
                "番外第壹佰话.md",
            ][..],
        ),
        (
            &["zeta.md", "alpha.md", "middle.md"][..],
            &["zeta.md", "alpha.md", "middle.md"][..],
        ),
        (
            &["拾光仟陌.md", "普通文件.md", "仟山拾月.md"][..],
            &["拾光仟陌.md", "普通文件.md", "仟山拾月.md"][..],
        ),
        (
            &["五十载人.md", "普通文件.md", "二十面体.md"][..],
            &["五十载人.md", "普通文件.md", "二十面体.md"][..],
        ),
        (
            &["五十.标题.md", "第五十章.md", "二十.md", "第二十话.md"][..],
            &["二十.md", "第二十话.md", "第五十章.md", "五十.标题.md"][..],
        ),
        (
            &["第十辑.md", "第二册.md", "第三折.md"][..],
            &["第二册.md", "第三折.md", "第十辑.md"][..],
        ),
    ];

    for (input, expected) in cases {
        assert_sorted(input, expected);
    }
}
