load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")

package(
    default_visibility = ["//:__subpackages__"],
)

rust_library(
    name = "vial-protocol",
    srcs = glob(["src/**/*.rs"]),
    aliases = aliases(),
    crate_features = [
        "default",
        "std",
    ],
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_test(
    name = "vial-protocol_test",
    crate = ":vial-protocol",
    crate_features = [
        "default",
        "std",
    ],
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ),
)
