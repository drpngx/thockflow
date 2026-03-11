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

To pull the latest cloudflared:
```
bazel fetch @cloudflared//...
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

## ZMK Studio initial bytes
You may request the client to what it read from the keyboard in a json using [`dump_init=1`](http://127.0.0.1:8081/zmk-studio?dump_init=1).

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
- implement the layer menu everywhere (about quick fill and kanata shadow keys)
- test write paths


Finish basic implementing Kanata:
- process-unmapped-keys and shadow keys
We need to improve the Kanata tab. We will add keys in the layout that are not in the `defsrc`. We will just draw the outline of the key without filling it out. It will be availabe to the `j` menu. If the user modifies it from its basic state, then it will be added to the `defsrc` in its proper place unless `process-unmapped-keys=yes`. We need a design and implementation doc that contains planning for extensive tests.
- support `none` as XX ✗ ∅ •: display and parse
We need to improve the Kanata tab. We need to support the `none` key. It can be added in the config and completion as `XX ✗ ∅ •` and we will always display it as ∅.
- pre-modified keys: C-S-c
We need to improve the Kanata tab. We will pre-modified keys support such as `C-S-a` which means control-shift-a. You can see how it's done here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc#output-chordscombos We need to modify the comletion so that we can see `C-`, etc. If we select this, then we will see the next keys, so `C-` can complete to `C-S-` or `C-a`. We need a design and implementation doc that contains planning for extensive tests.
- mousemove variants, setmouse, mousemove-speed
- cmd
- clipboard variants
- defchords


Mod-tap optimizer:
- see failure conditions from [howto](https://precondition.github.io/home-row-mods)
- tap term, permissive
- design text for emacs mods, `C-x C-s`, `C-a`, `M-b`, etc; `C-S-<tab>`, `C-M-v`.
- bilateral, miryoku, zmk
- diagnostics with helpful explanations
- hrm-enabled alternate base layer (needs to return properly from auto-mouse)
- maybe tap-dance double-click hold
- trainer
