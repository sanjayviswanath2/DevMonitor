use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::project::ProjectInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvGaps {
    pub example_file: String,
    pub missing_keys: Vec<String>,
    pub present_files: Vec<String>,
}

pub fn scan_env_gaps(project: &ProjectInfo) -> Option<EnvGaps> {
    let root = Path::new(&project.path);
    let example_name = project.env_example.as_deref()?;
    let example_path = root.join(example_name);
    let example_keys = parse_env_keys(&example_path);
    if example_keys.is_empty() {
        return None;
    }

    let candidates = [
        ".env",
        ".env.local",
        ".env.development",
        ".env.development.local",
        ".env.production",
        ".env.production.local",
    ];
    let mut present_files = Vec::new();
    let mut present_keys = BTreeSet::new();
    for name in candidates {
        let p = root.join(name);
        if p.exists() {
            present_files.push(name.to_string());
            for k in parse_env_keys(&p) {
                present_keys.insert(k);
            }
        }
    }

    let missing: Vec<String> = example_keys
        .into_iter()
        .filter(|k| !present_keys.contains(k))
        .collect();

    if missing.is_empty() {
        return None;
    }

    Some(EnvGaps {
        example_file: example_name.to_string(),
        missing_keys: missing,
        present_files,
    })
}

fn parse_env_keys(path: &Path) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = t.split_once('=') {
            let key = key.trim();
            if !key.is_empty() && !key.contains(' ') {
                keys.push(key.to_string());
            }
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn detects_missing_env_keys() {
        let dir = std::env::temp_dir().join(format!("dm-env-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".env.example"), "FOO=1\nBAR=2\n# comment\nBAZ=\n").unwrap();
        fs::write(dir.join(".env"), "FOO=hello\n").unwrap();

        let project = crate::doctor::project::ProjectInfo {
            path: dir.display().to_string(),
            name: "t".into(),
            package_manager: "npm".into(),
            package_manager_field: None,
            has_package_json: true,
            has_node_modules: false,
            lockfile: Some("package-lock.json".into()),
            lockfile_count: 1,
            engines_node: None,
            nvmrc: None,
            node_version_file: None,
            required_node: None,
            env_example: Some(".env.example".into()),
            python_required: None,
            has_native_deps_hint: false,
        };

        let gaps = scan_env_gaps(&project).expect("gaps");
        assert!(gaps.missing_keys.contains(&"BAR".into()));
        assert!(gaps.missing_keys.contains(&"BAZ".into()));
        assert!(!gaps.missing_keys.contains(&"FOO".into()));
        let _ = fs::remove_dir_all(&dir);
        let _: PathBuf = dir;
    }
}
