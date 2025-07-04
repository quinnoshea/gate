use crate::{
    HookResponse, RequestHookContext, ResponseHookContext, Result, StateBackend, UsageRecord,
};
use async_trait::async_trait;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait]
pub trait GatePlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    /// Initialize the plugin with the server context
    async fn init(&mut self, context: PluginContext) -> Result<()>;

    /// Called when plugin is being shut down
    async fn shutdown(&mut self) -> Result<()>;
}

pub struct PluginContext {
    pub state: Arc<dyn StateBackend>,
    pub hooks: Arc<HookRegistry>,
}

// Hook traits
#[async_trait]
pub trait RequestHook: Send + Sync {
    /// Called before request is processed
    async fn pre_request(&self, req: &mut RequestHookContext) -> Result<HookResponse>;

    /// Called after request is processed
    async fn post_request(
        &self,
        req: &RequestHookContext,
        resp: &mut ResponseHookContext,
    ) -> Result<()>;
}

#[async_trait]
pub trait UsageHook: Send + Sync {
    /// Called when usage needs to be recorded
    async fn record_usage(&self, usage: &UsageRecord) -> Result<()>;
}

// Hook registry
pub struct HookRegistry {
    hooks: RwLock<HashMap<TypeId, Vec<Arc<dyn Any + Send + Sync>>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register<T: 'static>(&self, hook: Arc<dyn Any + Send + Sync>) {
        let mut hooks = self.hooks.write().await;
        hooks
            .entry(TypeId::of::<T>())
            .or_insert_with(Vec::new)
            .push(hook);
    }

    pub async fn get<T: 'static + Send + Sync>(&self) -> Vec<Arc<T>> {
        let hooks = self.hooks.read().await;
        hooks
            .get(&TypeId::of::<T>())
            .map(|hooks| {
                hooks
                    .iter()
                    .filter_map(|hook| hook.clone().downcast::<T>().ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Plugin manager
pub struct PluginManager {
    plugins: Vec<Box<dyn GatePlugin>>,
    hooks: Arc<HookRegistry>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            hooks: Arc::new(HookRegistry::new()),
        }
    }

    pub async fn load(
        &mut self,
        mut plugin: Box<dyn GatePlugin>,
        state: Arc<dyn StateBackend>,
    ) -> Result<()> {
        let context = PluginContext {
            state,
            hooks: self.hooks.clone(),
        };

        plugin.init(context).await?;
        self.plugins.push(plugin);
        Ok(())
    }

    pub fn hooks(&self) -> Arc<HookRegistry> {
        self.hooks.clone()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        for plugin in &mut self.plugins {
            plugin.shutdown().await?;
        }
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
