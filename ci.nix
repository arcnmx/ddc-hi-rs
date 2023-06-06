{ config, channels, pkgs, lib, ... }: with pkgs; with lib; let
  inherit (import ./. { inherit pkgs; }) checks;
in {
  config = {
    name = "ddc-hi";
    ci = {
      version = "v0.6";
      gh-actions.enable = true;
    };
    cache.cachix.arc.enable = true;
    channels = {
      nixpkgs = "22.11";
    };
    tasks = {
      build.inputs = singleton checks.test;
    };
    jobs = {
      nixos = {
        tasks.windows.inputs = singleton checks.windows;
      };
      macos.system = "x86_64-darwin";
    };
  };
}
