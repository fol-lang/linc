use std::path::PathBuf;

use bic::{to_json, HeaderConfig, PreprocessedInput};

fn main() {
    if let Err(message) = run(std::env::args().skip(1).collect()) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let Some((command, rest)) = args.split_first() else {
        return Err(usage());
    };

    match command.as_str() {
        "scan" => run_scan(rest),
        "scan-preprocessed" => run_scan_preprocessed(rest),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{}", usage())),
    }
}

fn run_scan(args: &[String]) -> Result<(), String> {
    let mut cfg = HeaderConfig::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--header" => {
                i += 1;
                cfg = cfg.header(required_value(args, i, "--header")?);
            }
            "--include-dir" => {
                i += 1;
                cfg = cfg.include_dir(required_value(args, i, "--include-dir")?);
            }
            "--library-dir" => {
                i += 1;
                cfg = cfg.library_dir(required_value(args, i, "--library-dir")?);
            }
            "--define" => {
                i += 1;
                let define = required_value(args, i, "--define")?;
                let (name, value) = parse_define(define);
                cfg = cfg.define(name, value);
            }
            "--link-lib" => {
                i += 1;
                cfg = cfg.link_lib(required_value(args, i, "--link-lib")?);
            }
            "--link-static-lib" => {
                i += 1;
                cfg = cfg.link_static_lib(required_value(args, i, "--link-static-lib")?);
            }
            "--link-shared-lib" => {
                i += 1;
                cfg = cfg.link_shared_lib(required_value(args, i, "--link-shared-lib")?);
            }
            "--compiler" => {
                i += 1;
                cfg = cfg.compiler(required_value(args, i, "--compiler")?);
            }
            "--flavor" => {
                i += 1;
                cfg = cfg.flavor(parse_header_flavor(required_value(args, i, "--flavor")?)?);
            }
            "--no-origin-filter" => {
                cfg = cfg.no_origin_filter();
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other => {
                return Err(format!("unknown scan option '{other}'"));
            }
        }
        i += 1;
    }

    let result = cfg.process()?;
    println!("{}", to_json(&result.package)?);
    Ok(())
}

fn run_scan_preprocessed(args: &[String]) -> Result<(), String> {
    let mut file: Option<PathBuf> = None;
    let mut source_path: Option<String> = None;
    let mut flavor = pac::driver::Flavor::GnuC11;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--file" => {
                i += 1;
                file = Some(PathBuf::from(required_value(args, i, "--file")?));
            }
            "--source-path" => {
                i += 1;
                source_path = Some(required_value(args, i, "--source-path")?.to_string());
            }
            "--flavor" => {
                i += 1;
                flavor = parse_pac_flavor(required_value(args, i, "--flavor")?)?;
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            other => {
                return Err(format!("unknown scan-preprocessed option '{other}'"));
            }
        }
        i += 1;
    }

    let file = file.ok_or_else(|| "scan-preprocessed requires --file".to_string())?;
    let mut input = PreprocessedInput::from_file(&file).map_err(|e| e.to_string())?;
    if let Some(path) = source_path {
        input = input.with_path(path);
    }
    input = input.with_flavor(flavor);
    println!("{}", to_json(&input.extract())?);
    Ok(())
}

fn required_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(|value| value.as_str())
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_define(define: &str) -> (String, Option<String>) {
    match define.split_once('=') {
        Some((name, value)) => (name.to_string(), Some(value.to_string())),
        None => (define.to_string(), None),
    }
}

fn parse_header_flavor(value: &str) -> Result<bic::raw_headers::Flavor, String> {
    match value {
        "gnu" | "gnu-c11" => Ok(bic::raw_headers::Flavor::GnuC11),
        "clang" | "clang-c11" => Ok(bic::raw_headers::Flavor::ClangC11),
        "std" | "std-c11" => Ok(bic::raw_headers::Flavor::StdC11),
        other => Err(format!("unsupported header flavor '{other}'")),
    }
}

fn parse_pac_flavor(value: &str) -> Result<pac::driver::Flavor, String> {
    match value {
        "gnu" | "gnu-c11" => Ok(pac::driver::Flavor::GnuC11),
        "clang" | "clang-c11" => Ok(pac::driver::Flavor::ClangC11),
        "std" | "std-c11" => Ok(pac::driver::Flavor::StdC11),
        other => Err(format!("unsupported preprocessed flavor '{other}'")),
    }
}

fn usage() -> String {
    [
        "Usage:",
        "  bic scan --header <path> [options]",
        "  bic scan-preprocessed --file <path> [options]",
        "",
        "scan options:",
        "  --header <path>",
        "  --include-dir <path>",
        "  --library-dir <path>",
        "  --define NAME[=VALUE]",
        "  --link-lib <name>",
        "  --link-static-lib <name>",
        "  --link-shared-lib <name>",
        "  --compiler <cmd>",
        "  --flavor <gnu|clang|std>",
        "  --no-origin-filter",
        "",
        "scan-preprocessed options:",
        "  --file <path>",
        "  --source-path <path>",
        "  --flavor <gnu|clang|std>",
    ]
    .join("\n")
}

