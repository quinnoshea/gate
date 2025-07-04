{ pkgs
, lib
, config
, ...
}:
let
  cfg = config.services.gate-tlsforward-colmena;
  
  # Helper to create environment file from colmena secrets
  mkEnvFile = name: instanceCfg: pkgs.writeScript "gate-tlsforward-${name}-env.sh" ''
    #!/bin/sh
    # This script generates an environment file from colmena-managed secrets
    # It maps the old private-gate-relay variable names to the new gate-tlsforward format
    
    ENV_FILE="/run/gate-tlsforward-${name}/env"
    mkdir -p "$(dirname "$ENV_FILE")"
    
    # Clear the file
    : > "$ENV_FILE"
    
    # Map old environment variable names to new ones
    if [ -f "${instanceCfg.cloudflareApiTokenFile}" ]; then
      echo "RELAY__DNS__CLOUDFLARE__API_TOKEN=$(cat ${instanceCfg.cloudflareApiTokenFile})" >> "$ENV_FILE"
    fi
    
    if [ -f "${instanceCfg.cloudflareZoneIdFile}" ]; then
      echo "RELAY__DNS__CLOUDFLARE__ZONE_ID=$(cat ${instanceCfg.cloudflareZoneIdFile})" >> "$ENV_FILE"
    fi
    
    # Set permissions
    chmod 600 "$ENV_FILE"
    chown gate-tlsforward-${name}:gate-tlsforward-${name} "$ENV_FILE"
  '';
in
{
  options.services.gate-tlsforward-colmena = {
    enable = lib.mkEnableOption "Gate TLS Forward service with colmena secret management";
    
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.gate-tlsforward;
      defaultText = lib.literalExpression "pkgs.gate-tlsforward";
      description = "The gate-tlsforward package to use";
    };

    instances = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule {
        options = {
          enable = lib.mkEnableOption "this relay instance";

          httpsPort = lib.mkOption {
            type = lib.types.port;
            default = 8443;
            description = "HTTPS bind port for the relay";
          };

          p2pPort = lib.mkOption {
            type = lib.types.port;
            default = 41146;
            description = "P2P bind port for the relay";
          };

          metricsPort = lib.mkOption {
            type = lib.types.nullOr lib.types.port;
            default = null;
            example = 9090;
            description = "Prometheus metrics endpoint port. If null, metrics are disabled.";
          };

          logLevel = lib.mkOption {
            type = lib.types.enum ["error" "warn" "info" "debug" "trace"];
            default = "info";
            description = "Log level for the relay service";
          };

          bindAddress = lib.mkOption {
            type = lib.types.str;
            default = "0.0.0.0";
            description = "IP address to bind to";
          };

          domainSuffix = lib.mkOption {
            type = lib.types.str;
            default = "private.hellas.ai";
            description = "Domain suffix for relay addresses";
          };

          cloudflareApiTokenFile = lib.mkOption {
            type = lib.types.path;
            description = "Path to file containing Cloudflare API token (typically from colmena deployment.keys)";
          };

          cloudflareZoneIdFile = lib.mkOption {
            type = lib.types.path;
            description = "Path to file containing Cloudflare Zone ID (typically from colmena deployment.keys)";
          };

          openFirewall = lib.mkOption {
            type = lib.types.bool;
            default = false;
            description = "Open firewall ports for this instance on all interfaces";
          };

          openFirewallOnInterfaces = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [];
            description = "List of network interfaces to open firewall ports on";
          };

          otlpEndpoint = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            description = "OpenTelemetry endpoint for tracing (e.g. https://jaeger.example.com)";
          };
        };
      });
      default = {};
      description = "Gate Relay instances with colmena secret management";
    };
  };

  config = lib.mkIf cfg.enable {
    # Enable the base gate-tlsforward service
    services.gate-tlsforward = {
      enable = true;
      package = cfg.package;
      
      # Map colmena instances to gate-tlsforward instances
      instances = lib.mapAttrs (name: instanceCfg: {
        inherit (instanceCfg) enable httpsPort p2pPort metricsPort logLevel bindAddress domainSuffix openFirewall openFirewallOnInterfaces otlpEndpoint;
        
        # We'll dynamically generate the environment file
        environmentFile = "/run/gate-tlsforward-${name}/env";
        
        dns = {
          enabled = true;
          provider = "cloudflare";
        };
      }) cfg.instances;
    };
    
    # Create systemd services to generate environment files before gate-tlsforward starts
    systemd.services = lib.mapAttrs' (name: instanceCfg:
      lib.nameValuePair "gate-tlsforward-${name}-env" {
        description = "Generate environment file for gate-tlsforward-${name}";
        before = [ "gate-tlsforward-${name}.service" ];
        requiredBy = [ "gate-tlsforward-${name}.service" ];
        
        serviceConfig = {
          Type = "oneshot";
          ExecStart = mkEnvFile name instanceCfg;
          RemainAfterExit = true;
          
          # Run as root to read colmena secrets
          User = "root";
          Group = "root";
          
          # Create runtime directory
          RuntimeDirectory = "gate-tlsforward-${name}";
          RuntimeDirectoryMode = "0755";
        };
      }
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);
  };
}