# OmacDiag plugin and CLI release acceptance

Copy this file to `docs/releases/v<version>.md` after testing the exact CLI archive. Keep private hardware identifiers and report contents out of Git.

Status: pending
Artifact SHA-256: <64 lowercase hex digits>

- Release tag:
- Tested commit: <40 lowercase hex digits>
- Tested CLI archive filename:
- Omarchy version and kernel:
- Intel Mac model family and architecture:
- Quickshell and Qt runtime versions:
- Archive checksum and staged installation result:
- Installed engine matches archive result:
- In-shell GUI launch and basic interaction result: user confirmed the installed OmacDiag GUI works on 2026-10-12; record the launch method and any remaining UI checks here.
- Shell lifecycle result: record summon, Close, hide, and confirmation that Omarchy continues running without a second Quickshell process.
- Guided preflight, bounded session, and JSON report result: record the report state, result count, and file mode without committing the report itself.
- Plugin disable, re-enable, menu removal, and plugin removal result:
- Library dependency and third-party notice review:
- Tester and test date:

The confirmed GUI launch covers startup and basic interaction. Change `Status: pending` to `Status: passed` only after the guided report, shell lifecycle, installation/removal, and release artifact checks above have evidence. Commit the release record after the tested source commit; do not rebuild the tested archive.
