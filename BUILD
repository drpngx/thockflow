load("@aspect_rules_js//js:defs.bzl", "js_library", "js_run_binary", "js_run_devserver", "js_test")
load("@bazel_skylib//rules:common_settings.bzl", "bool_flag")
load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")
load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")
load("//emsdk:emsdk.bzl", "wasmopt")
load("@rules_python//python:pip.bzl", "compile_pip_requirements")

package(
    default_visibility = ["//:__subpackages__"],
)

exports_files(["podman-compose.yaml"])

config_setting(
    name = "debug",
    values = {
        "compilation_mode": "dbg",
    },
)

config_setting(
    name = "fastbuild",
    values = {
        "compilation_mode": "fastbuild",
    },
)

config_setting(
    name = "opt",
    values = {
        "compilation_mode": "opt",
    },
)

bool_flag(
    name = "show_drafts",
    build_setting_default = False,
)


compile_pip_requirements(
    name = "requirements",
    requirements_in = "requirements.txt",
    requirements_txt = "requirements_lock.txt",
)

genrule(
    name = "gen_contrib_layouts",
    srcs = ["@keymap_editor_contrib//:keyboard_data"],
    outs = ["src/keymap/contrib_layouts.rs"],
    cmd = """
        # Create a temporary directory for keyboard data
        KEYBOARD_DIR=$$(mktemp -d)
        
        # Copy all JSON files from srcs to the temp directory
        for src in $(SRCS); do
            if [[ $$src == *.json ]]; then
                cp "$$src" "$$KEYBOARD_DIR/"
            fi
        done
        
        # Run the fetcher with the keyboard data directory
        $(location //server:fetch_contrib_layouts) "$$KEYBOARD_DIR" "$@"
        
        # Cleanup
        rm -rf "$$KEYBOARD_DIR"
    """,
    tools = ["//server:fetch_contrib_layouts"],
    visibility = ["//visibility:public"],
)

genrule(
    name = "gen_builtin_keymaps",
    srcs = ["@zmk//:app"],
    outs = ["src/keymap/builtin_keymaps.rs"],
    cmd = """
        # Find the boards directory in the ZMK app
        BOARDS_DIR=""
        for src in $(SRCS); do
            if [[ $$src == *"/boards/"* ]]; then
                BOARDS_DIR=$$(dirname "$$(dirname "$$(dirname "$$src")")")
                break
            fi
        done
        
        if [ -z "$$BOARDS_DIR" ]; then
            # Fallback: try to find boards directory structure
            for src in $(SRCS); do
                if [[ $$src == *app/boards* ]]; then
                    # Extract up to boards/
                    BOARDS_DIR=$${src%%/boards/*}/boards
                    break
                fi
            done
        fi
        
        if [ -z "$$BOARDS_DIR" ] || [ ! -d "$$BOARDS_DIR" ]; then
            echo "Error: Could not find boards directory in ZMK app" >&2
            exit 1
        fi
        
        # Run the fetcher with the boards directory
        $(location //server:fetch_builtin_keymaps) "$$BOARDS_DIR" "$@"
    """,
    tools = ["//server:fetch_builtin_keymaps"],
    visibility = ["//visibility:public"],
)

rust_binary(
    name = "app",
    srcs = ["src/bin/app.rs"],
    aliases = aliases(),
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_flags = select({
        ":debug": [
            "-Copt-level=0",
        ],
        ":fastbuild": [],
        "//conditions:default": [
            "-Ccodegen-units=1",
            "-Cpanic=abort",
            "-Copt-level=z",
        ],
    }),
    deps = all_crate_deps(
        normal = True,
    ) + [
        ":thockflow",
    ],
)

genrule(
    name = "validate_quotes",
    srcs = ["static/quotes.txt"],
    outs = ["quotes_validated.txt"],
    cmd = "$(location //tools:validate_quotes) --input $(location static/quotes.txt) --output $@",
    tools = ["//tools:validate_quotes"],
)

rust_library(
    name = "thockflow",
    srcs = glob(
        include = [
            "src/**/*.rs",
        ],
        exclude = [
            "src/bin/**",
            "src/keymap/contrib_layouts.rs",
            "src/keymap/builtin_keymaps.rs",
        ],
    ) + [
        ":gen_contrib_layouts",
        ":gen_builtin_keymaps",
    ],
    aliases = aliases(),
    compile_data = [
        "static/quotes.txt",
        ":validate_quotes",
    ],
    edition = "2021",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    rustc_env = select({
        ":show_drafts_config": {
            "SHOW_UNPUBLISHED": "1",
        },
        "//conditions:default": {},
    }),
    deps = all_crate_deps(
        normal = True,
    ) + [
        "//vial-protocol",
        "//proto:zmk_studio_rust_proto",
    ],
)

rust_test(
    name = "thockflow_test",
    crate = ":thockflow",
    proc_macro_deps = all_crate_deps(
        proc_macro = True,
    ),
    deps = all_crate_deps(
        normal = True,
    ) + [
        "//vial-protocol",
        "//proto:zmk_studio_rust_proto",
    ],
)

config_setting(
    name = "show_drafts_config",
    flag_values = {
        "//:show_drafts": "1",
    },
)

rust_wasm_bindgen(
    name = "app_wasm",
    target = "web",
    wasm_file = ":app",
)

js_run_binary(
    name = "tailwind",
    srcs = glob(["src/**/*.rs"]) + ["tailwind.config.js"],
    args = [
        "-c",
        "tailwind.config.js",
        "--output=$(BINDIR)/static/css/tailwind.css",
    ],
    chdir = "../../..",
    copy_srcs_to_bin = False,
    out_dirs = ["static/css"],
    tool = "//bundle:tailwindcss",
)

filegroup(
    name = "static_files",
    srcs = glob(["static/**"]) + [
        ":tailwind",
        "//bundle",
    ],
)

wasmopt(
    name = "app_wasm_opt",
    src = ":app_wasm",
    out = "app_wasm/app_wasm_bg_opt.wasm",
)

genrule(
    name = "app_wasm_opt_br",
    srcs = [":app_wasm_opt"],
    outs = ["app_wasm/app_wasm_bg_opt.wasm.br"],
    cmd = "$(execpath @brotli) -9 $<",
    tools = ["@brotli"],
)
