# Keyboard training

Boileplate based on [`implfuture.dev`](https://implfuture.dev/blog/rewriting-the-modern-web-in-rust).

## Running locally
```bash
env HTTP_LISTEN_ADDR=0.0.0.0:8081 RUST_BACKTRACE=1 bazel run -c opt //server:opt
```
or the unoptimized version:
```bash
env HTTP_LISTEN_ADDR=0.0.0.0:8081 RUST_BACKTRACE=1 bazel run //server:server
```

## Running the rootless podman
```
bazel run -c opt //server:image-nonroot-amd64-load
podman-compose -f podman-compose.yaml up
```

## More quotes prompt
I want to generate long quotes for a typing practice. They don't have to be actual quotes. They must be about 150 words long. Please output a text file that has one such quote on each line. Please generate 20 quotes. Put each quote on a line, and no blank lines in between. I repeat, no blank lines in between the quotes. Strictly use ASCII characters, for instance with single quotes are: ', and hyphens are just a dash. Don't output any preamble or formatting, just the quotes, without blank lines between the quotes. You must make sure not to insert a blank line between the quotes. No blank line, please.
