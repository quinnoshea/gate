//! Test iroh API behavior for understanding address discovery

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use tokio::time::timeout;
    use n0_watcher::Watcher;

    #[tokio::test]
    async fn test_iroh_node_addr_watcher() {
        println!("=== Testing iroh node_addr() API ===");
        
        // Create endpoint
        let mut rng = rand::thread_rng();
        let secret_key = iroh::SecretKey::generate(&mut rng);
        
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .bind_addr_v4("0.0.0.0:0".parse().unwrap())
            .relay_mode(iroh::RelayMode::Disabled)
            .discovery_local_network()
            .bind()
            .await
            .expect("Failed to create endpoint");
            
        println!("✓ Endpoint created - Node ID: {}", endpoint.node_id());
        
        // Test if node_addr() method exists and what it returns
        println!("Checking node_addr() method...");
        
        // Get the node_addr watcher
        let node_addr_watcher = endpoint.node_addr();
        println!("✓ node_addr() method exists and returns a Watcher");
        
        // Try to get current value
        match node_addr_watcher.get() {
            Ok(Some(current_addr)) => {
                println!("  Immediate value: {:?}", current_addr);
                println!("  Direct addresses: {:?}", current_addr.direct_addresses);
            }
            Ok(None) => {
                println!("  Immediate value: None (addresses not discovered yet)");
                
                // Wait and check periodically
                for i in 1..=5 {
                    println!("  Waiting {} seconds for discovery...", i);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    
                    match node_addr_watcher.get() {
                        Ok(Some(addr)) => {
                            println!("  ✓ After {}s: {:?}", i, addr);
                            println!("    Direct addresses: {:?}", addr.direct_addresses);
                            break;
                        }
                        Ok(None) => {
                            println!("    After {}s: Still None", i);
                        }
                        Err(e) => {
                            println!("    After {}s: Error: {:?}", i, e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("  ❌ Error getting node_addr: {:?}", e);
            }
        }
        
        // Also check bound_sockets for comparison
        let bound_sockets = endpoint.bound_sockets();
        println!("Bound sockets: {:?}", bound_sockets);
        
        println!("=== End iroh API Test ===");
    }

    #[tokio::test]  
    async fn test_address_discovery_timing() {
        println!("=== Testing Address Discovery Timing ===");
        
        let mut rng = rand::thread_rng();
        let secret_key = iroh::SecretKey::generate(&mut rng);
        
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .bind_addr_v4("0.0.0.0:0".parse().unwrap())
            .discovery_local_network()
            .bind()
            .await
            .expect("Failed to create endpoint");
            
        // Test the actual pattern used in daemon
        println!("Testing daemon's node_addr pattern...");
        
        // Try to replicate the commented-out code pattern
        let node_addr_watcher = endpoint.node_addr();
        
        // Test immediate access (what daemon currently does wrong)
        match node_addr_watcher.get() {
            Ok(Some(addr)) => {
                println!("✓ Addresses available immediately: {:?}", addr.direct_addresses);
            }
            Ok(None) => {
                println!("⚠️ No addresses available immediately - need to wait");
                
                // Test waiting with timeout (what daemon should do)
                match timeout(Duration::from_secs(10), async {
                    loop {
                        match node_addr_watcher.get() {
                            Ok(Some(addr)) => return addr,
                            Ok(None) => tokio::time::sleep(Duration::from_millis(100)).await,
                            Err(_) => break,
                        }
                    }
                    panic!("Watcher disconnected");
                }).await {
                    Ok(addr) => {
                        println!("✓ Addresses discovered after waiting: {:?}", addr.direct_addresses);
                    }
                    Err(_) => {
                        println!("❌ Timeout waiting for address discovery");
                    }
                }
            }
            Err(e) => {
                println!("❌ Error accessing node_addr watcher: {:?}", e);
            }
        }
        
        println!("=== End Timing Test ===");
    }
}