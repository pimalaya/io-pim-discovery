# TODO: move this to nixpkgs
# This file aims to be a replacement for the nixpkgs derivation.

{
  lib,
  rustPlatform,
  fetchFromGitHub,
  buildPackages,
  stdenv,
  openssl,
  pkg-config,
  installShellFiles,
  installShellCompletions ? stdenv.buildPlatform.canExecute stdenv.hostPlatform,
  installManPages ? stdenv.buildPlatform.canExecute stdenv.hostPlatform,
  buildNoDefaultFeatures ? false,
  buildFeatures ? [ ],
}:

let
  version = "0.1.0";
  hash = "";
  cargoHash = "";
  hasNativeTlsFeature = builtins.elem "native-tls" buildFeatures;

in
rustPlatform.buildRustPackage {
  inherit cargoHash version buildNoDefaultFeatures;

  pname = "io-pim-discovery";

  src = fetchFromGitHub {
    inherit hash;
    owner = "pimalaya";
    repo = "io-pim-discovery";
    rev = "v${version}";
  };

  env = {
    # OpenSSL should not be provided by vendors, not even on Windows
    OPENSSL_NO_VENDOR = "1";
  };

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs = lib.optional hasNativeTlsFeature openssl;

  buildFeatures = [ "cli" ] ++ buildFeatures;

  doCheck = false;

  postInstall =
    let
      emulator = stdenv.hostPlatform.emulator buildPackages;
      exe = stdenv.hostPlatform.extensions.executable;
    in
    lib.optionalString (lib.hasInfix "wine" emulator) ''
      export WINEPREFIX="''${WINEPREFIX:-$(mktemp -d)}"
      mkdir -p $WINEPREFIX
    ''
    + ''
      mkdir -p $out/share/{completions,man}
      ${emulator} "$out"/bin/pim-discovery${exe} manuals "$out"/share/man
      ${emulator} "$out"/bin/pim-discovery${exe} completions -d "$out"/share/completions bash elvish fish powershell zsh
    ''
    + lib.optionalString installManPages ''
      installManPage "$out"/share/man/*
    ''
    + lib.optionalString installShellCompletions ''
      installShellCompletion --bash "$out"/share/completions/pim-discovery.bash
      installShellCompletion --fish "$out"/share/completions/pim-discovery.fish
      installShellCompletion --zsh "$out"/share/completions/_pim-discovery
    '';

  meta = {
    description = "CLI and lib to discover PIM-related services, written in Rust";
    mainProgram = "pim-discovery";
    homepage = "https://github.com/pimalaya/io-pim-discovery";
    changelog = "https://github.com/pimalaya/io-pim-discovery/blob/master/CHANGELOG.md";
    license = with lib.licenses; [
      mit
      asl20
    ];
    maintainers = with lib.maintainers; [ soywod ];
  };
}
