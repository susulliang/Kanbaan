# Kankan

A personal Kanban desktop app written in Rust. The native interface uses
`egui`/`eframe`, and issue data is stored locally in SQLite.

## Run

```sh
cargo run --manifest-path src-tauri/Cargo.toml
```

The first launch creates and seeds the local database. On macOS, the database
is stored at `~/Library/Application Support/com.kankan.app/kankan.db`, matching
the previous desktop app so existing data remains available.

## Structure

- `src-tauri/src/app.rs` contains the native egui interface and UI state.
- `src-tauri/src/repository.rs` contains SQLite queries and issue operations.
- `src-tauri/src/models.rs` defines the persisted domain models.
- `src-tauri/src/db.rs` configures SQLite, runs migrations, and seeds a new database.
- `src-tauri/migrations` contains the SQLite schema migration.
