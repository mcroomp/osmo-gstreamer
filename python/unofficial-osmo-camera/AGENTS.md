# Python testing wrapper

Read the [root instructions](../../AGENTS.md) first. This file applies to this
project and its descendants.

## Responsibility

- Keep PyO3 conversion/dispatch glue thin; call the native Rust library
  in-process. Do not implement protocol decoding, transport, pairing,
  keepalives, ownership or lifecycle in Python.
- Preserve native errors, cancellation and unsupported states. Keep Python APIs
  typed with matching stubs and `py.typed`.
- Tests are offline by default. Do not add scripts that automatically discover,
  connect to or configure a camera when imported or executed.

## Environment

- Use uv and this project's `.venv`, not another project's interpreter environment.
  The default `microsoft-python` index is
  `https://packagefeedproxy.microsoft.io/pypi/simple/`.
  It is a package feed, not a distinct Python interpreter distribution.
- Preserve Python 3.10+ compatibility and the abi3-py310 boundary.
  Keep the standalone Cargo project, lockfile and build outputs independent of
  the native workspace. No GStreamer SDK is required.
- After Rust changes, rebuild the extension with maturin before Python testing.
  See the [root guide](../../README.md#python-testing-wrapper) for commands.
- Update `uv.lock` through uv when dependencies or indexes change; never bypass
  locked CI checks or TLS validation to hide dependency failures.

## Packaging and publication

- Keep versions aligned with the native library and include the MIT notice.
  Test both editable bindings and an isolated installed release wheel.
- Ensure the source distribution includes the native path dependency and builds
  independently. Exclude Python caches, debug symbols and local evidence.
- Tests, stubs, examples and package metadata follow the root provenance and
  sensitive-data rules. Do not expose raw sensitive payloads through repr/errors
  or distribute experimental captures with wheels.
