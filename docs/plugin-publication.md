# Omarchy plugin publication

The root `manifest.json` defines the `io.github.sonique6784.omacdiag` panel. Its diagnostic UI runs inside the existing Omarchy shell and starts the separately installed `/usr/bin/omacdiag` Rust engine. The CLI archive is a manual prerequisite; the plugin never installs packages or starts a second Quickshell process.

## Prepare and verify the release

1. Finish the plugin source, commit it, and run `./verify-plugin.sh --preflight`. On a live Omarchy desktop run `./precheck.sh`. It checks Qt 6.8+, the system paths, and Quickshell 0.3.1 before building a CLI archive and guiding the in-shell UI smoke test.
2. Copy the verified CLI archive and checksum out of `/tmp`. Install **that exact archive**, install and enable the plugin, add the menu row, and complete a Quick + Guided check. The verifier checks that the installed engine matches the archive and that the report is complete, current, and mode 0600.
3. Exercise summon, hide, menu launch, close, disable, re-enable, menu row removal, and plugin removal. Review the CLI archive's third-party notices. Keep private hardware identifiers and reports out of Git.
4. Tag the tested source commit `v<version>`. Publish `omacdiag-<version>-linux-x86_64-cli.tar.gz` and its `.sha256` file in a final GitHub release. Add `docs/releases/v<version>.md` from the acceptance template in a **later docs-only commit**, recording the tested source commit and archive SHA. Do not rebuild the archive after recording its SHA: the package copies `docs/` and would otherwise contain its own acceptance record.

## Submit the listing

Make the GitHub repository public, push the tested tag and the acceptance commit to the default branch, and run `gh auth login -h github.com` if needed. Run `./publish.sh` to check the release and review the proposed issue; run `./publish.sh --submit` to create the marketplace submission. The script refuses changes outside `docs/releases/` after the tested commit.

Marketplace validation and security checks run on the issue. Fix findings on the existing issue and retest a new source commit if code changes. Maintainers must approve the validated commit before it appears on the site. The separate CLI installation and menu row should be disclosed as manual setup.

References: [development guide](https://plugins.omarchy.org/develop.html), [publishing guide](https://plugins.omarchy.org/publish.html), [submission guide](https://github.com/omacom/omarchy-plugin-marketplace/blob/main/SUBMISSION.md).
