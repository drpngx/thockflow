load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_image_index", "oci_load", "oci_push")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_test")
load("//tools:platform_transitions.bzl", "platform_passthrough")
load("@rules_python//python:defs.bzl", "py_binary")

rust_binary(
    name = "zmk_behaviors",
    srcs = ["src/bin/zmk_behaviors.rs"],
    crate_features = ["runfiles"],
    data = ["@zmk//:behaviors"],
    edition = "2021",
    deps = all_crate_deps(
        normal = True,
    ) + [
        "//:thockflow",
        "@rules_rust//tools/runfiles",
    ],
)

rust_binary(
    name = "zmk_layouts",
    srcs = ["src/bin/zmk_layouts.rs"],
    crate_features = ["runfiles"],
    data = ["@zmk//:app"],
    edition = "2021",
    deps = all_crate_deps(
        normal = True,
    ) + [
        "@rules_rust//tools/runfiles",
    ],
)

rust_binary(
    name = "keymap_svg",
    srcs = ["src/bin/keymap_svg.rs"],
    edition = "2021",
    deps = all_crate_deps(
        normal = True,
    ) + [
        "//:thockflow",
    ],
)

rust_binary(
    name = "fetch_contrib_layouts",
    srcs = ["src/bin/fetch_contrib_layouts.rs"],
    crate_features = ["runfiles"],
    data = ["@keymap_editor_contrib//:keyboard_data"],
    edition = "2021",
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ) + [
        "@rules_rust//tools/runfiles",
    ],
)

rust_binary(
    name = "fetch_builtin_keymaps",
    srcs = ["src/bin/fetch_builtin_keymaps.rs"],
    crate_features = ["runfiles"],
    data = ["@zmk//:app"],
    edition = "2021",
    visibility = ["//visibility:public"],
    deps = all_crate_deps(
        normal = True,
    ) + [
        "@rules_rust//tools/runfiles",
    ],
)

rust_binary(
    name = "server",
    srcs = glob(["src/**/*.rs"]),
    aliases = aliases(),
    compile_data = [
        "//:static/hshs52.keymap",
        "//:static/kanata-surf.kbd",
    ],
    data = select({
        "//:opt": [
            "//:app_wasm_opt",
            "//:app_wasm_opt_br",
            "//:static_files",
        ],
        "//conditions:default": [
            "//:app_wasm",
            "//:static_files",
        ],
    }),
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = select({
        "//:opt": {
            "APP_WASM_PATH": "/app_wasm_bg_opt.wasm",
            "AXUM_PRECOMPRESSED_WASM": "1",
        },
        "//conditions:default": {},
    }),
    rustc_flags = select({
        "//:debug": [
            "-Copt-level=0",
        ],
        "//:fastbuild": [],
        "//conditions:default": [],
    }),
    deps = all_crate_deps(
        normal = True,
    ) + ["//:thockflow"],
)

rust_test(
    name = "server_test",
    crate = ":server",
    compile_data = [
        "//:static/hshs52.keymap",
        "//:static/kanata-surf.kbd",
    ],
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ) + ["//:thockflow"],
)

rust_binary(
    name = "opt",
    srcs = glob(["src/**/*.rs"]),
    aliases = aliases(),
    data = [
        "//:app_wasm",
        # wasm-opt takes ~3s to run, only use in release build
        "//:app_wasm_opt",
        "//:app_wasm_opt_br",
        "//:static_files",
    ],
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = {
        "APP_WASM_PATH": "/app_wasm_bg_opt.wasm",
        "AXUM_PRECOMPRESSED_WASM": "1",
    },
    deps = all_crate_deps(
        normal = True,
    ) + ["//:thockflow"],
)

platform(
    name = "linux_amd64",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
)

platform(
    name = "linux_arm64",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:arm64",
    ],
)

