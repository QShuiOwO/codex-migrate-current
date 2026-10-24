# Third-party notices

Codex Migrate Current retains the upstream MIT license in [LICENSE](LICENSE). Its Windows release also contains open-source Rust dependencies, bundled SQLite, and the default fonts supplied by `epaint_default_fonts`.

`scripts/generate-notices.py` uses locked Cargo metadata for the release target and GUI feature to collect dependency identities, authors, license declarations, and complete license/notice files. The inventory includes normal and build dependencies; it may include optional packages Cargo resolves but does not link. Development-only dependencies are excluded. The generated `THIRD_PARTY_NOTICES.txt` and `THIRD_PARTY_COMPONENTS.json` are included in every Windows release ZIP.

Some crates omit workspace license files from their crate archive. Exact-version upstream texts are retained in `docs/third-party/`, with source URLs and covered versions in `docs/third-party/sources.json`. Notice generation fails if a dependency has neither packaged nor explicitly supplied license text. Updating dependencies may require updating these entries.

Font notices include the MIT, Bitstream Vera, SIL Open Font License 1.1, and Ubuntu Font Licence 1.0 texts shipped by `epaint_default_fonts`. No system Chinese font file is redistributed. SQLite's bundled source includes its public-domain dedication, which the generated notices preserve.

Official Windows releases use MSVC, rather than the local GNU validation toolchain. Packaging also includes the Rust toolchain's copyright and license notices. Operating-system libraries are provided by Windows and are not shipped in the ZIP.

The optional single-file standalone build uses the GNU target with LLVM-MinGW MSVCRT and checks that all imported DLLs are Windows system components. It embeds the project license and attribution, target-specific dependency notices, Rust standard-library notices, and LLVM/MinGW runtime notices in the EXE. Users can save the complete embedded text from Settings. `scripts/build-standalone.ps1` collects these texts before enabling the `standalone` Cargo feature; that feature fails to build if the notice bundle is missing. The standalone feature does not change the migration engine.
