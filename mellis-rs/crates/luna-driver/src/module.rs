use std::path::PathBuf;
use std::collections::HashMap;

// ── Identity ────────────────────────────────────────────────────────

/// Unique numeric identity for a loaded module. Allocated by ModuleRegistry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

/// Logical module name as it appears in source: `import <std>` → ModuleName("std").
/// This is NOT a filesystem path; it is the language-level identity of a module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleName(pub String);

impl ModuleName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Origin & State ──────────────────────────────────────────────────

/// Where the module's content comes from.
#[derive(Debug, Clone)]
pub enum ModuleOrigin {
    /// A `.ms` source file that will be parsed and compiled.
    Source(PathBuf),
    /// A pre-compiled `.mlib` artifact.
    Mlib(PathBuf),
}

/// Tracks the loading lifecycle of a module.
/// Prevents infinite recursion on cyclic imports (A → B → A).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// Module has been discovered but not yet loaded.
    Discovered,
    /// Module is currently being loaded (cycle detection sentinel).
    Loading,
    /// Module has been fully loaded and its namespace is available.
    Loaded,
    /// Module failed to load (parse error, missing file, etc.).
    Failed,
}

// ── LoadedModule ────────────────────────────────────────────────────

/// A module that has been registered in the system.
/// Starts in Discovered state; transitions to Loading → Loaded/Failed.
pub struct LoadedModule {
    pub id: ModuleId,
    pub name: ModuleName,
    pub origin: ModuleOrigin,
    pub state: LoadState,
    /// Root scope of this module's namespace.
    /// Set to 0 (sentinel) until the module is fully loaded.
    pub root_scope: u32,
}

// ── Registry ────────────────────────────────────────────────────────

/// Central registry of all known modules.
/// Guarantees: each logical module name maps to exactly one ModuleId.
/// Multiple files importing the same module share the same entry.
pub struct ModuleRegistry {
    modules: HashMap<ModuleName, LoadedModule>,
    by_id: HashMap<ModuleId, ModuleName>,
    next_id: u32,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            by_id: HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate the next ModuleId without registering anything.
    fn alloc_id(&mut self) -> ModuleId {
        let id = ModuleId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Register a module. If the module name already exists, returns
    /// the existing ModuleId (load-once semantics).
    pub fn register(&mut self, name: ModuleName, origin: ModuleOrigin) -> ModuleId {
        if let Some(existing) = self.modules.get(&name) {
            return existing.id;
        }

        let id = self.alloc_id();
        self.by_id.insert(id, name.clone());
        self.modules.insert(name.clone(), LoadedModule {
            id,
            name,
            origin,
            state: LoadState::Discovered,
            root_scope: 0,
        });
        id
    }

    /// Look up a module by its logical name.
    pub fn get(&self, name: &ModuleName) -> Option<&LoadedModule> {
        self.modules.get(name)
    }

    /// Look up a module by its logical name (mutable).
    pub fn get_mut(&mut self, name: &ModuleName) -> Option<&mut LoadedModule> {
        self.modules.get_mut(name)
    }

    /// Look up a module by its ModuleId.
    pub fn get_by_id(&self, id: ModuleId) -> Option<&LoadedModule> {
        self.by_id.get(&id).and_then(|name| self.modules.get(name))
    }

    /// Check if a module with this name has already been registered.
    pub fn contains(&self, name: &ModuleName) -> bool {
        self.modules.contains_key(name)
    }

    /// Transition a module to Loading state.
    /// Returns the ModuleId on success.
    /// Returns Err(CycleError) if the module is already in Loading state (cyclic import).
    /// Returns Ok with existing id if already Loaded (load-once fast path).
    pub fn begin_loading(&mut self, name: &ModuleName) -> Result<BeginLoadResult, CycleError> {
        if let Some(module) = self.modules.get(name) {
            match module.state {
                LoadState::Loading => {
                    return Err(CycleError { module_name: name.clone() });
                }
                LoadState::Loaded => {
                    return Ok(BeginLoadResult::AlreadyLoaded(module.id));
                }
                LoadState::Failed => {
                    return Ok(BeginLoadResult::PreviouslyFailed(module.id));
                }
                LoadState::Discovered => {
                    // Fall through to set Loading
                }
            }
        }

        if let Some(module) = self.modules.get_mut(name) {
            module.state = LoadState::Loading;
            Ok(BeginLoadResult::StartedLoading(module.id))
        } else {
            // Module not registered yet — caller should register first
            Err(CycleError { module_name: name.clone() })
        }
    }

    /// Mark module as successfully loaded with its root scope.
    pub fn finish_loading(&mut self, name: &ModuleName, root_scope: u32) {
        if let Some(module) = self.modules.get_mut(name) {
            module.state = LoadState::Loaded;
            module.root_scope = root_scope;
        }
    }

    /// Mark module as failed to load.
    pub fn mark_failed(&mut self, name: &ModuleName) {
        if let Some(module) = self.modules.get_mut(name) {
            module.state = LoadState::Failed;
        }
    }
}

/// Result of begin_loading: distinguishes first-load from already-loaded.
#[derive(Debug)]
pub enum BeginLoadResult {
    /// Module loading has started (state transitioned Discovered → Loading).
    StartedLoading(ModuleId),
    /// Module was already fully loaded — no work needed.
    AlreadyLoaded(ModuleId),
    /// Module previously failed to load.
    PreviouslyFailed(ModuleId),
}

/// Error when a cyclic import is detected.
#[derive(Debug)]
pub struct CycleError {
    pub module_name: ModuleName,
}
