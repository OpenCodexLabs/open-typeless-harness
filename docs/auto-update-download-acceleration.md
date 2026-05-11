# Auto Update Download Acceleration

## Problem

OpenLess used a single Tauri updater endpoint on GitHub Releases:

```text
https://github.com/appergb/openless/releases/latest/download/latest-{{target}}-{{arch}}.json
```

The manifest also pointed installer downloads back to GitHub Releases. On networks where GitHub release assets are slow, a small updater package can take minutes to download.

Desktop apps do not reliably inherit a user's shell proxy environment. Instead of making updater correctness depend on whether a proxy is visible to the app process, the updater should use a GitHub release acceleration URL directly.

## Runtime Behavior

The app does not manually probe local proxy ports. It lets the OS/process network stack do whatever it normally does, while the updater endpoint itself points at `fastgit.cc` first. This keeps the rule simple: proxy or no proxy, updater traffic should prefer the fastgit transport.

## Fastgit Acceleration

Release builds now publish two updater manifests per target:

```text
latest-<target>-<arch>.json
latest-<target>-<arch>-mirror.json
```

The client checks the mirror manifest first, then GitHub. The mirror manifest points its installer URL at:

```text
https://fastgit.cc/https://github.com/<repo>/releases/latest/download/<asset>
```

The updater signature still protects the downloaded package. The mirror only changes transport; it cannot replace the signed payload without verification failing.

## Maintainer Notes

Set `OPENLESS_UPDATE_MIRROR_BASE_URL` in CI to change the mirror host. Keep it formatted as a prefix for GitHub URLs, for example:

```text
https://fastgit.cc/https://github.com
```

If a mirror becomes unreliable, replace that environment value and the mirror endpoint in `openless-all/app/src-tauri/tauri.conf.json`.

## Evidence

Measured from Windows on 2026-05-01. Direct GitHub release downloads were tested with local proxy disabled to reproduce the slow path. `fastgit.cc` was tested both through the normal local proxy environment and with local proxy disabled; results vary by route, so do not treat one machine's no-proxy number as a CDN SLA.

```text
Direct GitHub installer asset, 4.78 MB, proxy disabled:
run 1: timed out after 90.75s, 1.73 MB received
run 2: timed out after 90.06s, 2.44 MB received

fastgit.cc installer asset, 4.78 MB, normal local proxy environment:
with protocol prefix:    3.12s / 3.63s / 3.39s
without protocol prefix: 2.92s / 2.45s / 2.87s

fastgit.cc target-user signal:
manual browser/download usage reported completing in under 1s without enabling a proxy.
```

This is enough to justify a `fastgit.cc` mirror path, but not enough to treat a public mirror as permanently trusted infrastructure. `fastgit.cc` explicitly documents support for GitHub release/archive acceleration and accepts GitHub links with or without the protocol prefix. Keep the mirror configurable and re-test before each release if download performance is a release blocker.
