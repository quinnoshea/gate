# Overlay that adds gate packages to nixpkgs
self: final: prev:
let
  # Only add packages if they exist for this system
  gatePackages = self.packages.${final.system} or {};
in
{
  gate-daemon = gatePackages.gate-daemon or null;
  gate-tlsforward = gatePackages.gate-tlsforward or null;
  gate-frontend-daemon = gatePackages.gate-frontend-daemon or null;
  gate-frontend-tauri = gatePackages.gate-frontend-tauri or null;
  gate-frontend-relay = gatePackages.gate-frontend-relay or null;
  gate = gatePackages.gate or null;
}