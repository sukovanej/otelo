# otelo

otelo receives the OpenTelemetry logs, traces, and metrics of the apps on one server, stores them on that server, and shows them in a web UI and a CLI. It is one binary, and it covers one server.

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/sukovanej/otelo/releases/latest/download/otelo-installer.sh | sh
```

The installer puts `otelo` in `~/.local/bin`. Releases cover macOS and Linux on aarch64 and x86_64.

## Update

```sh
otelo update            # the newest release
otelo update --canary   # the newest build of main
```

A running daemon keeps the old version until it restarts. [CHANGELOG.md](CHANGELOG.md) lists the changes of each release.

## License

[MIT](LICENSE)
