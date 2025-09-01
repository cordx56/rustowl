use clap_complete_nushell::Nushell;

use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;

use clap::ValueEnum;

use clap_complete::Generator;
use clap_complete::shells;

/// Extended shell support including Nushell
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
#[non_exhaustive]
#[value(rename_all = "lower")]
pub enum Shell {
    /// Bourne Again `SHell` (bash)
    Bash,
    /// Elvish shell  
    Elvish,
    /// Friendly Interactive `SHell` (fish)
    Fish,
    /// `PowerShell`
    PowerShell,
    /// Z `SHell` (zsh)
    Zsh,
    /// Nushell
    Nushell,
}

impl Display for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Shell::Bash => write!(f, "bash"),
            Shell::Elvish => write!(f, "elvish"),
            Shell::Fish => write!(f, "fish"),
            Shell::PowerShell => write!(f, "powershell"),
            Shell::Zsh => write!(f, "zsh"),
            Shell::Nushell => write!(f, "nushell"),
        }
    }
}

impl FromStr for Shell {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(Shell::Bash),
            "elvish" => Ok(Shell::Elvish),
            "fish" => Ok(Shell::Fish),
            "powershell" => Ok(Shell::PowerShell),
            "zsh" => Ok(Shell::Zsh),
            "nushell" => Ok(Shell::Nushell),
            _ => Err(format!("invalid variant: {s}")),
        }
    }
}

impl Generator for Shell {
    fn file_name(&self, name: &str) -> String {
        match self {
            Shell::Bash => shells::Bash.file_name(name),
            Shell::Elvish => shells::Elvish.file_name(name),
            Shell::Fish => shells::Fish.file_name(name),
            Shell::PowerShell => shells::PowerShell.file_name(name),
            Shell::Zsh => shells::Zsh.file_name(name),
            Shell::Nushell => Nushell.file_name(name),
        }
    }

    fn generate(&self, cmd: &clap::Command, buf: &mut dyn std::io::Write) {
        match self {
            Shell::Bash => shells::Bash.generate(cmd, buf),
            Shell::Elvish => shells::Elvish.generate(cmd, buf),
            Shell::Fish => shells::Fish.generate(cmd, buf),
            Shell::PowerShell => shells::PowerShell.generate(cmd, buf),
            Shell::Zsh => shells::Zsh.generate(cmd, buf),
            Shell::Nushell => Nushell.generate(cmd, buf),
        }
    }
}

impl Shell {
    /// Parse a shell from a path to the executable for the shell
    pub fn from_shell_path<P: AsRef<Path>>(path: P) -> Option<Shell> {
        let path = path.as_ref();
        let name = path.file_stem()?.to_str()?;
        
        match name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            "elvish" => Some(Shell::Elvish),
            "powershell" | "powershell_ise" => Some(Shell::PowerShell),
            "nu" | "nushell" => Some(Shell::Nushell),
            _ => None,
        }
    }

    /// Determine the user's current shell from the environment
    pub fn from_env() -> Option<Shell> {
        if let Some(env_shell) = std::env::var_os("SHELL") {
            Shell::from_shell_path(env_shell)
        } else if cfg!(windows) {
            Some(Shell::PowerShell)
        } else {
            None
        }
    }
    
    /// Convert to the standard shell type if possible, for compatibility
    pub fn to_standard_shell(&self) -> Option<shells::Shell> {
        match self {
            Shell::Bash => Some(shells::Shell::Bash),
            Shell::Elvish => Some(shells::Shell::Elvish),
            Shell::Fish => Some(shells::Shell::Fish),
            Shell::PowerShell => Some(shells::Shell::PowerShell),
            Shell::Zsh => Some(shells::Shell::Zsh),
            Shell::Nushell => None, // Not supported by standard shells
        }
    }
}
