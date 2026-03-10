load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen_toolchain")
load("@rules_rust_prost//:defs.bzl", "rust_prost_toolchain")
load("@rules_rust//rust:defs.bzl", "rust_library_group")

rust_wasm_bindgen_toolchain(
    name = "wasm_bindgen_toolchain_impl",
    wasm_bindgen_cli = "@bindeps//:wasm-bindgen-cli__wasm-bindgen",
)

toolchain(
    name = "wasm_bindgen_toolchain",
    toolchain = ":wasm_bindgen_toolchain_impl",
    toolchain_type = "@rules_rust_wasm_bindgen//:toolchain_type",
)

rust_library_group(
    name = "prost_runtime",
    deps = ["@crate_index//:prost"],
)

rust_prost_toolchain(
    name = "prost_toolchain_impl",
    prost_plugin = "@prost_bindeps//:protoc-gen-prost__protoc-gen-prost",
    prost_runtime = ":prost_runtime",
    prost_types = "@crate_index//:prost-types",
    tonic_plugin_flag = "",
)

toolchain(
    name = "prost_toolchain",
    toolchain = "prost_toolchain_impl",
    toolchain_type = "@rules_rust_prost//:toolchain_type",
)
