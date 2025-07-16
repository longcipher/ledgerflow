---
description: "Instructions for generating code for Rust backend"
applyTo: "**"
---

- Make sure to always use the latest version of all libraries
- `context7`: to get the latest docs and code for any library
- Use `torii-rs` for authentication and session management
- Use `axum` for building REST APIs
- Always use `config`, `tracing`, `clap`, `eyre` for configuration, logging, command-line parsing, and error handling.
- Prefer to use `sqlx` for database interactions, or use `sea-orm` for ORM capabilities.
- Run `just format` to format the code
- Run `just lint` to lint the code, alwasy make sure that zero warnings and errors are present.
- Always generate and update the README.md file after making code changes.
- Always use English for comments, strings and documentation.
- Always use workspace dependencies in `Cargo.toml` to avoid version conflicts.
