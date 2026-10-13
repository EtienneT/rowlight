# Security

Tablelamp is an early alpha. Security fixes target the latest released alpha;
older versions should be updated. Supported platforms are listed in
[platforms.md](docs/platforms.md).

Report vulnerabilities through [GitHub's private vulnerability reporting](https://github.com/EtienneT/tablelamp/security/advisories/new).
Include the Tablelamp version, operating system, impact and reproduction steps
using synthetic data where possible. Never include real cloud credentials,
signed URLs or private datasets in a public issue.

The private report control must be enabled when the repository becomes public.
If it is unavailable, wait for a private reporting channel before sharing a
sensitive reproducer. Ordinary crashes without a security impact can use the
bug-report template.

Local sources are opened read-only. Remote access uses your provider's credentials
and permissions; use read-only credentials. SQL runs in an isolated worker with
memory and temporary-storage budgets. These guards do not make the app a sandbox
for arbitrary hostile input.
