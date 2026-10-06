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

    pub fn script_args(&self, script: &str) -> Vec<String> {
        match self {
            PackageManager::Bun => vec!["run".to_string(), script.to_string()],
            PackageManager::Pnpm => vec!["run".to_string(), script.to_string()],
            PackageManager::Yarn => vec!["run".to_string(), script.to_string()],
            PackageManager::Npm => vec!["run".to_string(), script.to_string()],
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

pub fn discover_services(root: &Path, script: &str) -> Result<Vec<ServiceTarget>> {
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
                        if let Some(target) = inspect_package_dir(root, &path, pm, script) {
                            services.push(target);
                        }
                    }
                }
            }
        }
    }

    // Fallback: If no workspace targets discovered, check if the root itself has the script
    if services.is_empty() {
        if let Some(scripts) = root_pkg.scripts {
            if scripts.contains_key(script) {
                services.push(ServiceTarget {
                    name: "root".to_string(),
                    relative_path: ".".to_string(),
                    directory: root.to_path_buf(),
                    command: pm.command_name().to_string(),
                    args: pm.script_args(script),
                });
            }
        }
    }

    // Sort stably by relative path for consistent sidebar ordering
    services.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(services)
}

fn inspect_package_dir(
    root: &Path,
    dir: &Path,
    pm: PackageManager,
    script: &str,
) -> Option<ServiceTarget> {
    let pkg_file = dir.join("package.json");
    if !pkg_file.exists() {
        return None;
    }

    let content = fs::read_to_string(&pkg_file).ok()?;
    let pkg: ChildPackageJson = serde_json::from_str(&content).ok()?;

    let scripts = pkg.scripts?;
    if !scripts.contains_key(script) {
        return None;
    }

    let relative = dir.strip_prefix(root).ok()?;
    let relative_str = relative.to_string_lossy().to_string();

    Some(ServiceTarget {
        name: pkg.name.unwrap_or_else(|| relative_str.clone()),
        relative_path: relative_str,
        directory: dir.to_path_buf(),
        command: pm.command_name().to_string(),
        args: pm.script_args(script),
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
    fn test_discover_workspaces_with_custom_script() {
        let root = create_test_dir();

        fs::write(
            root.join("package.json"),
            r#"{"name": "test-repo", "workspaces": ["apps/*", "libs/ui"]}"#,
        )
        .unwrap();
        fs::write(root.join("bun.lock"), "").unwrap();

        // App 1: has dev, test, build
        let app1 = root.join("apps").join("backend");
        fs::create_dir_all(&app1).unwrap();
        fs::write(
            app1.join("package.json"),
            r#"{"name": "@test/backend", "scripts": {"dev": "bun run src/index.ts", "test": "bun test"}}"#,
        )
        .unwrap();

        // App 2: has dev only
        let app2 = root.join("apps").join("frontend");
        fs::create_dir_all(&app2).unwrap();
        fs::write(
            app2.join("package.json"),
            r#"{"name": "@test/frontend", "scripts": {"dev": "vite", "build": "vite build"}}"#,
        )
        .unwrap();

        // App 3: has test only
        let app3 = root.join("libs").join("ui");
        fs::create_dir_all(&app3).unwrap();
        fs::write(
            app3.join("package.json"),
            r#"{"name": "@test/ui", "scripts": {"test": "vitest", "build": "tsc"}}"#,
        )
        .unwrap();

        // Test running "dev": should find backend and frontend, but NOT ui
        let dev_services = discover_services(&root, "dev").unwrap();
        assert_eq!(dev_services.len(), 2);
        assert_eq!(dev_services[0].relative_path, "apps/backend");
        assert_eq!(dev_services[0].args, vec!["run", "dev"]);
        assert_eq!(dev_services[1].relative_path, "apps/frontend");

        // Test running "test": should find backend and ui, but NOT frontend!
        let test_services = discover_services(&root, "test").unwrap();
        assert_eq!(test_services.len(), 2);
        assert_eq!(test_services[0].relative_path, "apps/backend");
        assert_eq!(test_services[0].args, vec!["run", "test"]);
        assert_eq!(test_services[1].relative_path, "libs/ui");
        assert_eq!(test_services[1].args, vec!["run", "test"]);

        // Test running "nonexistent": should find nothing
        let none = discover_services(&root, "nonexistent").unwrap();
        assert_eq!(none.len(), 0);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_discover_real_traki_monorepo() {
        let t_path = PathBuf::from("/Users/muthu/Desktop/Projects/T");
        if t_path.exists() {
            let dev_services = discover_services(&t_path, "dev").unwrap();
            let dev_paths: Vec<String> =
                dev_services.into_iter().map(|s| s.relative_path).collect();
            assert_eq!(dev_paths, vec!["apps/admin", "apps/backend", "apps/frontend"]);

            // In T: apps/admin, apps/backend, and ui have typecheck, but apps/frontend does not
            let typecheck_services = discover_services(&t_path, "typecheck").unwrap();
            let tc_paths: Vec<String> = typecheck_services
                .into_iter()
                .map(|s| s.relative_path)
                .collect();
            assert!(tc_paths.contains(&"apps/admin".to_string()));
            assert!(tc_paths.contains(&"apps/backend".to_string()));
            assert!(tc_paths.contains(&"ui".to_string()));
            assert!(!tc_paths.contains(&"apps/frontend".to_string()));

            // In T: apps/admin and apps/frontend have build, but apps/backend does not
            let build_services = discover_services(&t_path, "build").unwrap();
            let build_paths: Vec<String> = build_services
                .into_iter()
                .map(|s| s.relative_path)
                .collect();
            assert!(build_paths.contains(&"apps/admin".to_string()));
            assert!(build_paths.contains(&"apps/frontend".to_string()));
            assert!(!build_paths.contains(&"apps/backend".to_string()));
        }
    }
}
