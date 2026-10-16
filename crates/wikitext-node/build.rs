//! Node loads addons with a `.node` extension, and napi-build arranges for the
//! cdylib to carry it.
fn main() {
    napi_build::setup();
}