pkg_tar(
    name = "opt_tar",
    srcs = [":opt"],
    include_runfiles = True,
    package_dir = "/app",
    stamp = 1,
    # keeps relative path consistent for static files in both dev and release
    strip_prefix = "/server",
)

oci_image(
    name = "image-amd64",
    base = "@distroless_base_linux_amd64",
    cmd = ["/app/opt"],
    env = {
        "HTTP_LISTEN_ADDR": "0.0.0.0:8080",
    },

    # Ensure the tar (and thus the embedded opt binary) is built for linux/amd64
    tars = [":opt_tar_amd64"],
    workdir = "/app/opt.runfiles/_main",
)

# Rebuild the tar under the linux/amd64 platform for image-amd64
platform_passthrough(
    name = "opt_tar_amd64",
    src = ":opt_tar",
    platform = ":linux_amd64",
)

oci_image(
    name = "image-arm64",
    base = "@distroless_base_linux_arm64_v8",
    cmd = ["/app/opt"],
    env = {
        "HTTP_LISTEN_ADDR": "0.0.0.0:8080",
    },
    tars = [":opt_tar"],
    workdir = "/app/opt.runfiles/_main",
)

oci_image(
    name = "image",
    base = "@distroless_base",
    cmd = ["/app/opt"],
    env = {
        "HTTP_LISTEN_ADDR": "0.0.0.0:8080",
    },
    tars = [":opt_tar"],
    workdir = "/app/opt.runfiles/_main",
)

# This doesn't work with lambda (which doesn't support multi-arch images :( ),
# so we hard-code amd64 above.
oci_image_index(
    name = "image-multiarch",
    images = [":image"],
    platforms = [
        ":linux_amd64",
        ":linux_arm64",
    ],
)

# Nonroot image for rootless container runtimes (Podman, etc.)
# Runs as UID 65532 (nonroot user in distroless)
# Listens on localhost only since Cloudflare Tunnel connects locally
oci_image(
    name = "image-nonroot-amd64",
    base = "@distroless_base_nonroot_linux_amd64",
    cmd = ["/app/opt"],
    env = {
        "HTTP_LISTEN_ADDR": "127.0.0.1:8080",
    },
    tars = [":opt_tar_amd64"],
    workdir = "/app/opt.runfiles/_main",
)

oci_load(
    name = "image-nonroot-amd64-load",
    image = ":image-nonroot-amd64",
    repo_tags = ["thockflow:latest"],
)

filegroup(
    name = "image-nonroot-amd64-tarball",
    srcs = [":image-nonroot-amd64-load"],
    output_group = "tarball",
)

oci_load(
    name = "cloudflared-load",
    image = "@cloudflared_linux_amd64",
    repo_tags = ["cloudflare/cloudflared:latest"],
)

filegroup(
    name = "cloudflared-tarball",
    srcs = [":cloudflared-load"],
    output_group = "tarball",
)

py_binary(
    name = "serve",
    srcs = ["serve.py"],
    data = [
        "//server:image-nonroot-amd64-tarball",
        "//server:cloudflared-tarball",
        "//:podman-compose.yaml",
    ],
    imports = [".."], # Helps resolve modules from the workspace root
    deps = ["@rules_python//python/runfiles"],
)

# uses https://github.com/awslabs/amazon-ecr-credential-helper
# oci_push(
#     name = "push",
#     image = ":image-amd64",
#     remote_tags = ["latest"],
#     repository = "689191389309.dkr.ecr.us-west-1.amazonaws.com/implfuture",
# )

# oci_push(
#     name = "push-arm64",
#     image = ":image-arm64",
#     remote_tags = ["latest"],
#     repository = "689191389309.dkr.ecr.us-west-1.amazonaws.com/implfuture",
# )

# oci_push(
#     name = "push-multiarch",
#     image = ":image-multiarch",
#     remote_tags = ["latest"],
#     repository = "689191389309.dkr.ecr.us-west-1.amazonaws.com/implfuture",
# )
