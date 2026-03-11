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
- save keymap
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
We need to improve the Kanata tab. The layout must have a menu, exactly like we have in the `keymap` tab. The menu contains: move up, move down, rename, duplicate, delete, and then all to none, trans to none, none to trans, quick assignment. We need a design and implementation doc with error checking and edge cases, that contains planning for exhaustive tests.

We need to improve the Vial tab. The layout must have a menu, exactly like we have in the `keymap` tab. The menu contains: move up, move down, rename, duplicate, delete, and then all to none, trans to none, none to trans, quick assignment. We need a design and implementation doc with error checking and edge cases, that contains planning for exhaustive tests.

We need to improve the ZMK Studio tab. The layout must have a menu, exactly like we have in the `keymap` tab. The menu contains: move up, move down, rename, duplicate, delete, and then all to none, trans to none, none to trans, quick assignment. We need a design and implementation doc with error checking and edge cases, that contains planning for exhaustive tests.

- test write paths


Finish basic implementing Kanata:
- process-unmapped-keys and shadow keys
- support `none` as XX ✗ ∅ •: display and parse
We need to improve the Kanata tab. We need to support the `none` key. It can be added in the config and completion as `XX ✗ ∅ •` and we will always display it as ∅.
- pre-modified keys: C-S-c
We need to improve the Kanata tab. We will pre-modified keys support such as `C-S-a` which means control-shift-a. You can see how it's done here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc#output-chordscombos We need to modify the completion so that we can see `C-`, etc. If we select this, then we will see the next keys, so `C-` can complete to `C-S-` or `C-a`. We need a design and implementation doc that contains planning for extensive tests.
- mousemove variants, setmouse, mousemove-speed
We need to improve the Kanata tab. We will add support for mousemove variants, setmouse, and mousemove-speed. You can see how it's done here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc#mouse-movement We need a design and implementation doc that contains planning for extensive tests.
- cmd
We need to improve the Kanata tab. We will add support for the `cmd` action which takes strings. You can see how it's done here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc The command can be unquoted strings for instance `(cmd bazel build -c opt //...)` We need a design and implementation doc that contains planning for extensive tests.
- clipboard variants
We need to improve the Kanata tab. We will add support for clipboard ring. You can see how it's done here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc#clipboard-actions We need a design and implementation doc that contains planning for extensive tests.
- defchords
We need to improve the Kanata tab. We will add support for defchords. You can see how it's done here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc#input-chords--combos-v2 We need to be mindful of the parameters types for completion. We need a design and implementation doc that contains planning for extensive tests.
- is_laptop: true detection using proper BatteryManager and screen size


Mod-tap optimizer:
- see failure conditions from [howto](https://precondition.github.io/home-row-mods)
- tap term, permissive
- design text for emacs mods, `C-x C-s`, `C-a`, `M-b`, etc; `C-S-<tab>`, `C-M-v`.
- bilateral, miryoku, zmk
- diagnostics with helpful explanations
- hrm-enabled alternate base layer (needs to return properly from auto-mouse)
- maybe tap-dance double-click hold
- trainer
