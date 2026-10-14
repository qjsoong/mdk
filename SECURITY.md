# Security policy

## Supported versions

mdk is pre-1.0; security fixes are applied to the latest release on the `main`
branch. There are no older supported branches yet.

## Reporting a vulnerability

Please report security issues privately using GitHub's [private vulnerability
reporting](https://github.com/qjsoong/mdk/security/advisories/new) rather than
opening a public issue.

Include the affected version or commit, a description of the impact, and a
minimal reproduction if possible. You can expect an initial response within a
few days. Once a fix is available, the issue will be disclosed with credit
unless you prefer to remain anonymous.

## Scope

mdk parses untrusted Markdown input. The most relevant areas are:

- parser panics or excessive resource use on crafted input;
- the HTML renderer passing through raw HTML (`--allow-html` is explicit and
  documented; without it raw HTML is escaped);
- terminal escape sequences in rendered output when the input contains control
  characters.

Reports about these areas are welcome, as are dependency vulnerabilities
reported through Dependabot.
