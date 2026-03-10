load("@rules_proto//proto:defs.bzl", "proto_library")

proto_library(
    name = "zmk_meta_proto",
    srcs = ["proto/zmk/meta.proto"],
    strip_import_prefix = "proto/zmk",
    visibility = ["//visibility:public"],
)

proto_library(
    name = "zmk_core_proto",
    srcs = ["proto/zmk/core.proto"],
    strip_import_prefix = "proto/zmk",
    visibility = ["//visibility:public"],
)

proto_library(
    name = "zmk_behaviors_proto",
    srcs = ["proto/zmk/behaviors.proto"],
    strip_import_prefix = "proto/zmk",
    visibility = ["//visibility:public"],
)

proto_library(
    name = "zmk_keymap_proto",
    srcs = ["proto/zmk/keymap.proto"],
    strip_import_prefix = "proto/zmk",
    visibility = ["//visibility:public"],
)

proto_library(
    name = "zmk_studio_proto",
    srcs = ["proto/zmk/studio.proto"],
    strip_import_prefix = "proto/zmk",
    deps = [
        ":zmk_meta_proto",
        ":zmk_core_proto",
        ":zmk_behaviors_proto",
        ":zmk_keymap_proto",
    ],
    visibility = ["//visibility:public"],
)
