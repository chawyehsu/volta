use std::path::{Path, PathBuf};

use node_semver::{Range, Version};

use crate::error::{ErrorKind, Fallible};
use crate::hook::ToolHooks;
use crate::tool::Node;
use crate::tool::node::resolve_with_hooks;
use crate::version::VersionSpec;

#[derive(Debug)]
pub(crate) struct NodeVersionSource {
    path: PathBuf,
    spec: VersionSpec,
}

impl NodeVersionSource {
    pub(crate) fn from_file(path: PathBuf) -> Fallible<Self> {
        let contents = std::fs::read_to_string(&path)
            .map_err(|_| ErrorKind::DotNodeVersionMalformed { file: path.clone() })?;
        let spec = parse_contents(&contents)
            .ok_or_else(|| ErrorKind::DotNodeVersionMalformed { file: path.clone() })?;

        Ok(Self { path, spec })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn resolve(&self, hooks: Option<&ToolHooks<Node>>) -> Fallible<Version> {
        resolve_with_hooks(self.spec.clone(), hooks).map_err(|error| match error.kind() {
            ErrorKind::NodeVersionNotFound { matching } => ErrorKind::NodeVersionNotFoundInFile {
                matching: matching.clone(),
                file: self.path.clone(),
            }
            .into(),
            _ => error,
        })
    }
}

pub(crate) fn find_node_version_file(start: &Path, project_root: Option<&Path>) -> Option<PathBuf> {
    let mut current = Some(start);

    while let Some(dir) = current {
        if !is_inside_node_modules(dir) {
            let candidate = dir.join(".node-version");
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        if project_root == Some(dir) {
            break;
        }

        current = dir.parent();
    }

    None
}

fn is_inside_node_modules(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor
            .file_name()
            .is_some_and(|name| name == "node_modules")
    })
}

fn parse_contents(contents: &str) -> Option<VersionSpec> {
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let version = contents.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    let parts: Vec<&str> = version.split('.').collect();

    if parts.is_empty() || parts.len() > 3 {
        return None;
    }

    if parts.iter().any(|part| {
        part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
    }) {
        return None;
    }

    match parts.as_slice() {
        [major] => Range::parse(format!("{major}.x"))
            .ok()
            .map(VersionSpec::Semver),
        [major, minor] => Range::parse(format!("{major}.{minor}.x"))
            .ok()
            .map(VersionSpec::Semver),
        _ => Version::parse(version).ok().map(VersionSpec::Exact),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(parts: &[&str]) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("fixtures");
        for part in parts {
            path.push(part);
        }
        path
    }

    #[test]
    fn parses_exact_versions() {
        assert_eq!(
            parse_contents("6.11.0"),
            Some(VersionSpec::Exact("6.11.0".parse().unwrap()))
        );
        assert_eq!(
            parse_contents("v6.11.0"),
            Some(VersionSpec::Exact("6.11.0".parse().unwrap()))
        );
        assert_eq!(
            parse_contents("\u{feff}  v6.11.0\r\n"),
            Some(VersionSpec::Exact("6.11.0".parse().unwrap()))
        );
    }

    #[test]
    fn parses_partial_versions() {
        assert_eq!(
            parse_contents("6"),
            Some(VersionSpec::Semver(Range::parse("6.x").unwrap()))
        );
        assert_eq!(
            parse_contents("v6.11"),
            Some(VersionSpec::Semver(Range::parse("6.11.x").unwrap()))
        );
    }

    #[test]
    fn rejects_unsupported_content() {
        for contents in [
            "",
            " ",
            "v",
            "lts/*",
            "20.x",
            ">=20",
            "20.0.0-rc.1",
            "20.0.0+build",
            "20.05.0",
            "20.5.0.1",
            "20.5.0 # comment",
            "20.5.0\n21.0.0",
        ] {
            assert_eq!(parse_contents(contents), None, "accepted {contents:?}");
        }
    }

    #[test]
    fn reads_node_version_file() {
        let path = fixture_path(&["dot-node-version", "only-dot-node-version", ".node-version"]);
        let source = NodeVersionSource::from_file(path.clone()).unwrap();

        assert_eq!(source.path(), path);
        assert_eq!(source.spec, VersionSpec::Exact("6.11.0".parse().unwrap()));
    }

    #[test]
    fn reads_node_version_file_with_leading_v() {
        let path = fixture_path(&[
            "dot-node-version",
            "only-dot-node-version-with-v",
            ".node-version",
        ]);
        let source = NodeVersionSource::from_file(path).unwrap();

        assert_eq!(source.spec, VersionSpec::Exact("6.11.0".parse().unwrap()));
    }

    #[test]
    fn rejects_empty_node_version_file() {
        let path = fixture_path(&[
            "dot-node-version",
            "empty-dot-node-version",
            ".node-version",
        ]);
        let error = NodeVersionSource::from_file(path.clone()).unwrap_err();

        match error.kind() {
            ErrorKind::DotNodeVersionMalformed { file } => assert_eq!(file, &path),
            kind => panic!("Wrong error kind: {kind:?}"),
        }
    }

    #[test]
    fn rejects_malformed_node_version_file() {
        let path = fixture_path(&[
            "dot-node-version",
            "malformed-dot-node-version",
            ".node-version",
        ]);
        let error = NodeVersionSource::from_file(path.clone()).unwrap_err();

        match error.kind() {
            ErrorKind::DotNodeVersionMalformed { file } => assert_eq!(file, &path),
            kind => panic!("Wrong error kind: {kind:?}"),
        }
    }

    #[test]
    fn finds_nearest_file_up_to_project_root() {
        let root = fixture_path(&["dot-node-version", "walk-up"]);
        let start = root.join("subdir");

        assert_eq!(
            find_node_version_file(&start, Some(&root)),
            Some(root.join(".node-version"))
        );
    }

    #[test]
    fn does_not_search_above_project_root() {
        let root = fixture_path(&["dot-node-version", "walk-up"]);
        let start = root.join("subdir");

        assert_eq!(find_node_version_file(&start, Some(&start)), None);
    }

    #[test]
    fn ignores_node_modules() {
        let root = fixture_path(&["dot-node-version", "walk-up"]);
        let start = root.join("node_modules").join("dependency");

        assert_eq!(
            find_node_version_file(&start, Some(&root)),
            Some(root.join(".node-version"))
        );
    }

    #[test]
    fn walks_to_filesystem_root_without_project() {
        let root = fixture_path(&["dot-node-version", "walk-up"]);
        let start = root.join("subdir");

        assert_eq!(
            find_node_version_file(&start, None),
            Some(root.join(".node-version"))
        );
    }
}
