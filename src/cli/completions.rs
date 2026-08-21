use std::io;

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use super::commands::Cli;

/// Generate shell completion scripts for the given shell and print to stdout.
///
/// Users can redirect the output to install completions:
/// ```sh
/// smriti completions bash > ~/.bash_completion.d/smriti
/// smriti completions zsh  > ~/.zsh/completions/_smriti
/// smriti completions fish > ~/.config/fish/completions/smriti.fish
/// ```
pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "smriti", &mut io::stdout());
}
