//! The npm package of mxs, in js/package.

#[test]
fn the_npm_package_has_the_crate_s_version() {
    let manifest = include_str!("../js/package/package.json");
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("\"version\": \"{version}\"");
    assert!(
        manifest.contains(&expected),
        "js/package/package.json has no {expected}"
    );
}
