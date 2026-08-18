//! `completions` command: outputs PowerShell tab-completion script.
//!
//! The script registers a native PowerShell argument completer using the AST
//! (Abstract Syntax Tree) for proper parsing. This uses the same minimal approach
//! as Taurine, which works reliably in Windows PowerShell 5.1.

use crate::cli::help::completions as completions_help;
use crate::cli::parser::Command;

/// The PowerShell completion script using native AST-based completion.
/// Minimal version matching Taurine's working approach.
const COMPLETION_SCRIPT: &str = r#"# rmod PowerShell tab completion (native AST-based)
# Install by adding to your PowerShell profile:
#   rmod completions >> $PROFILE
# Or source directly:
#   rmod completions | Out-String | Invoke-Expression

$scriptBlock = {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    if (-not $commandElements -or $commandElements.Count -eq 0) { return }

    # Build the command chain from AST elements (skip flags, stop at word being completed)
    # This matches Taurine's exact approach: start at index 1 (skip command name),
    # collect complete words until we hit a flag or the word being completed
    $cmdChain = @('rmod')
    for ($i = 1; $i -lt $commandElements.Count; $i++) {
        $element = $commandElements[$i]
        if ($element -isnot [System.Management.Automation.Language.StringConstantExpressionAst] -or
            $element.StringConstantType -ne [System.Management.Automation.Language.StringConstantType]::BareWord -or
            $element.Value.StartsWith('-')) {
            break
        }
        # Stop if this element is the word being completed (partial or full match)
        if ($wordToComplete -and $element.Value.StartsWith($wordToComplete)) {
            break
        }
        $cmdChain += $element.Value
    }

    $commandKey = $cmdChain -join ';'

    $completions = @(switch ($commandKey) {
        'rmod' {
            [System.Management.Automation.CompletionResult]::new('list', 'list', [System.Management.Automation.CompletionResultType]::ParameterValue, 'List displays')
            [System.Management.Automation.CompletionResult]::new('set', 'set', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Set resolution')
            [System.Management.Automation.CompletionResult]::new('layout', 'layout', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Show layout')
            [System.Management.Automation.CompletionResult]::new('monitor', 'monitor', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Monitor control')
            [System.Management.Automation.CompletionResult]::new('temp', 'temp', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Temperature control')
            [System.Management.Automation.CompletionResult]::new('view', 'view', [System.Management.Automation.CompletionResultType]::ParameterValue, 'View modes')
            [System.Management.Automation.CompletionResult]::new('completions', 'completions', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Completions script')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;view' {
            [System.Management.Automation.CompletionResult]::new('mirror', 'mirror', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Mirror mode')
            [System.Management.Automation.CompletionResult]::new('extend', 'extend', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Extend mode')
            [System.Management.Automation.CompletionResult]::new('project', 'project', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Project mode')
            [System.Management.Automation.CompletionResult]::new('single', 'single', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Single mode')
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID')
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip confirmation')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip confirmation')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            # Subcommands
            if ($wordToComplete -like 'm*' -or $wordToComplete -like 'e*' -or $wordToComplete -like 'p*' -or $wordToComplete -like 's*') {
                'mirror', 'extend', 'project', 'single' | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
                    [System.Management.Automation.CompletionResult]::new($_, $_, [System.Management.Automation.CompletionResultType]::ParameterValue, 'View mode')
                }
            }
            break
        }
        default { break }
    })

    # Filter completions
    $filtered = $completions | Where-Object { $_.CompletionText -like "$wordToComplete*" }
    if ($filtered.Count -eq 0) { return } # Suppress fallback to file completion
    $filtered
}

Register-ArgumentCompleter -Native -CommandName 'rmod' -ScriptBlock $scriptBlock
"#;

/// Runs the `completions` command and outputs the PowerShell script.
pub(super) fn run_completions(help: bool) -> i32 {
    if help {
        println!("{}", completions_help());
    } else {
        println!("{}", COMPLETION_SCRIPT);
    }
    0
}

/// Parses the `completions` command.
pub(crate) fn parse_completions(_cmd: &str, args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut i = 1;
    let mut help = false;

    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--help" => {
                help = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for completions. use --help",
                    other
                ));
            }
        }
    }

    Ok(Command::Completions { help })
}
