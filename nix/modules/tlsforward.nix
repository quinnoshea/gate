{ pkgs
, lib
, config
, ...
}:
let
  cfg = config.services.gate-tlsforward;
  
  # Helper to format socket addresses correctly (handle IPv6 brackets)
  formatSocketAddr = addr: port:
    if lib.hasInfix ":" addr && !lib.hasPrefix "[" addr
    then "[${addr}]:${toString port}"
    else "${addr}:${toString port}";
  
  # Generate TLS forward configuration for an instance
  mkTlsForwardConfig = name: instanceCfg: {
    server = {
      log_level = instanceCfg.logLevel;
      metrics_addr = if instanceCfg.metricsPort != null 
        then formatSocketAddr instanceCfg.bindAddress instanceCfg.metricsPort
        else null;
    };
    p2p = {
      bind_addrs = if instanceCfg.bindAddress == "::" || instanceCfg.bindAddress == "0.0.0.0"
        then [ 
          "[::]:${toString instanceCfg.p2pPort}"
          "0.0.0.0:${toString instanceCfg.p2pPort}"
        ]
        else [ (formatSocketAddr instanceCfg.bindAddress instanceCfg.p2pPort) ];
      secret_key_path = "/var/lib/gate-tlsforward-${name}/keys/secret";
      enable_discovery = true;
    };
    https_proxy = {
      bind_addr = formatSocketAddr instanceCfg.bindAddress instanceCfg.httpsPort;
      domain_suffix = instanceCfg.domainSuffix;
      max_connections = 1000;
      connection_timeout_secs = 30;
    };
    dns = {
      enabled = instanceCfg.dns.enabled;
      provider = instanceCfg.dns.provider;
      cloudflare = {
        # API token and zone ID come from environment
      };
    };
  };
  
  # Helper to create instance configurations
  mkInstance = name: instanceCfg: 
    let
      serviceName = "gate-tlsforward-${name}";
      userName = "gate-tlsforward-${name}";
      bindAddr = formatSocketAddr instanceCfg.bindAddress instanceCfg.httpsPort;
      p2pBindAddr = formatSocketAddr instanceCfg.bindAddress instanceCfg.p2pPort;
      stateDir = "/var/lib/gate-tlsforward-${name}";
      configFile = (pkgs.formats.json {}).generate "gate-tlsforward-${name}-config.json" (mkTlsForwardConfig name instanceCfg);
    in {
      inherit serviceName userName bindAddr p2pBindAddr stateDir configFile;
      config = instanceCfg;
    };
