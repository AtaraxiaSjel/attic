let
  outputs = builtins.getFlake (toString ./.);
  pkgs = outputs.inputs.nixpkgs;
  attic = pkgs.lib.collect pkgs.lib.isDerivation outputs.packages.x86_64-linux.attic;
  attic-client = pkgs.lib.collect pkgs.lib.isDerivation outputs.packages.x86_64-linux.attic-client;
  attic-server = pkgs.lib.collect pkgs.lib.isDerivation outputs.packages.x86_64-linux.attic-server;
in attic ++ attic-client ++ attic-server