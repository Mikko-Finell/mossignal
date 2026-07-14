# mossignal

## Development

Install Rust and [uv](https://docs.astral.sh/uv/getting-started/installation/), then run:

```bash
make setup
make check-dev
```

`make setup` creates the repository-local Python environment from the committed `uv.lock`. Python tooling runs through `uv`; Rust commands continue to use Cargo normally.
