use std::{fs, process::Command};

use crypto_sso_issuer::{Issuer, IssuerPublic, wire};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_generate-issuer-key"))
}

#[test]
fn generated_backup_restores_with_the_printed_public_key() {
    let directory = tempfile::tempdir().unwrap();
    let mut public_keys = Vec::new();

    for name in ["first.json", "second.json"] {
        let output = command()
            .current_dir(directory.path())
            .args(["--organization", "示例组织", "--output", name])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let public: IssuerPublic = wire::from_json(&output.stdout).unwrap();
        let backup = fs::read(directory.path().join(name)).unwrap();
        let issuer = Issuer::import_secret(&backup, &public).unwrap();
        assert_eq!(issuer.public(), &public);
        assert_eq!(public.organization(), "示例组织");
        assert!(String::from_utf8_lossy(&output.stderr).contains(issuer.issuer_id().as_str()));
        public_keys.push(public);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::metadata(directory.path().join(name))
                .unwrap()
                .permissions();
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }
    }

    assert_ne!(
        public_keys[0].public_key_hex(),
        public_keys[1].public_key_hex()
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn existing_file_is_preserved_and_temporary_file_is_removed() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("issuer-secret.json");
    fs::write(&path, b"existing private key").unwrap();

    let output = command()
        .args(["--organization", "Example Org", "--output"])
        .arg(&path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"existing private key");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn invalid_organization_and_missing_arguments_create_no_files() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        vec![],
        vec!["--organization", "Example Org"],
        vec!["--output", "issuer-secret.json"],
        vec!["--organization", "", "--output", "issuer-secret.json"],
        vec![
            "--organization",
            " leading",
            "--output",
            "issuer-secret.json",
        ],
    ] {
        let output = command()
            .current_dir(directory.path())
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn missing_parent_directory_returns_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let output = command()
        .current_dir(directory.path())
        .args([
            "--organization",
            "Example Org",
            "--output",
            "missing/issuer-secret.json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}
