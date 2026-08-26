{ pkgs, lib, config, inputs, ... }:

{
  # Postgres for the accounts/vaults schema.
  services.postgres = {
    enable = true;
    listen_addresses = "127.0.0.1";
    initialDatabases = [ { name = "sync"; user = "sync"; pass = "sync"; } ];
  };

  # Garage for local S3-compatible vault blob storage. MinIO is nixpkgs
  # `insecure` (abandoned upstream, unpatched CVEs) as of this setup, so we're
  # on Garage instead.
  services.garage = {
    enable = true;
    buckets = [ "vaults" ];
  };

  env.DATABASE_URL = "postgres://sync:sync@127.0.0.1:5432/sync";

  enterShell = ''
    echo "postgres: $DATABASE_URL"
    echo "garage s3: $GARAGE_S3_ENDPOINT (bucket: vaults, admin on :$GARAGE_ADMIN_PORT)"
  '';
}
