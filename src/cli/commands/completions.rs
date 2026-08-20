//! `completions` command: outputs PowerShell tab-completion script.
//!
//! The script registers a native PowerShell argument completer using the AST
//! (Abstract Syntax Tree) for proper parsing. This uses the same minimal approach
//! as Taurine, which works reliably in Windows PowerShell 5.1.

use crate::cli::help::completions as completions_help;
use crate::cli::parser::Command;

/// The PowerShell completion script using native AST-based completion.
/// Minimal version matching Taurine's working approach.
/// Completions for the flat CLI: 15 root verbs, each with its own flags.
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
            [System.Management.Automation.CompletionResult]::new('list', 'list', [System.Management.Automation.CompletionResultType]::ParameterValue, 'List displays and their current settings')
            [System.Management.Automation.CompletionResult]::new('set', 'set', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Apply resolution, refresh rate, and orientation')
            [System.Management.Automation.CompletionResult]::new('layout', 'layout', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Show the monitor arrangement or move monitors')
            [System.Management.Automation.CompletionResult]::new('brightness', 'brightness', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Set the display backlight level (0-100, or min, max, boost)')
            [System.Management.Automation.CompletionResult]::new('contrast', 'contrast', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Set the display contrast (0-130, 100 = neutral)')
            [System.Management.Automation.CompletionResult]::new('temp', 'temp', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Set or show the display color temperature')
            [System.Management.Automation.CompletionResult]::new('attach', 'attach', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Re-attach a monitor to the desktop')
            [System.Management.Automation.CompletionResult]::new('detach', 'detach', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Detach a monitor from the desktop')
            [System.Management.Automation.CompletionResult]::new('sleep', 'sleep', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Put every monitor to sleep')
            [System.Management.Automation.CompletionResult]::new('wake', 'wake', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Wake every monitor')
            [System.Management.Automation.CompletionResult]::new('mirror', 'mirror', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Clone all displays: same position (0,0), same resolution (lowest common)')
            [System.Management.Automation.CompletionResult]::new('extend', 'extend', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Restore extended desktop: side-by-side, auto-arranged left-to-right')
            [System.Management.Automation.CompletionResult]::new('project', 'project', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Second screen only: disable primary, keep external monitor(s) enabled')
            [System.Management.Automation.CompletionResult]::new('single', 'single', [System.Management.Automation.CompletionResultType]::ParameterValue, 'PC screen only: enable only one monitor, disable all others')
            [System.Management.Automation.CompletionResult]::new('completions', 'completions', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Output PowerShell tab-completion script')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('-V', '-V', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;list' {
            [System.Management.Automation.CompletionResult]::new('--short', '--short', [System.Management.Automation.CompletionResultType]::ParameterName, 'Compact one-line output')
            [System.Management.Automation.CompletionResult]::new('--all', '--all', [System.Management.Automation.CompletionResultType]::ParameterName, 'Show all monitors including detached')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;set' {
            [System.Management.Automation.CompletionResult]::new('-w', '-w', [System.Management.Automation.CompletionResultType]::ParameterName, 'Resolution width (requires --height)')
            [System.Management.Automation.CompletionResult]::new('--width', '--width', [System.Management.Automation.CompletionResultType]::ParameterName, 'Resolution width (requires --height)')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Resolution height (requires --width)')
            [System.Management.Automation.CompletionResult]::new('--height', '--height', [System.Management.Automation.CompletionResultType]::ParameterName, 'Resolution height (requires --width)')
            [System.Management.Automation.CompletionResult]::new('-r', '-r', [System.Management.Automation.CompletionResultType]::ParameterName, 'Refresh rate in Hz, or max')
            [System.Management.Automation.CompletionResult]::new('--refresh', '--refresh', [System.Management.Automation.CompletionResultType]::ParameterName, 'Refresh rate in Hz, or max')
            [System.Management.Automation.CompletionResult]::new('-p', '-p', [System.Management.Automation.CompletionResultType]::ParameterName, 'Resolution preset (720, 1080, 1440, 4k, 8k)')
            [System.Management.Automation.CompletionResult]::new('--profile', '--profile', [System.Management.Automation.CompletionResultType]::ParameterName, 'Resolution preset (720, 1080, 1440, 4k, 8k)')
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('-o', '-o', [System.Management.Automation.CompletionResultType]::ParameterName, 'Rotation angle (see Orientations)')
            [System.Management.Automation.CompletionResult]::new('--orientation', '--orientation', [System.Management.Automation.CompletionResultType]::ParameterName, 'Rotation angle (see Orientations)')
            [System.Management.Automation.CompletionResult]::new('--max', '--max', [System.Management.Automation.CompletionResultType]::ParameterName, "Use the display's highest supported mode")
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;layout' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID or primary to move or promote')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID or primary to move or promote')
            [System.Management.Automation.CompletionResult]::new('--left-of', '--left-of', [System.Management.Automation.CompletionResultType]::ParameterName, 'Place the monitor left of the reference')
            [System.Management.Automation.CompletionResult]::new('--right-of', '--right-of', [System.Management.Automation.CompletionResultType]::ParameterName, 'Place the monitor right of the reference')
            [System.Management.Automation.CompletionResult]::new('--above', '--above', [System.Management.Automation.CompletionResultType]::ParameterName, 'Place the monitor above the reference')
            [System.Management.Automation.CompletionResult]::new('--below', '--below', [System.Management.Automation.CompletionResultType]::ParameterName, 'Place the monitor below the reference')
            [System.Management.Automation.CompletionResult]::new('--primary', '--primary', [System.Management.Automation.CompletionResultType]::ParameterName, 'Make the monitor the main display')
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;brightness' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor number or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor number or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--via', '--via', [System.Management.Automation.CompletionResultType]::ParameterName, 'Backend: ddc, slider, or gamma (default: auto)')
            [System.Management.Automation.CompletionResult]::new('min', 'min', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Composite mode: barely lit')
            [System.Management.Automation.CompletionResult]::new('max', 'max', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Composite mode: full brightness (restore)')
            [System.Management.Automation.CompletionResult]::new('boost', 'boost', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Composite mode: overdriven gamma')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;contrast' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor number or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor number or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--via', '--via', [System.Management.Automation.CompletionResultType]::ParameterName, 'Backend: ddc or gamma (default: auto)')
            [System.Management.Automation.CompletionResult]::new('reset', 'reset', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Reset contrast to defaults')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;temp' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (default: primary)')
            [System.Management.Automation.CompletionResult]::new('candle', 'candle', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Preset: 1900K')
            [System.Management.Automation.CompletionResult]::new('warm', 'warm', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Preset: 2700K')
            [System.Management.Automation.CompletionResult]::new('neutral', 'neutral', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Preset: 3400K')
            [System.Management.Automation.CompletionResult]::new('cool', 'cool', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Preset: 4500K')
            [System.Management.Automation.CompletionResult]::new('daylight', 'daylight', [System.Management.Automation.CompletionResultType]::ParameterValue, 'Preset: 6500K')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;attach' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (required)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (required)')
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;detach' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (required)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID, primary, or all (required)')
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;sleep' {
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;wake' {
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;mirror' {
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;extend' {
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;project' {
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;single' {
            [System.Management.Automation.CompletionResult]::new('-m', '-m', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID or number (default: primary)')
            [System.Management.Automation.CompletionResult]::new('--monitor', '--monitor', [System.Management.Automation.CompletionResultType]::ParameterName, 'Monitor ID or number (default: primary)')
            [System.Management.Automation.CompletionResult]::new('-y', '-y', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('--yes', '--yes', [System.Management.Automation.CompletionResultType]::ParameterName, 'Skip the confirmation prompt')
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'rmod;completions' {
            [System.Management.Automation.CompletionResult]::new('-h', '-h', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--help', '--help', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print help')
            [System.Management.Automation.CompletionResult]::new('--version', '--version', [System.Management.Automation.CompletionResultType]::ParameterName, 'Print version')
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
            "-h" | "--help" => {
                help = true;
                i += 1;
            }
            "--version" => return Ok(Command::Version),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn completions_help_flags() {
        assert_eq!(
            parse(&["completions", "-h"]),
            Ok(Command::Completions { help: true })
        );
        assert_eq!(
            parse(&["completions", "--help"]),
            Ok(Command::Completions { help: true })
        );
    }

    #[test]
    fn completions_version_flag() {
        assert_eq!(parse(&["completions", "--version"]), Ok(Command::Version));
    }
}
