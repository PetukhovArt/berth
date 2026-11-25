use berth_core::platform;
use std::path::{Path, PathBuf};

pub fn unwrap(program: PathBuf) -> (PathBuf, Vec<String>) {
    npm_target(&program).unwrap_or((program, Vec::new()))
}

fn npm_target(program: &Path) -> Option<(PathBuf, Vec<String>)> {
    let is_batch = program
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"));
    if !is_batch {
        return None;
    }
    let dir = program.parent()?;
    let target = shim_target(&std::fs::read_to_string(program).ok()?, dir)?;
    if !target.is_file() {
        return None;
    }
    let is_script = target
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ["js", "mjs", "cjs"].contains(&ext.to_ascii_lowercase().as_str()));
    if !is_script {
        return Some((target, Vec::new()));
    }
    let bundled = dir.join("node.exe");
    let node = if bundled.is_file() {
        bundled
    } else {
        platform::resolve_program("node")?
    };
    Some((node, vec![target.to_string_lossy().into_owned()]))
}

fn shim_target(text: &str, dir: &Path) -> Option<PathBuf> {
    let launch = text.lines().rev().find(|line| line.contains("%*"))?;
    let target = launch
        .split('"')
        .skip(1)
        .step_by(2)
        .find(|part| part.contains("%dp0%") || part.contains("%~dp0"))?;
    let dir = dir.to_string_lossy();
    let path = target.replace("%dp0%", &dir).replace("%~dp0", &dir);
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NPM_SHIM: &str = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\r\nIF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n) ELSE (\r\n  SET \"_prog=node\"\r\n  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n)\r\n\r\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\cli.js\" %*\r\n";

    #[test]
    fn an_npm_shim_names_the_script_it_wraps() {
        let target = shim_target(NPM_SHIM, Path::new("C:/npm"));
        assert_eq!(
            target,
            Some(PathBuf::from(
                "C:/npm\\node_modules\\@anthropic-ai\\claude-code\\cli.js"
            ))
        );
    }

    #[test]
    fn a_shim_unwraps_to_node_and_its_script() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir
            .path()
            .join("node_modules/@anthropic-ai/claude-code/cli.js");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, "").unwrap();
        std::fs::write(dir.path().join("node.exe"), "").unwrap();
        let shim = dir.path().join("claude.cmd");
        std::fs::write(&shim, NPM_SHIM).unwrap();
        let (program, lead) = unwrap(shim);
        assert_eq!(program, dir.path().join("node.exe"));
        assert_eq!(lead.len(), 1);
        assert!(Path::new(&lead[0]).ends_with("claude-code/cli.js"));
    }

    #[test]
    fn a_batch_file_that_is_not_an_npm_shim_has_no_target() {
        assert_eq!(
            shim_target("@echo off\r\necho hi\r\n", Path::new("C:/x")),
            None
        );
    }
}