in
{
  options.services.gate-tlsforward = {
    enable = lib.mkEnableOption "Gate TLS Forward service";
    
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.gate-tlsforward;
      defaultText = lib.literalExpression "pkgs.gate-tlsforward";
      description = "The gate-tlsforward package to use";
    };

    instances = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule {
        options = {
          enable = lib.mkEnableOption "this TLS forward instance";

          httpsPort = lib.mkOption {
            type = lib.types.port;
            default = 8443;
            description = "HTTPS bind port for the TLS forward service";
          };

          p2pPort = lib.mkOption {
            type = lib.types.port;
            default = 41146;
            description = "P2P bind port for the TLS forward service";
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
            description = "Log level for the TLS forward service";
          };

          bindAddress = lib.mkOption {
            type = lib.types.str;
            default = "0.0.0.0";
            description = "IP address to bind to";
          };

          domainSuffix = lib.mkOption {
            type = lib.types.str;
            default = "gate.hellas.ai";
            description = "Domain suffix for TLS forward addresses";
          };

          dns = lib.mkOption {
            type = lib.types.submodule {
              options = {
                enabled = lib.mkOption {
                  type = lib.types.bool;
                  default = true;
                  description = "Enable DNS record management";
                };

                provider = lib.mkOption {
                  type = lib.types.enum ["cloudflare"];
                  default = "cloudflare";
                  description = "DNS provider for record management";
                };
              };
            };
            default = {};
            description = "DNS configuration";
          };

          otlpEndpoint = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            example = "http://localhost:4318";
            description = ''
              OpenTelemetry Protocol (OTLP) endpoint for exporting traces.
              If set, enables trace export to Jaeger or other OTLP collectors.
            '';
          };

          environmentFile = lib.mkOption {
            type = lib.types.nullOr lib.types.path;
            default = null;
            description = ''
              Path to environment file containing secrets.
              Should define:
              - TLSFORWARD__DNS__CLOUDFLARE__API_TOKEN
              - TLSFORWARD__DNS__CLOUDFLARE__ZONE_ID
            '';
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
        };
      });
      default = {};
      description = "Gate TLS Forward instances";
    };
  };

  config = lib.mkIf cfg.enable {
    # Create users and groups for each instance
    users.groups = lib.mapAttrs' (name: instanceCfg:
      lib.nameValuePair "gate-tlsforward-${name}" {}
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);

    users.users = lib.mapAttrs' (name: instanceCfg:
      lib.nameValuePair "gate-tlsforward-${name}" {
        isSystemUser = true;
        group = "gate-tlsforward-${name}";
        home = "/var/lib/gate-tlsforward-${name}";
        createHome = true;
      }
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);

    # Create systemd services for each instance
    systemd.services = lib.mapAttrs' (name: instanceCfg:
      let
        instance = mkInstance name instanceCfg;
      in
      lib.nameValuePair instance.serviceName {
        description = "Gate TLS Forward service (${name})";
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];
        wantedBy = [ "multi-user.target" ];

        serviceConfig = {
          Type = "exec";
          User = instance.userName;
          Group = instance.userName;
          ExecStart = "${cfg.package}/bin/gate-tlsforward -c ${instance.configFile}";
          
          Environment = [
            "RUST_LOG=${instanceCfg.logLevel}"
            "GATE_STATE_DIR=${instance.stateDir}"
          ] ++ lib.optional (instanceCfg.otlpEndpoint != null) "OTLP_ENDPOINT=${instanceCfg.otlpEndpoint}";
          
          EnvironmentFile = lib.optional (instanceCfg.environmentFile != null) instanceCfg.environmentFile;
          
          # State directory management
          StateDirectory = "gate-tlsforward-${name}";
          StateDirectoryMode = "0755";
          
          # Logging
          StandardOutput = "journal";
          StandardError = "journal";
          WorkingDirectory = instance.stateDir;
          
          # Restart behavior
          Restart = "on-failure";
          RestartSec = "5s";
          StartLimitBurst = 3;
          StartLimitIntervalSec = "1m";
          
          # Security hardening
          NoNewPrivileges = true;
          ProtectSystem = "strict";
          ProtectHome = true;  
          PrivateTmp = true;
          PrivateDevices = true;  
          ProtectHostname = true;
          ProtectClock = true;
          ProtectKernelTunables = true;
          ProtectKernelModules = true;
          ProtectKernelLogs = true;
          ProtectControlGroups = true;
          RestrictNamespaces = true;
          LockPersonality = true;
          MemoryDenyWriteExecute = true;
          RestrictRealtime = true;
          RestrictSUIDSGID = true;
          RemoveIPC = true;
          
          # Network capabilities for binding and interface management
          AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" "CAP_NET_RAW" "CAP_NET_ADMIN" ];
          CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" "CAP_NET_RAW" "CAP_NET_ADMIN" ];
        };
      }
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);

    # Configure firewall for enabled instances
    networking.firewall = 
      let
        enabledInstances = lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances;
        
        # Instances that want all interfaces
        globalInstances = lib.filterAttrs (_: instanceCfg: instanceCfg.openFirewall) enabledInstances;
        globalTcpPorts = lib.flatten (lib.mapAttrsToList (_: instanceCfg: 
          [instanceCfg.httpsPort instanceCfg.p2pPort] ++ lib.optional (instanceCfg.metricsPort != null) instanceCfg.metricsPort
        ) globalInstances);
        globalUdpPorts = lib.flatten (lib.mapAttrsToList (_: instanceCfg: [instanceCfg.p2pPort]) globalInstances);
        
        # Instances that want specific interfaces
        interfaceInstances = lib.filterAttrs (_: instanceCfg: instanceCfg.openFirewallOnInterfaces != []) enabledInstances;
        interfaceConfigs = lib.foldl (acc: instanceCfg:
          lib.foldl (acc2: iface:
            acc2 // {
              ${iface} = (acc2.${iface} or {}) // {
                allowedTCPPorts = (acc2.${iface}.allowedTCPPorts or []) ++ 
                  [instanceCfg.httpsPort instanceCfg.p2pPort] ++ lib.optional (instanceCfg.metricsPort != null) instanceCfg.metricsPort;
                allowedUDPPorts = (acc2.${iface}.allowedUDPPorts or []) ++ [instanceCfg.p2pPort];
              };
            }
          ) acc instanceCfg.openFirewallOnInterfaces
        ) {} (lib.attrValues interfaceInstances);
      in
      {
        allowedTCPPorts = globalTcpPorts;
        allowedUDPPorts = globalUdpPorts;
        interfaces = interfaceConfigs;
      };
  };
}