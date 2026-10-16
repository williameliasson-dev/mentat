{ pkgs, lib, config, inputs, ... }:

{
  # Toolchain for the whole cargo workspace (tui, sync, proto), which is why
  # this lives at the repo root rather than next to any one crate.
  languages.rust = {
    enable = true;
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
  };

  # Lean shell for CI: `devenv --profile ci`, which skips rust-analyzer, a
  # language server CI never runs, whose closure drags in clang-tools, ccls
  # and chunks of llvm — the fattest part of an otherwise fresh nix store
  # download.
  profiles.ci.module = {
    languages.rust.components = lib.mkForce [ "rustc" "cargo" "clippy" "rustfmt" ];
    languages.rust.lsp.enable = lib.mkForce false;
  };

  # Local S3 for the sync server's bucket. versitygw over a plain directory:
  # unlike Garage (which ignores them) it enforces conditional PUTs
  # (If-None-Match / If-Match), which account creation and vault headers rely
  # on. A bucket is just a directory here, so creating it is a mkdir.
  processes.s3 = {
    exec = ''
      mkdir -p "$S3_DATA_DIR/$S3_BUCKET"
      exec ${pkgs.versitygw}/bin/versitygw --port 127.0.0.1:7070 \
        --access "$S3_ACCESS_KEY_ID" --secret "$S3_SECRET_ACCESS_KEY" \
        posix "$S3_DATA_DIR"
    '';
  };

  # `devenv up` starts every server; `devenv up sync` starts only that one.
  # Each server reads its own url, so a second one does not collide.
  processes.sync = {
    exec = "cargo run -p sync";

    # Rebuild and restart on source changes. `proto` is watched too, since the
    # generated grpc types compile from it.
    watch = {
      paths = [
        "${config.devenv.root}/sync/src"
        "${config.devenv.root}/proto/src"
        "${config.devenv.root}/proto/proto"
      ];
      extensions = [ "rs" "proto" ];
    };
  };

  # Dev-only credentials for the local versitygw above; never real ones.
  env.S3_ENDPOINT = "http://127.0.0.1:7070";
  env.S3_BUCKET = "vaults";
  env.S3_REGION = "us-east-1";
  env.S3_ACCESS_KEY_ID = "mentat-dev";
  env.S3_SECRET_ACCESS_KEY = "mentat-dev-secret";
  env.S3_DATA_DIR = "${config.devenv.state}/s3";

  # Points the tui at the `sync` process above. Unset outside devenv, the tui
  # runs offline.
  env.MENTAT_SYNC_URL = "http://127.0.0.1:3000";

  # To stderr, so `devenv shell -- some-command` has a clean stdout and stays
  # usable in a pipeline or a command substitution.
  enterShell = ''
    echo "s3: $S3_ENDPOINT (bucket: $S3_BUCKET)" >&2
  '';
}
