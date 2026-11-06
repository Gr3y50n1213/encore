//! encore-tokens generate [--tokens <tokens.json>] [--out <dir>]
//! encore-tokens check    [--tokens <tokens.json>] [--out <dir>]
//!
//! `generate` writes the Swift, Kotlin and Dart constants. `check` writes nothing and
//! fails if the files in `--out` differ from what `generate` would write (CI runs it).
//! Defaults: `design/tokens.json` and `core/bindings/` in this repository.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("encore-tokens: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, String> {
    let mut mode = None;
    let mut tokens = repo_root().join("design").join("tokens.json");
    let mut out = repo_root().join("core").join("bindings");
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "generate" | "check" if mode.is_none() => mode = Some(arg),
            "--tokens" => tokens = it.next().ok_or("--tokens needs a path")?.into(),
            "--out" => out = it.next().ok_or("--out needs a folder")?.into(),
            other => {
                return Err(format!(
                    "unknown argument `{other}`; use `generate` or `check`"
                ));
            }
        }
    }
    let mode = mode.ok_or("say `generate` or `check`")?;

    let json = fs::read_to_string(&tokens)
        .map_err(|e| format!("cannot read {}: {e}", tokens.display()))?;
    let files = encore_tokens::generate(&json)?;

    if mode == "generate" {
        for (name, text) in &files {
            let path = out.join(name);
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)
                    .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            }
            fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            println!("wrote {}", path.display());
        }
        return Ok(ExitCode::SUCCESS);
    }

    let mut stale = Vec::new();
    for (name, text) in &files {
        let path = out.join(name);
        if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            stale.push(path.display().to_string());
        }
    }
    if stale.is_empty() {
        println!("token constants are up to date ({} files)", files.len());
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("token constants are out of date; run `make tokens` in core/:");
        for s in stale {
            eprintln!("  {s}");
        }
        Ok(ExitCode::FAILURE)
    }
}
