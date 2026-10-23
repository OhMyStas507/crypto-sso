use std::{collections::HashSet, fs, path::Path, process::Command};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    repositories: Vec<Repository>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Repository {
    name: String,
    url: String,
    branch: Option<String>,
}

fn validate(config: &Config) -> Result<()> {
    let mut names = HashSet::new();
    for repo in &config.repositories {
        // A portable single directory component; also excludes traversal and Git options.
        ensure!(
            !repo.name.is_empty()
                && repo.name != "."
                && repo.name != ".."
                && !repo.name.starts_with('-')
                && repo
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
            "invalid repository name: {:?}",
            repo.name
        );
        ensure!(
            names.insert(repo.name.to_ascii_lowercase()),
            "duplicate repository name: {}",
            repo.name
        );
        ensure!(!repo.url.trim().is_empty(), "empty URL for {}", repo.name);
        if let Some(branch) = &repo.branch {
            ensure!(!branch.trim().is_empty(), "empty branch for {}", repo.name);
        }
    }
    Ok(())
}

fn fetch(root: &Path) -> Result<()> {
    let config_path = root.join("references.toml");
    let source = fs::read_to_string(&config_path)
        .with_context(|| format!("cannot read {}", config_path.display()))?;
    let config: Config = toml::from_str(&source)
        .with_context(|| format!("cannot parse {}", config_path.display()))?;
    validate(&config)?;

    if config.repositories.is_empty() {
        println!("No repositories configured in {}", config_path.display());
        return Ok(());
    }

    let destination = root.join("docs/references");
    fs::create_dir_all(&destination)
        .with_context(|| format!("cannot create {}", destination.display()))?;
    let mut failures = Vec::new();
    for repo in config.repositories {
        let target = destination.join(&repo.name);
        if target.symlink_metadata().is_ok() {
            println!("Skipping {}: destination already exists", target.display());
            continue;
        }

        println!("Cloning {} into {}", repo.url, target.display());
        let mut command = Command::new("git");
        command.arg("clone");
        if let Some(branch) = &repo.branch {
            command.arg("--branch").arg(branch);
        }
        command.arg("--").arg(&repo.url).arg(&target);
        match command.status() {
            Ok(status) if status.success() => {}
            Ok(status) => failures.push(format!("{}: git clone exited with {status}", repo.name)),
            Err(error) => failures.push(format!("{}: cannot run git: {error}", repo.name)),
        }
    }

    if !failures.is_empty() {
        bail!("Failed to clone repositories:\n{}", failures.join("\n"));
    }
    Ok(())
}

fn main() -> Result<()> {
    // Resolve relative to the crate, so invocation does not depend on the working directory.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("cannot locate workspace root")?;
    fetch(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_local_repository_and_preserves_existing_checkout() -> Result<()> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("fetch-references-{unique}"));
        fs::create_dir_all(&root)?;
        let result = (|| -> Result<()> {
            let upstream = root.join("upstream");
            let git = |args: &[&str]| -> Result<()> {
                let status = Command::new("git").args(args).status()?;
                ensure!(status.success(), "git test fixture failed");
                Ok(())
            };
            let upstream_path = upstream.to_str().context("invalid temporary path")?;
            git(&["init", "--initial-branch=main", upstream_path])?;
            fs::write(upstream.join("README.md"), "reference content")?;
            git(&["-C", upstream_path, "add", "README.md"])?;
            git(&[
                "-C",
                upstream_path,
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "initial",
            ])?;
            let url = upstream_path.replace('\\', "/");
            fs::write(
                root.join("references.toml"),
                format!("[[repositories]]\nname = 'local'\nurl = '{url}'\nbranch = 'main'\n"),
            )?;
            fetch(&root)?;
            let checkout = root.join("docs/references/local/README.md");
            assert_eq!(fs::read_to_string(&checkout)?, "reference content");
            fs::write(&checkout, "local changes")?;
            fetch(&root)?;
            assert_eq!(fs::read_to_string(&checkout)?, "local changes");
            Ok(())
        })();
        fs::remove_dir_all(&root)?;
        result
    }

    #[test]
    fn rejects_unsafe_and_duplicate_destinations() {
        for names in [
            vec!["../outside"],
            vec!["a/b"],
            vec![".."],
            vec!["Repo", "repo"],
        ] {
            let config = Config {
                repositories: names
                    .into_iter()
                    .map(|name| Repository {
                        name: name.into(),
                        url: "https://example.com/repo.git".into(),
                        branch: None,
                    })
                    .collect(),
            };
            assert!(validate(&config).is_err());
        }
    }

    #[test]
    fn parses_repository_list_with_optional_branch() {
        let config: Config = toml::from_str(
            r#"
            [[repositories]]
            name = "first"
            url = "https://example.com/first.git"
            branch = "main"
            [[repositories]]
            name = "second"
            url = "git@example.com:second.git"
        "#,
        )
        .unwrap();
        validate(&config).unwrap();
        assert_eq!(config.repositories[0].branch.as_deref(), Some("main"));
        assert!(config.repositories[1].branch.is_none());
    }
}
