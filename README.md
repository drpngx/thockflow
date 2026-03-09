# Keyboard training

Boileplate based on [`implfuture.dev`](https://implfuture.dev/blog/rewriting-the-modern-web-in-rust).

## Running locally
```bash
env HTTP_LISTEN_ADDR=0.0.0.0:8081 RUST_BACKTRACE=1 RUST_LOG=info bazel run -c opt //server:opt
```
or the unoptimized version:
```bash
env HTTP_LISTEN_ADDR=0.0.0.0:8081 RUST_BACKTRACE=1 RUST_LOG=info bazel run //server:server
```

## Exporting with my environment
```
envgpg -e THOCKFLOW bazel run -c opt //server:serve
```

## Rehash the zmk behaviors file
```
env HTTP_LISTEN_ADDR=0.0.0.0:8081 RUST_BACKTRACE=1 RUST_LOG=info bazel run -c opt //server:zmk_behaviors
```
Not a `genrule`, so that I may still watch the git diffs as they pass by.

## Dump an svg
```
bazel run -c opt //server:keymap_svg -- ~/glove80/zmk-config/config/glove80.keymap
```

## More quotes prompt
I want to generate long quotes for a typing practice. They don't have to be actual quotes. They must be about 150 words long. Please output a text file that has one such quote on each line. Please generate 20 quotes. Put each quote on a line, and no blank lines in between. I repeat, no blank lines in between the quotes. Strictly use ASCII characters, for instance with single quotes are: ', and hyphens are just a dash. Don't output any preamble or formatting, just the quotes, without blank lines between the quotes. You must make sure not to insert a blank line between the quotes. No blank line, please.

## TODO
Vial:
- broken mouse scroll down weird character
- broken pre-shifted position in layout
- layout menu with moves and quick populate
- serialize to json
- check the save
- save svg
- TAB should move the inner tabs?
- test matrix

Tidy:
- implement the same pretty keys everywhere
- implement the OS pretty keys everywhere
- share code for basic operations with Traits, at some point
- tree-sitter goes in a sandboxed process
- implement C-o and C-s for all tabs


Mod-tap optimizer:
- vial connect to read the tap terms
- use kanata as a hardware-independent quick turnaround config instead
- see failure conditions from [howto](https://precondition.github.io/home-row-mods)
- tap term, permissive
- design text for emacs mods, `C-x C-s`, `C-a`, `M-b`, etc; `C-S-<tab>`, `C-M-v`.
- bilateral, miryoku, zmk
- diagnostics with helpful explanations
- hrm-enabled alternate base layer (needs to return properly from auto-mouse)
- maybe tap-dance double-click hold
- trainer
