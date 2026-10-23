use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use crypto_sso_issuer::{Issuer, wire};
use rand::rngs::OsRng;

#[derive(Parser)]
#[command(
    version,
    about = "Generate an issuer private key and print its public JSON"
)]
struct Args {
    /// Organization name (1-128 UTF-8 bytes, no surrounding whitespace or control characters)
    #[arg(short = 'n', long)]
    organization: String,

    /// Save the unencrypted SDK private backup to a new file; never overwrite an existing file
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,
}

fn save_secret(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    // tempfile creates files with mode 0600 on Unix. Write beside the destination
    // so publication stays on the same filesystem and exposes only complete data.
    let mut file = tempfile::Builder::new()
        .prefix(".issuer-secret-")
        .tempfile_in(parent)
        .with_context(|| format!("cannot create a private key file in {}", parent.display()))?;
    file.write_all(bytes)
        .context("cannot write the issuer private key")?;
    file.as_file()
        .sync_all()
        .context("cannot flush the issuer private key")?;
    file.persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "cannot save private key to {} (destination must not exist)",
                path.display()
            )
        })?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let issuer = Issuer::generate(&args.organization, &mut OsRng)
        .context("cannot generate issuer: check the organization name")?;
    let secret = issuer
        .export_secret()
        .context("cannot export the issuer private key")?;
    let public = wire::to_json(issuer.public()).context("cannot encode the issuer public key")?;

    save_secret(&args.output, secret.expose_secret())?;
    eprintln!("Issuer private key saved to {}", args.output.display());
    eprintln!("Issuer ID: {}", issuer.issuer_id().as_str());

    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&public)
        .context("cannot print the issuer public key")?;
    writeln!(stdout).context("cannot print the issuer public key")?;
    Ok(())
}
