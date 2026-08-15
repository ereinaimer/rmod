# Security Policy

We take security seriously from day one. If you discover a vulnerability in the latest release or the `main` branch, we highly encourage you to report it so we can evaluate and address it as soon as possible.

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them by creating a [Private Vulnerability Report](https://github.com/ereinaimer/rmod/security/advisories/new) in this repository.

Please include the following information in your report if possible:

* Type of vulnerability 
* A clear description of the vulnerability and its potential impact
* Step-by-step instructions to reproduce the issue
* Any proposed solutions or mitigations (optional)

We will try our best to respond to your report within 48-72 hours and keep you informed of our progress towards a fix. Once the issue is resolved, we will publish a security advisory and credit you for the discovery (if desired).

## Security Features

rmod is designed with safety and a minimal attack surface in mind:

- **Zero Dependencies**: Pure Rust with raw OS FFI bindings and no external crates. There is no supply-chain surface — nothing to audit beyond the code in this repository.
- **Guarded Display Changes**: Every display change prompts `keep changes? [N/y]`; anything but `y`/`yes` — or no answer within 5 seconds — automatically reverts to the previous mode. Nothing changes on your screen without explicit confirmation.
- **Dry-Run Validation**: Every mode change is tested against the display (`CDS_TEST`) before it is applied, so a change is never applied blind. Batch operations dry-run every monitor first; if any display rejects the mode, nothing changes.
- **Rollback on Failure**: Multi-monitor position swaps restore the original layout if a later apply fails, so a partial change is never left in place.
- **Primary Display Protection**: The primary display cannot be detached.
- **Strict Input Validation**: The CLI rejects malformed input fail-closed — monitor numbers must be 1-based, conflicting flags are errors, and color temperature is clamped to 1000–6500K in both the parser and the backend.
- **Isolated Testing**: Automated tests run against a fake backend (`RMOD_SYS_FAKE=1`) and never touch the real display, so test suites cannot alter your dev machine's monitor configuration. The suite spans ~500 unit tests plus CLI integration tests.
