use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTarget {
    pub name: String,
    pub relative_path: String,
    pub directory: PathBuf,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Bun,
    Pnpm,
    Yarn,
    Npm,
}

impl PackageManager {
    pub fn command_name(&self) -> &'static str {
        match self {
            PackageManager::Bun => "bun",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Npm => "npm",
        }
    }

    pub fn dev_args(&self) -> Vec<String> {
        match self {
            PackageManager::Bun => vec!["run".to_string(), "dev".to_string()],
            PackageManager::Pnpm => vec!["run".to_string(), "dev".to_string()],
            PackageManager::Yarn => vec!["run".to_string(), "dev".to_string()],
            PackageManager::Npm => vec!["run".to_string(), "dev".to_string()],
        }
    }
}

pub fn detect_package_manager(root: &Path) -> PackageManager {
    if root.join("bun.lock").exists() || root.join("bun.lockb").exists() {
        PackageManager::Bun
    } else if root.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if root.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

#[derive(Deserialize, Debug)]
struct RootPackageJson {
    workspaces: Option<WorkspacesField>,
    scripts: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum WorkspacesField {
    List(Vec<String>),
    Config { packages: Option<Vec<String>> },
}

impl WorkspacesField {
    fn patterns(&self) -> Vec<String> {
        match self {
            WorkspacesField::List(patterns) => patterns.clone(),
            WorkspacesField::Config { packages } => packages.clone().unwrap_or_default(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct ChildPackageJson {
    name: Option<String>,
    scripts: Option<HashMap<String, String>>,
}

pub fn discover_services(root: &Path) -> Result<Vec<ServiceTarget>> {
    let pm = detect_package_manager(root);
    let root_pkg_path = root.join("package.json");

    if !root_pkg_path.exists() {
        anyhow::bail!("No package.json found at {}", root.display());
    }

    let root_content = fs::read_to_string(&root_pkg_path)
        .with_context(|| format!("Failed to read {}", root_pkg_path.display()))?;
    let root_pkg: RootPackageJson = serde_json::from_str(&root_content)
        .with_context(|| format!("Failed to parse {}", root_pkg_path.display()))?;

    let mut services = Vec::new();

    if let Some(workspaces) = root_pkg.workspaces {
        let patterns = workspaces.patterns();
        for pattern in patterns {
            let glob_path = root.join(&pattern);
            let glob_str = glob_path.to_string_lossy();

            for entry in glob::glob(&glob_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
                if let Ok(path) = entry {
                    if path.is_dir() {
                        if let Some(target) = inspect_package_dir(root, &path, pm) {
                            services.push(target);
                        }
                    }
                }
            }
        }
    }

    // Fallback: If no workspace dev services were discovered, check if the root itself has a dev script
    if services.is_empty() {
        if let Some(scripts) = root_pkg.scripts {
            if scripts.contains_key("dev") {
                services.push(ServiceTarget {
                    name: "root".to_string(),
                    relative_path: ".".to_string(),
                    directory: root.to_path_buf(),
                    command: pm.command_name().to_string(),
                    args: pm.dev_args(),
                });
            }
        }
    }

    // Sort stably by relative path for consistent sidebar ordering
    services.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(services)
}

fn inspect_package_dir(root: &Path, dir: &Path, pm: PackageManager) -> Option<ServiceTarget> {
    let pkg_file = dir.join("package.json");
    if !pkg_file.exists() {
        return None;
    }

    let content = fs::read_to_string(&pkg_file).ok()?;
    let pkg: ChildPackageJson = serde_json::from_str(&content).ok()?;

    let scripts = pkg.scripts?;
    if !scripts.contains_key("dev") {
        return None;
    }

    let relative = dir.strip_prefix(root).ok()?;
    let relative_str = relative.to_string_lossy().to_string();

    Some(ServiceTarget {
        name: pkg.name.unwrap_or_else(|| relative_str.clone()),
        relative_path: relative_str,
        directory: dir.to_path_buf(),
        command: pm.command_name().to_string(),
        args: pm.dev_args(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_dir() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tx_test_{}", now));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_detect_package_manager() {
        let dir = create_test_dir();
        assert_eq!(detect_package_manager(&dir), PackageManager::Npm);

        fs::write(dir.join("bun.lock"), "").unwrap();
        assert_eq!(detect_package_manager(&dir), PackageManager::Bun);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_workspaces_with_dev_filter() {
        let root = create_test_dir();

        // Root package.json with workspaces
        fs::write(
            root.join("package.json"),
            r#"{"name": "test-repo", "workspaces": ["apps/*", "libs/ui"]}"#,
        )
        .unwrap();
        fs::write(root.join("bun.lock"), "").unwrap();

        // App 1: has dev script
        let app1 = root.join("apps").join("backend");
        fs::create_dir_all(&app1).unwrap();
        fs::write(
            app1.join("package.json"),
            r#"{"name": "@test/backend", "scripts": {"dev": "bun run src/index.ts"}}"#,
        )
        .unwrap();

        // App 2: has dev script
        let app2 = root.join("apps").join("frontend");
        fs::create_dir_all(&app2).unwrap();
        fs::write(
            app2.join("package.json"),
            r#"{"name": "@test/frontend", "scripts": {"dev": "vite", "build": "vite build"}}"#,
        )
        .unwrap();

        // Lib: has build script but NO dev script -> MUST be ignored
        let lib = root.join("libs").join("ui");
        fs::create_dir_all(&lib).unwrap();
        fs::write(
            lib.join("package.json"),
            r#"{"name": "@test/ui", "scripts": {"build": "tsc"}}"#,
        )
        .unwrap();

        let services = discover_services(&root).unwrap();
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].relative_path, "apps/backend");
        assert_eq!(services[0].command, "bun");
        assert_eq!(services[1].relative_path, "apps/frontend");
        assert_eq!(services[1].command, "bun");

        let _ = fs::remove_dir_all(&root);
    }
}
