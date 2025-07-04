{ pkgs
, lib
, config
, ...
}:
let
  cfg = config.services.gate-daemon;
  
  # Format upstream configuration for the config file
  formatUpstream = upstream: {
    inherit (upstream) name provider base_url;
    # API key will be added at runtime
  };
  
  # Generate the initial configuration file for an instance
  mkInstanceConfig = name: instanceCfg: 
    let
      stateDir = if instanceCfg.stateDir != null 
        then instanceCfg.stateDir 
        else "/var/lib/gate-daemon-${name}";
    in {
      server = {
        host = instanceCfg.host;
        port = instanceCfg.port;
        cors_origins = instanceCfg.corsOrigins;
        metrics_port = instanceCfg.metricsPort;
      };
      database = {
        url = "sqlite://${stateDir}/db.sqlite";
        max_connections = 10;
      };
      plugins = {
        enabled = instanceCfg.plugins.enabled;
        directories = instanceCfg.plugins.directories;
      };
      auth = {
        webauthn = instanceCfg.auth.webauthn;
        jwt = {
          issuer = instanceCfg.auth.jwt.issuer;
          expiration_hours = instanceCfg.auth.jwt.expirationHours;
          # Secret provided via environment
        };
      };
      upstreams = map formatUpstream instanceCfg.initialUpstreams;
      tlsforward = {
        enabled = instanceCfg.tlsforward.enable;
        tlsforward_addresses = instanceCfg.tlsforward.addresses;
        custom_domain = instanceCfg.tlsforward.customDomain;
        secret_key_path = "${stateDir}/keys/tlsforward-secret";
        heartbeat_interval = instanceCfg.tlsforward.heartbeatInterval;
        auto_reconnect = instanceCfg.tlsforward.autoReconnect;
        max_reconnect_attempts = instanceCfg.tlsforward.maxReconnectAttempts;
        reconnect_backoff = instanceCfg.tlsforward.reconnectBackoff;
      };
      letsencrypt = instanceCfg.letsencrypt;
      state_dir = stateDir;
    };
  
  # Helper to create instance configurations
  mkInstance = name: instanceCfg:
    let
      serviceName = "gate-daemon-${name}";
      userName = "gate-daemon-${name}";
      stateDir = if instanceCfg.stateDir != null 
        then instanceCfg.stateDir 
        else "/var/lib/gate-daemon-${name}";
      configFile = (pkgs.formats.json {}).generate "gate-daemon-${name}-startup.json" (mkInstanceConfig name instanceCfg);
    in {
      inherit serviceName userName stateDir configFile;
      config = instanceCfg;
    };
