fn main() {
    cxx_build::bridge("src/crypto/ponc/ffi.rs")
        .file("src/crypto/ponc/ponc.cpp")
        .file("src/crypto/ponc/sha3.cpp")
        .include("src/crypto/ponc")
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-O3")
        .compile("ponc_engine");

    println!("cargo:rerun-if-changed=src/crypto/ponc/ffi.rs");
    println!("cargo:rerun-if-changed=src/crypto/ponc/ponc.cpp");
    println!("cargo:rerun-if-changed=src/crypto/ponc/ponc.h");
}
