# GitHub Pages release

Public URL: <https://wotzlaff.github.io/packing-circles/>.

The site is a static, browser-local application. Uploaded input, optimizer work,
and recording validation stay on the visitor's device. GitHub serves the HTML,
JavaScript, stylesheet, build metadata, and WASM binary; no application server,
API credentials, or cross-origin isolation is required.

## Build and publish

The [Pages workflow](../.github/workflows/pages.yml) runs on relevant `main`
changes or manual dispatch. It checks formatting, Clippy, Rust, Python, frozen
datasets, numerical reference replay, same-host native-A output/counter replay,
worker/native comparisons, and short UI acceptance tests before deployment.
Thirty-second reference-machine performance measurements are documented
separately in [website.md](website.md); shared CI smoke checks do not qualify
performance. The deployment job uses the `github-pages` environment and the
repository's Pages/OIDC permissions.

Native A uses platform math, so its archived byte hashes are specific to the
qualification environment. An initial release job incorrectly required those
Debian/Rust-1.100 hashes on Ubuntu/Rust-1.99 and stopped at `10_01.txt` despite
passing all 426 numerical reference cases. The release job now compiles the
pinned Rust reference `bb3609c0bd6afa36c1af140ee0573cb8848be47f` on the same runner
and requires all 50 fixed-work outputs and counters to match exactly. It retains
the complete comparison report as a workflow artifact, including mismatches.
`python3 bench/check_a_regression.py` still checks the original archive locally;
`--reference-binary PATH --report PATH` selects same-host comparison. No numerical
tolerances or quality targets were changed.

`tools/build_wasm.py` generates the binary and `web/build-info.json`, including
the source commit, working-tree state, Rust compiler, portable numerical profile
and WASM SHA-256. `tools/package_site.py --output DIRECTORY` verifies the source
commit and hash and refuses a nonempty output directory. It copies only ten
explicitly listed public files. Tests, `node_modules`, benchmark raw records,
repository metadata, and the separate ignored legacy checkout are excluded.

The root HTML points to `assets/<commit-prefix>/app.js` and its stylesheet.
Modules, worker, WASM and build metadata all resolve relative to that directory,
so a repository base path works and successive versions cannot mix cached
modules from different commits. The footer links the browser build commit and
the native benchmark report. Exported recordings carry their build provenance.

For a local release preflight:

```sh
python3 tools/build_wasm.py
python3 tools/package_site.py --output /tmp/circles-site
python3 -m http.server 8000 --bind 127.0.0.1 --directory /tmp/circles-site
```

Use a fresh output directory on subsequent builds. Documentation-only commits
do not redeploy; the footer continues to identify the actual application release.

## Verify the published release

Install the stable browser binaries separately, then run:

```sh
node web/tests/release.mjs --url https://wotzlaff.github.io/packing-circles/ \
  --chrome /path/to/chrome --firefox /path/to/firefox --geckodriver /path/to/geckodriver \
  --expected-commit FULL_RELEASE_COMMIT --report /tmp/circles-release.json
```

The check fetches versioned build metadata, verifies the served WASM SHA-256,
opens and reloads the repository URL in both browsers, waits for the worker,
runs A then an explicitly selected B with equal budgets, and downloads each
solution through the UI. Independent checks reject malformed four-decimal text,
containment violations, or overlapping circles. Firefox uses WebDriver because
stock Firefox does not implement Playwright's patched-browser protocol.

The packaged local preflight passed in Chrome 157.0.8103.78 and Firefox 160.0
(geckodriver 0.37.1). Stable versions were obtained from the official
[Chrome for Testing feed](https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json)
and [Mozilla product feed](https://product-details.mozilla.org/1.0/firefox_versions.json).
The published URL passed the same checks on 2026-11-16 in both stable browsers.
The first Firefox export check exposed a test-harness race: Firefox creates an
empty final filename before finishing the `.part` file. The harness now waits
for a nonempty completed download; all geometry and format assertions remain
unchanged. [Initial failure](../bench/results/pages-firefox-download-race.json)
and [successful public verification](../bench/results/pages-release.json) are retained.

Verified application release: `3cf0f1ea95326a91749bc4c5f807c16a6209e3d3`.
[Checks](https://github.com/wotzlaff/packing-circles/actions/runs/43943101368)
and [Pages deployment](https://github.com/wotzlaff/packing-circles/actions/runs/43943101493)
both passed. The served WASM hash is
`4fc7da442f4c9219e975543fe3d370c507da57ba59d33d3b28ea23c2993ac475`.
The clean release build used Rust 1.99.1 and the same portable math profile.
The [same-host native CI report](../bench/results/pages-native-ci-regression.json)
records zero differences on all 50 development inputs. The original
[platform-specific failure log](../bench/results/pages-native-platform-mismatch.txt)
is preserved alongside it.

The browser uses portable software math. Native A's benchmark qualification is
preserved, and B remains explicitly experimental; publication does not claim
that a B release target or portable timed-quality qualification has passed.