in
{
  options.services.gate-daemon = {
    enable = lib.mkEnableOption "Gate daemon service";
    
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.gate-daemon;
      defaultText = lib.literalExpression "pkgs.gate-daemon";
      description = "The gate-daemon package to use";
    };

    instances = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule {
        options = {
          enable = lib.mkEnableOption "this daemon instance";

          frontendPackage = lib.mkOption {
            type = lib.types.nullOr lib.types.package;
            default = if (pkgs ? gate-frontend-daemon) then pkgs.gate-frontend-daemon else null;
            defaultText = lib.literalExpression "pkgs.gate-frontend-daemon";
            description = "The frontend package containing static files to serve";
          };

          host = lib.mkOption {
            type = lib.types.str;
            default = "127.0.0.1";
            description = "Host to bind the HTTP server to";
          };

          port = lib.mkOption {
            type = lib.types.port;
            default = 3000;
            description = "HTTP port to bind to";
          };

          p2pPort = lib.mkOption {
            type = lib.types.port;
            default = 41147;
            description = "P2P port for TLS forward connections";
          };

          metricsPort = lib.mkOption {
            type = lib.types.nullOr lib.types.port;
            default = null;
            example = 9091;
            description = "Prometheus metrics endpoint port. If null, metrics endpoint is disabled.";
          };

          corsOrigins = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [];
            description = "CORS allowed origins";
          };

          stateDir = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            description = "State directory for database, runtime config, and keys. Defaults to /var/lib/gate-daemon-{name}";
          };

          logLevel = lib.mkOption {
            type = lib.types.enum ["error" "warn" "info" "debug" "trace"];
            default = "info";
            description = "Log level for the service";
          };

          environmentFile = lib.mkOption {
            type = lib.types.nullOr lib.types.path;
            default = null;
            description = ''
              Path to environment file containing secrets.
              Can define:
              - GATE_AUTH__JWT__SECRET
              - GATE_UPSTREAMS__<NAME>__API_KEY
              - GATE_LETSENCRYPT__EMAIL
            '';
          };

          otlpEndpoint = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            example = "http://localhost:4318";
            description = ''
              OpenTelemetry Protocol (OTLP) endpoint for exporting traces and metrics.
              If set, enables telemetry export to Jaeger or other OTLP collectors.
            '';
          };

          openFirewall = lib.mkOption {
            type = lib.types.bool;
            default = false;
            description = "Open firewall ports on all interfaces";
          };

          openFirewallOnInterfaces = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [];
            description = "List of network interfaces to open firewall ports on";
          };

          initialUpstreams = lib.mkOption {
            type = lib.types.listOf (lib.types.submodule {
              options = {
                name = lib.mkOption {
                  type = lib.types.str;
                  description = "Name identifier for this upstream";
                };
                provider = lib.mkOption {
                  type = lib.types.enum ["openai" "anthropic" "google" "mistral" "groq" "openrouter" "custom"];
                  description = "LLM provider type";
                };
                base_url = lib.mkOption {
                  type = lib.types.str;
                  default = "";
                  description = "Base URL for the upstream API (uses provider default if empty)";
                };
              };
            });
            default = [];
            description = "Initial upstream providers (API keys configured at runtime)";
          };

          plugins = lib.mkOption {
            type = lib.types.submodule {
              options = {
                enabled = lib.mkOption {
                  type = lib.types.bool;
                  default = true;
                  description = "Enable plugin system";
                };
                directories = lib.mkOption {
                  type = lib.types.listOf lib.types.str;
                  default = [];
                  description = "Plugin directories to load from";
                };
              };
            };
            default = {};
            description = "Plugin configuration";
          };

          auth = lib.mkOption {
            type = lib.types.submodule {
              options = {
                webauthn = lib.mkOption {
                  type = lib.types.submodule {
                    options = {
                      enabled = lib.mkOption {
                        type = lib.types.bool;
                        default = false;
                        description = "Enable WebAuthn authentication";
                      };
                      rpId = lib.mkOption {
                        type = lib.types.str;
                        default = "localhost";
                        description = "Relying Party ID (usually domain name)";
                      };
                      rpName = lib.mkOption {
                        type = lib.types.str;
                        default = "Gate Self-Hosted";
                        description = "Relying Party Name (display name)";
                      };
                      rpOrigin = lib.mkOption {
                        type = lib.types.str;
                        example = "https://gate.example.com";
                        description = "Relying Party Origin (full URL). This should match the URL users access your service from.";
                      };
                      allowedOrigins = lib.mkOption {
                        type = lib.types.listOf lib.types.str;
                        default = [];
                        description = "Additional allowed origins";
                      };
                      allowTlsForwardOrigins = lib.mkOption {
                        type = lib.types.bool;
                        default = true;
                        description = "Allow TLS forward origins automatically (*.hellas.ai domains)";
                      };
                      allowSubdomains = lib.mkOption {
                        type = lib.types.bool;
                        default = false;
                        description = "Allow subdomains of configured origins";
                      };
                    };
                  };
                  default = {};
                  description = "WebAuthn configuration";
                };
                jwt = lib.mkOption {
                  type = lib.types.submodule {
                    options = {
                      issuer = lib.mkOption {
                        type = lib.types.str;
                        default = "gate-daemon";
                        description = "JWT issuer";
                      };
                      expirationHours = lib.mkOption {
                        type = lib.types.int;
                        default = 24;
                        description = "Token expiration in hours";
                      };
                    };
                  };
                  default = {};
                  description = "JWT configuration";
                };
              };
            };
            default = {};
            description = "Authentication configuration";
          };

          tlsforward = lib.mkOption {
            type = lib.types.submodule {
              options = {
                enable = lib.mkOption {
                  type = lib.types.bool;
                  default = false;
                  description = "Enable TLS forward client functionality";
                };
                addresses = lib.mkOption {
                  type = lib.types.listOf lib.types.str;
                  default = [];
                  description = "List of TLS forward server addresses";
                };
                customDomain = lib.mkOption {
                  type = lib.types.nullOr lib.types.str;
                  default = null;
                  description = "Custom subdomain preference";
                };
                heartbeatInterval = lib.mkOption {
                  type = lib.types.int;
                  default = 30;
                  description = "Heartbeat interval in seconds";
                };
                autoReconnect = lib.mkOption {
                  type = lib.types.bool;
                  default = true;
                  description = "Auto-reconnect on disconnect";
                };
                maxReconnectAttempts = lib.mkOption {
                  type = lib.types.int;
                  default = 10;
                  description = "Maximum reconnection attempts";
                };
                reconnectBackoff = lib.mkOption {
                  type = lib.types.int;
                  default = 5;
                  description = "Reconnection backoff in seconds";
                };
              };
            };
            default = {};
            description = "TLS forward client configuration";
          };

          letsencrypt = lib.mkOption {
            type = lib.types.submodule {
              options = {
                enabled = lib.mkOption {
                  type = lib.types.bool;
                  default = false;
                  description = "Enable Let's Encrypt certificate management";
                };
                email = lib.mkOption {
                  type = lib.types.nullOr lib.types.str;
                  default = null;
                  description = "Email address for Let's Encrypt account";
                };
                staging = lib.mkOption {
                  type = lib.types.bool;
                  default = false;
                  description = "Use staging environment for testing";
                };
                domains = lib.mkOption {
                  type = lib.types.listOf lib.types.str;
                  default = [];
                  description = "Domains to request certificates for";
                };
                autoRenewDays = lib.mkOption {
                  type = lib.types.int;
                  default = 30;
                  description = "Auto-renew certificates before expiry (days)";
                };
              };
            };
            default = {};
            description = "Let's Encrypt configuration";
          };
        };
      });
      default = {};
      description = "Gate daemon instances";
    };
  };

  config = lib.mkIf cfg.enable {
    # Create users and groups for each instance
    users.groups = lib.mapAttrs' (name: instanceCfg:
      lib.nameValuePair "gate-daemon-${name}" {}
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);

    users.users = lib.mapAttrs' (name: instanceCfg:
      lib.nameValuePair "gate-daemon-${name}" {
        isSystemUser = true;
        group = "gate-daemon-${name}";
        home = if instanceCfg.stateDir != null 
          then instanceCfg.stateDir 
          else "/var/lib/gate-daemon-${name}";
        createHome = true;
      }
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);

    # Create systemd services for each instance
    systemd.services = lib.mapAttrs' (name: instanceCfg:
      let
        instance = mkInstance name instanceCfg;
      in
      lib.nameValuePair instance.serviceName {
        description = "Gate daemon service (${name})";
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];
        wantedBy = [ "multi-user.target" ];

        preStart = ''
          # Ensure state directory structure exists
          mkdir -p ${instance.stateDir}/{config,keys,plugins,data/certs,data/accounts}
          
          # Write startup configuration to state directory
          cat ${instance.configFile} > ${instance.stateDir}/config/startup.json
          
          # Set proper permissions
          chown -R ${instance.userName}:${instance.userName} ${instance.stateDir}
          chmod 750 ${instance.stateDir}
          chmod 750 ${instance.stateDir}/config
          chmod 700 ${instance.stateDir}/keys
          chmod 755 ${instance.stateDir}/data
          chmod 755 ${instance.stateDir}/data/certs
          chmod 700 ${instance.stateDir}/data/accounts
        '';

        serviceConfig = {
          Type = "exec";
          User = instance.userName;
          Group = instance.userName;
          ExecStart = "${cfg.package}/bin/gate -c ${instance.stateDir}/config/startup.json";
          
          Environment = [
            "RUST_LOG=${instanceCfg.logLevel}"
            "GATE_STATE_DIR=${instance.stateDir}"
          ] ++ lib.optional (instanceCfg.frontendPackage != null) "GATE_SERVER__STATIC_DIR=${instanceCfg.frontendPackage}"
            ++ lib.optional (instanceCfg.otlpEndpoint != null) "OTLP_ENDPOINT=${instanceCfg.otlpEndpoint}";
          
          EnvironmentFile = lib.optional (instanceCfg.environmentFile != null) instanceCfg.environmentFile;
          
          # Working directory
          WorkingDirectory = instance.stateDir;
          
          # Logging
          StandardOutput = "journal";
          StandardError = "journal";
          
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
          
          # Allow state directory access
          StateDirectory = lib.optionalString (instanceCfg.stateDir == null) "gate-daemon-${name}";
          StateDirectoryMode = "0750";
          
          # For custom state directories, we need to allow read/write access
          ReadWritePaths = lib.optional (instanceCfg.stateDir != null) instanceCfg.stateDir;
          
          # Network capabilities for P2P
          AmbientCapabilities = lib.optional (instanceCfg.tlsforward.enable || instanceCfg.p2pPort < 1024) "CAP_NET_BIND_SERVICE";
          CapabilityBoundingSet = lib.optional (instanceCfg.tlsforward.enable || instanceCfg.p2pPort < 1024) "CAP_NET_BIND_SERVICE";
        };
      }
    ) (lib.filterAttrs (_: instanceCfg: instanceCfg.enable) cfg.instances);

    # Configure firewall for all instances
    networking.firewall = let
      # Collect all TCP ports from all enabled instances
      allTcpPorts = lib.flatten (lib.mapAttrsToList (name: instanceCfg:
        let
          tcpPorts = [instanceCfg.port] 
            ++ lib.optional instanceCfg.tlsforward.enable instanceCfg.p2pPort
            ++ lib.optional (instanceCfg.metricsPort != null) instanceCfg.metricsPort;
        in
          if instanceCfg.enable && instanceCfg.openFirewall then tcpPorts else []
      ) cfg.instances);
      
      # Collect all UDP ports from all enabled instances
      allUdpPorts = lib.flatten (lib.mapAttrsToList (name: instanceCfg:
        let
          udpPorts = lib.optional instanceCfg.tlsforward.enable instanceCfg.p2pPort;
        in
          if instanceCfg.enable && instanceCfg.openFirewall then udpPorts else []
      ) cfg.instances);
      
      # Collect interface-specific rules
      interfaceRules = lib.foldl' (acc: instance:
        lib.foldl' (acc2: iface:
          acc2 // {
            ${iface} = {
              allowedTCPPorts = (acc2.${iface}.allowedTCPPorts or []) ++ 
                [instance.config.port] ++
                lib.optional instance.config.tlsforward.enable instance.config.p2pPort ++
                lib.optional (instance.config.metricsPort != null) instance.config.metricsPort;
              allowedUDPPorts = (acc2.${iface}.allowedUDPPorts or []) ++
                lib.optional instance.config.tlsforward.enable instance.config.p2pPort;
            };
          }
        ) acc instance.config.openFirewallOnInterfaces
      ) {} (lib.filter (i: i.config.enable) 
        (lib.mapAttrsToList (name: cfg: mkInstance name cfg) cfg.instances));
    in {
      allowedTCPPorts = allTcpPorts;
      allowedUDPPorts = allUdpPorts;
      interfaces = interfaceRules;
    };
  };
}