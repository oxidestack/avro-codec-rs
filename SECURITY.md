# Security policy

Please report a vulnerability privately, so that it can be fixed before it is public: on the repository's **Security**
tab choose **Report a vulnerability**, or use
[the form](https://github.com/oxidestack/avro-codec-rs/security/advisories/new). Only the maintainers see it. Do not open
a public issue, discussion or pull request for one.

Say which version or commit, the schema and payload that trigger it, and what an attacker gets (memory or CPU
exhausted, a panic, a read outside the input). The fix is released with a GitHub security advisory and credits you
unless you prefer not to be named. Fixes go into the latest release.

## Supported versions

Only the latest release.

In scope: the schema parser, the JSON and binary decoders and the JSON encoder this crate ports. Untrusted schemas and
payloads are the normal input of such a codec, so a hang, an unbounded allocation or a panic on one is a bug worth
reporting. A difference from Apache Avro's output is a bug, to report as one.
