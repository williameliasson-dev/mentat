{ pkgs, lib, config, inputs, ... }:

{
  # Toolchain for the whole cargo workspace (tui, sync, proto), which is why
  # this lives at the repo root rather than next to any one crate.
  languages.rust = {
    enable = true;
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
  };

  # One postgres instance with a database per server, each owned by its own
  # role, so servers cannot read each other's tables. Adding a server means
  # adding a database here, a process below, and its own *_DATABASE_URL.
  services.postgres = {
    enable = true;
    listen_addresses = "127.0.0.1";
    initialDatabases = [
      { name = "sync"; user = "sync"; pass = "sync"; }
    ];
  };

  # Garage for local S3-compatible vault blob storage. MinIO is nixpkgs
  # `insecure` (abandoned upstream, unpatched CVEs) as of this setup, so we're
  # on Garage instead.
  services.garage = {
    enable = true;
    buckets = [ "vaults" ];
  };

  # `devenv up` starts every server; `devenv up sync` starts only that one.
  # Each server reads its own url, so a second one does not collide.
  processes.sync = {
    exec = "cargo run -p sync";

    # Rebuild and restart on source changes. `proto` is watched too, since the
    # generated grpc types compile from it, and `migrations` because a new one
    # only runs on startup.
    watch = {
      paths = [
        "${config.devenv.root}/sync/src"
        "${config.devenv.root}/sync/migrations"
        "${config.devenv.root}/proto/src"
        "${config.devenv.root}/proto/proto"
      ];
      extensions = [ "rs" "proto" "sql" ];
    };
  };

  env.SYNC_DATABASE_URL = "postgres://sync:sync@127.0.0.1:5432/sync";
  # Tests and ad-hoc psql default to the sync database.
  env.DATABASE_URL = "postgres://sync:sync@127.0.0.1:5432/sync";

  enterShell = ''
    echo "postgres: $DATABASE_URL"
    echo "garage s3: $GARAGE_S3_ENDPOINT (bucket: vaults, admin on :$GARAGE_ADMIN_PORT)"
  '';
}
