# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue:

1. Open the [Security tab](https://github.com/hashcott/browserdeck/security) of this repository.
2. Choose **Report a vulnerability** and describe the problem, how to reproduce it, and what an attacker could do.

The reply comes in the same private report. Please allow time for a fix before you publish the problem.

## Supported versions

Only the latest release gets security fixes.

## Scope

Browserdeck runs a small web server on your PC so your phone can press buttons. Problems we especially want to hear about:

- Pressing buttons, or reading the button layout, without the pairing code.
- Getting the pairing code from another device on the network, or from another program on the PC.
- In administrator mode: any way for a program running without administrator rights to gain them through Browserdeck.
- Anything the installer or uninstaller leaves behind that weakens the PC.

## Known limitations

These are known and documented, so they are not treated as vulnerabilities on their own:

- The phone connects over plain HTTP on the local network. Someone who can watch that traffic can read the pairing code. Use Browserdeck on networks you trust.
- In administrator mode, harmful software that is already running on the PC as the same user still has some ways in that Browserdeck cannot fully close (for example through WSL or a virtual machine on the same PC, a router that loops traffic back, or Windows versions without Redirection Guard). The app warns about this before the mode is turned on.
- Some games and anti-cheat software ignore simulated keys.
