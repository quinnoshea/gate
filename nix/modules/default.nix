# NixOS modules for Gate services
{ ... }:
{
  imports = [
    ./tlsforward.nix
    ./daemon.nix
  ];
}