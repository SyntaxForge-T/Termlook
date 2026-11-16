# Security policy

## Supported versions

TermLook is pre-1.0. Security fixes are released only for the latest published
version, so please update before reporting.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Report it privately
through GitHub:

[Report a vulnerability](https://github.com/SyntaxForge-T/Termlook/security/advisories/new)

Include the TermLook version (`termlook --version`), your Linux distribution and
desktop, and the steps needed to reproduce the problem. Remove tokens and private
terminal output from anything you attach.

TermLook is maintained by volunteers, so reports are handled on a best-effort
basis. You will get a reply on the advisory thread.

## Scope

TermLook runs the shell and commands you configure with your own user privileges.
That is expected behavior, not a vulnerability. Problems that let one party run
commands, read files or escape a confirmation they did not intend are in scope,
for example through crafted settings, layout or project files, or through terminal
escape sequences.
