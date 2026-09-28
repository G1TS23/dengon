//! `uniffi-bindgen` épinglé sur la même version qu'`uniffi` (=0.28.3) : un
//! générateur d'une autre version produirait des bindings incompatibles avec
//! le scaffolding compilé dans `libdengon_ffi.so`. Usage :
//! `android/scripts/build-ffi.sh`.

fn main() {
    uniffi::uniffi_bindgen_main();
}
