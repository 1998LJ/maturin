use anyhow::{Context, Result, bail};
use maturin::{DevelopOptions, Target, develop};
use std::env;
use std::path::PathBuf;
use tracing::{debug, instrument};

#[instrument(skip_all)]
pub fn develop_cmd(develop_options: DevelopOptions) -> Result<()> {
    let target = Target::from_target_triple(develop_options.cargo_options.target.as_ref())?;
    let venv_dir = detect_venv(&target)?;
    develop(develop_options, &venv_dir)?;
    Ok(())
}

fn normalize_windows_msys_path(path: PathBuf) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path;
    };
    let bytes = path_str.as_bytes();
    if bytes.len() >= 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b'/'
    {
        let drive = (bytes[1] as char).to_ascii_uppercase();
        PathBuf::from(format!("{drive}:{}", &path_str[2..]))
    } else {
        path
    }
}

fn venv_path_from_env(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from).map(|path| {
        if cfg!(windows) {
            normalize_windows_msys_path(path)
        } else {
            path
        }
    })
}

fn detect_venv(target: &Target) -> Result<PathBuf> {
    let virtual_env = venv_path_from_env("VIRTUAL_ENV");
    let conda_prefix = venv_path_from_env("CONDA_PREFIX");
    match (virtual_env, conda_prefix) {
        (Some(dir), None) => return Ok(dir),
        (None, Some(dir)) => return Ok(dir),
        (Some(venv), Some(conda)) if venv == conda => return Ok(venv),
        (Some(_), Some(_)) => {
            bail!("Both VIRTUAL_ENV and CONDA_PREFIX are set. Please unset one of them")
        }
        (None, None) => {
            // No env var, try finding .venv
        }
    };

    let current_dir = env::current_dir().context("Failed to detect current directory ಠ_ಠ")?;
    // .venv in the current or any parent directory
    for dir in current_dir.ancestors() {
        let dot_venv = dir.join(".venv");
        if dot_venv.is_dir() {
            if !dot_venv.join("pyvenv.cfg").is_file() {
                bail!(
                    "Expected {} to be a virtual environment, but pyvenv.cfg is missing",
                    dot_venv.display()
                );
            }
            let python = target.get_venv_python(&dot_venv);
            if !python.is_file() {
                bail!(
                    "Your virtualenv at {} is broken. It contains a pyvenv.cfg but no python at {}",
                    dot_venv.display(),
                    python.display()
                );
            }
            debug!("Found a virtualenv named .venv at {}", dot_venv.display());
            return Ok(dot_venv);
        }
    }

    bail!(
        "Couldn't find a virtualenv or conda environment, but you need one to use this command. \
        For maturin to find your virtualenv you need to either set VIRTUAL_ENV (through activate), \
        set CONDA_PREFIX (through conda activate) or have a virtualenv called .venv in the current \
        or any parent folder. \
        See https://virtualenv.pypa.io/en/latest/index.html on how to use virtualenv or \
        use `maturin build` and `pip install <path/to/wheel>` instead."
    )
}

#[cfg(test)]
mod tests {
    use super::normalize_windows_msys_path;
    use std::path::PathBuf;

    #[test]
    fn normalizes_msys_drive_paths() {
        assert_eq!(
            normalize_windows_msys_path(PathBuf::from("/c/Users/lifr0m/project/.venv")),
            PathBuf::from("C:/Users/lifr0m/project/.venv")
        );
        assert_eq!(
            normalize_windows_msys_path(PathBuf::from("/D/work/project/.venv")),
            PathBuf::from("D:/work/project/.venv")
        );
    }

    #[test]
    fn leaves_non_msys_paths_unchanged() {
        for path in [
            "/home/user/project/.venv",
            "C:/Users/user/project/.venv",
            r"C:\Users\user\project\.venv",
            "//server/share/.venv",
        ] {
            let path = PathBuf::from(path);
            assert_eq!(normalize_windows_msys_path(path.clone()), path);
        }
    }
}
