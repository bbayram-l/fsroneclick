# Security Policy

FSRoneclick downloads third-party DLLs and writes them into game folders, so we take security reports seriously.

## Supported versions

Only the [latest release](https://github.com/bbayram-l/fsroneclick/releases/latest) receives fixes.

## Reporting a vulnerability

Please **do not** open a public issue. Instead, use GitHub's
[private vulnerability reporting](https://docs.github.com/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability):
open the repository's **Security** tab and choose **Report a vulnerability**.

Please include the affected version, steps to reproduce, and the impact you observed. We aim to acknowledge
reports within a week and will keep you updated until a fix is released.

## Scope

In scope:

- Writing, overwriting, or deleting files outside the selected game folder or the FSRoneclick cache
- Downloading or executing content from an unexpected source, or accepting tampered remote data
  (compatibility wiki, RenoDX catalog, GitHub release metadata)
- Unexpected changes to the Windows registry or system settings

Out of scope:

- Vulnerabilities in OptiScaler, ReShade, RenoDX, REFramework, or other upstream projects — please report those upstream
- Anti-cheat detections or account bans; FSRoneclick warns about these but cannot prevent them
