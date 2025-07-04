# Gate NixOS Modules

This directory contains NixOS modules and overlays for deploying Gate services.

## Usage

### In a NixOS Configuration

Add gate to your flake inputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    gate.url = "github:hellas-ai/gate";
  };

  outputs = { self, nixpkgs, gate, ... }: {
    nixosConfigurations.myserver = nixpkgs.lib.nixosSystem {
      modules = [
        gate.nixosModules.default
        {
          services.gate-daemon = {
            enable = true;
            port = 8080;
            initialUpstreams = [{
              name = "openai";
              provider = "openai";
            }];
          };
        }
      ];
    };
  };
}
```

### Available Modules

#### `gate-daemon`

The main Gate service that provides the AI gateway functionality.

```nix
services.gate-daemon = {
  enable = true;
  
  # Network configuration
  host = "127.0.0.1";
  port = 3000;
  p2pPort = 41147;
  
  # Firewall options
  openFirewall = true;  # Opens on all interfaces
  openFirewallOnInterfaces = [ "eth0" "wg0" ];  # Or specific interfaces
  
  # Initial upstream providers (API keys added at runtime)
  initialUpstreams = [
    { name = "openai"; provider = "openai"; }
    { name = "anthropic"; provider = "anthropic"; }
  ];
  
  # Relay client configuration
  relay = {
    enable = true;
    addresses = [ "relay.hellas.ai:443" ];
    customDomain = "myserver";  # Optional: myserver.gate.hellas.ai
  };
  
  # Environment file for secrets
  environmentFile = "/run/secrets/gate-daemon";
};
```

#### `gate-relay`

The relay server that enables P2P connections and HTTPS proxying.

```nix
services.gate-relay = {
  enable = true;
  
  instances.default = {
    enable = true;
    httpsPort = 443;
    p2pPort = 31145;
    logLevel = "info";
    
    # Firewall configuration
    openFirewallOnInterfaces = [ "eth0" ];
    
    # DNS provider settings
    domainSuffix = "gate.hellas.ai";
    dns = {
      enabled = true;
      provider = "cloudflare";
    };
    
    # Secrets via environment file
    environmentFile = "/run/secrets/gate-relay";
  };
};
```

### Runtime Configuration

Gate daemon supports runtime configuration for flexibility:

1. **Initial Configuration**: Set via NixOS module options
2. **Runtime Configuration**: Modified via web UI or CLI
3. **Configuration Priority**: Environment > Runtime > Nix > Defaults

Runtime configuration is stored in `/var/lib/gate/config.toml` and persists across restarts.

### Secrets Management

Sensitive values should be provided via environment files:

```bash
# /run/secrets/gate-daemon
GATE_AUTH__JWT__SECRET=your-secret-here
GATE_UPSTREAMS__OPENAI__API_KEY=sk-...
GATE_LETSENCRYPT__EMAIL=admin@example.com

# /run/secrets/gate-relay
RELAY__DNS__CLOUDFLARE__API_TOKEN=your-token
RELAY__DNS__CLOUDFLARE__ZONE_ID=your-zone-id
```

### Overlay Usage

To add Gate packages to your nixpkgs:

```nix
{
  nixpkgs.overlays = [ gate.overlays.default ];
  
  environment.systemPackages = with pkgs; [
    gate-daemon
    gate-relay
  ];
}
```

## Development

### Testing Modules Locally

```bash
# Build and test a configuration
nix build -f '<nixpkgs/nixos>' system \
  --arg configuration ./nix/test-config.nix

# Run in a VM
nixos-rebuild build-vm \
  --flake .#test-vm
```

### Multiple Instances

Both services support running multiple instances:

```nix
services.gate-relay.instances = {
  public = {
    enable = true;
    httpsPort = 443;
    p2pPort = 31145;
  };
  internal = {
    enable = true;
    httpsPort = 8443;
    p2pPort = 31146;
    bindAddress = "10.0.0.1";
  };
};
```