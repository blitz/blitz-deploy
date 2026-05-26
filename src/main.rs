mod hercules;

use std::str::FromStr;

use anyhow::{Context, Result};
use hercules::{Account, Derivation, Evaluation, JobList};
use itertools::Itertools;
use urlencoding::encode as url_encode;

/// The location of a project. Currently assumes GitHub.
#[derive(clap::Parser, Clone, Debug)]
struct ProjectLocation {
    owner: String,
    repo: String,
    branch: String,
}

impl FromStr for ProjectLocation {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(location_str) = s.strip_prefix("github:") {
            let components: Vec<&str> = location_str.split('/').collect();
            if components.len() != 3 {
                anyhow::bail!("Invalid GitHub URL: {}", s)
            }
            Ok(Self {
                owner: components[0].to_owned(),
                repo: components[1].to_owned(),
                branch: components[2].to_owned(),
            })
        } else {
            anyhow::bail!("Unsupported project location: {}", s)
        }
    }
}

#[derive(Debug, Clone, Default, clap::Subcommand)]
enum Action {
    /// Only print the discovered NixOS configuration toplevel.
    #[default]
    Discover,

    /// Realise the NixOS configuration, but don't switch to it.
    Realise,

    /// Switch to the realised NixOS configuration.
    Switch,

    /// Switch to the realised NixOS configuration on the next boot.
    Boot,
}

#[derive(Debug, clap::Parser)]
struct Opts {
    /// The location of the project to deploy. This has to be a string of the form `github:owner/repo/branch`.
    #[clap(long)]
    project: ProjectLocation,

    /// The name of the NixOS configuration to deploy. If not specified, defaults to
    /// the hostname.
    #[clap(long)]
    nixos_config: Option<String>,

    /// Which command to perform.
    #[clap(subcommand)]
    action: Action,
}

fn main() -> Result<()> {
    let opts = <Opts as clap::Parser>::parse();
    let forge = "github";
    let owner = &opts.project.owner;
    let repo = &opts.project.repo;
    let branch = &opts.project.branch;

    let nixos_config_name: String = if let Some(config_name) = &opts.nixos_config {
        config_name.to_owned()
    } else {
        hostname::get()
            .context("Failed to query hostname")?
            .to_str()
            .context("Hostname is not valid UTF-8")?
            .to_owned()
    };
    let accounts: Vec<Account> = ureq::get(format!(
        "https://hercules-ci.com/api/v1/accounts?site={}&name={}",
        url_encode(forge),
        url_encode(owner)
    ))
    .call()
    .context("Failed to fetch account")?
    .body_mut()
    .read_json()
    .context("Failed to parse account")?;
    let account = &accounts[0];

    let jobs: JobList = ureq::get(format!(
        "https://hercules-ci.com/api/v1/site/github/account/{}/project/{}/jobs?limit=25",
        url_encode(owner),
        url_encode(repo)
    ))
    .call()
    .context("Failed to fetch jobs")?
    .body_mut()
    .read_json()
    .context("Failed to parse jobs")?;

    if let Some(latest_success) = jobs
        .into_iter()
        .sorted_by_key(|job| -job.index)
        .find(|job| {
            job.derivation_status == "Success"
                && job.job_type == "OnPush"
                && job.source.branch() == Some(branch)
        })
    {
        let evaluation: Evaluation = ureq::get(&format!(
            "https://hercules-ci.com/api/v1/jobs/{}/evaluation",
            latest_success.id
        ))
        .call()
        .context("Failed to fetch evaluation")?
        .body_mut()
        .read_json()
        .context("Failed to parse evaluation")?;

        if let Some(attr) = evaluation
            .attributes
            .iter()
            .find(|attr| attr.nixos_configuration_name().as_deref() == Some(&nixos_config_name))
        {
            let derivation_path = attr
                .derivation_path()
                .context("Attribute has no derivation path")?;

            let derivation: Derivation = ureq::get(&format!(
                "https://hercules-ci.com/api/v1/accounts/{}/derivations/{}",
                account.id,
                url_encode(&derivation_path)
            ))
            .call()
            .context("Failed to fetch derivation")?
            .body_mut()
            .read_json()
            .context("Failed to parse derivation")?;

            if let Some(output) = derivation
                .outputs()
                .find(|output| output.output_name == "out")
            {
                if let Action::Discover = &opts.action {
                    println!("{}", output.output_path);
                    return Ok(());
                }

                let current_toplevel = std::fs::read_link("/run/current-system")
                    .context("Failed to find toplevel of current system")?;

                if output.output_path == current_toplevel {
                    println!("Current system is recent. Skipping deployment.");
                    return Ok(());
                }

                println!(
                    "Realising toplevel for {}: {}",
                    nixos_config_name, output.output_path
                );

                std::process::Command::new("nix-store")
                    .arg("--realise")
                    .arg(&output.output_path)
                    .status()
                    .context("Failed to realise output")?;

                let nixos_rebuild_action = match &opts.action {
                    Action::Discover => unreachable!(),
                    Action::Realise => None,
                    Action::Switch => Some("switch"),
                    Action::Boot => Some("boot"),
                };

                if let Some(action) = nixos_rebuild_action {
                    std::process::Command::new("nixos-rebuild")
                        .arg("--sudo")
                        .arg("--ask-sudo-password")
                        .arg("--no-reexec")
                        .arg("--store-path")
                        .arg(&output.output_path)
                        .arg(action)
                        .status()
                        .context("Failed to run nixos-rebuild")?;
                }
            }
        }
    }

    Ok(())
}
