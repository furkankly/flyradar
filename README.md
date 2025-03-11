<p align="center">
    <img src="https://raw.githubusercontent.com/furkankly/flyradar/main/website/priv/flyradar.png" width="600" />
</p>

<p align="center">
<em>Manage your Fly.io resources in style!</em>
</p>

<hr/>

<p align="center">
<a href="https://crates.io/crates/flyradar"><img src="https://img.shields.io/crates/v/flyradar.svg?style=flat&labelColor=1d1d1d&color=5b21b6&logo=Rust&logoColor=white" /></a>
<a href="https://github.com/furkankly/homebrew-tap"><img src="https://img.shields.io/badge/homebrew-available-success?style=flat&labelColor=1d1d1d&color=5b21b6&logo=homebrew&logoColor=white" /></a>
<a href="https://github.com/furkankly/flyradar/actions?query=workflow%3A%22release%22"><img src="https://img.shields.io/github/actions/workflow/status/furkankly/flyradar/release.yml?style=flat&labelColor=1d1d1d6&color=white&logo=GitHub%20Actions&logoColor=white&label=deploy" /></a>
</p>

<h4 align="center">
  <img src="https://raw.githubusercontent.com/furkankly/flyradar/main/website/priv/flyradar.svg" width="64" ></img>
  &nbsp;
<a href="https://flyradar.fly.dev/">Website</a>

</h4>

🪁 flyradar 🌟 is a terminal UI for managing and monitoring your Fly.io resources, inspired by [k9s](https://github.com/derailed/k9s). It provides an intuitive, keyboard-driven interface to interact with your Fly.io apps, and more—all from your terminal.

<p align="center">
  <img src="https://via.placeholder.com/800x400?text=flyradar+demo" alt="flyradar demo" width="80%" />
</p>

<p align="center">
<em>Manage your Fly.io apps, VMs, volumes and secrets - all in your terminal</em>
</p>

## Quickstart

> [!NOTE]
>
> _flyradar_ is an OSS third-party tool and is not an official Fly.io project.

> [!IMPORTANT]
>
> _flyradar_ relies on the Fly CLI for authentication and its built-in agent for operational functionality. Make sure you have [flyctl](https://fly.io/docs/hands-on/install-flyctl/) installed on your system before proceeding.

Install `flyradar` with `cargo`:

```bash
cargo install flyradar
```

> [!NOTE]  
> See the other [installation methods](https://flyradar.fly.dev/#installation) 📦

Make sure you are authenticated into `fly`:

```bash
fly auth login
```

Just run `flyradar`:

```bash
flyradar
```

## Features

- 💻 Interactive terminal UI for managing Fly.io resources
- 🔄 Real-time monitoring of your applications and other resources
- 🔍 Quick access to resource logs with filtering and dumping capabilities
- 🎯 Focused on operational workflows (viewing, monitoring, deleting, logging) for existing resources
  - Creation workflows are intentionally left to the Fly CLI, as they tend to be more complex and change more frequently
  - Operational workflows have more stable interfaces and are naturally well-suited for a terminal UI

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Guidelines

- This project follows [Conventional Commits](https://www.conventionalcommits.org/) for all commit messages (e.g., `feat(logs): add dumping logs`, `fix(tui): fix tab completion for command input`)
- Before submitting a PR, please run:
  - `cargo fmt` to ensure consistent code formatting
  - `cargo clippy` to catch common mistakes and improve code quality
  - `cargo test` to verify your changes don't break existing functionality

### Project Structure

This project implements parts of several components from [flyctl](https://github.com/superfly/flyctl) in Rust, with a focus on maintaining equivalent functionality:

- `src/agent` → [flyctl/agent](https://github.com/superfly/flyctl/tree/master/agent) (client-side only)
- `src/logs` → [flyctl/logs](https://github.com/superfly/flyctl/tree/master/logs)
- `src/ops` → [flyctl/internal/command](https://github.com/superfly/flyctl/tree/master/internal/command)
- `src/wireguard` → [flyctl/internal/wireguard/wg.go](https://github.com/superfly/flyctl/blob/master/internal/wireguard/wg.go)
- `src/fly_rust` → [fly-go](https://github.com/superfly/fly-go)

> [!NOTE]
>
> Only the necessary functionality from these components are implemented while aiming to maintain the same behavior and interfaces where possible.

This project uses a patched version of the [async-nats](https://github.com/nats-io/nats.rs/tree/main) crate to enable IPC communication with the Fly agent. You can find the [fork here](https://github.com/furkankly/nats.rs/tree/ipc-support).

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

## Acknowledgements

- [Fly.io](https://fly.io) for their amazing platform
- [k9s](https://github.com/derailed/k9s) for inspiration
