{ pkgs ? import <nixpkgs> { } }: (import ./. { inherit pkgs; }).devShells.default
