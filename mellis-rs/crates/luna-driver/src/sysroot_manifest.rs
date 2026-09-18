use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderVisibility {
    Public,
    Internal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderEntry {
    pub name: String,
    pub path: String,
    pub visibility: ProviderVisibility,
    pub lang_contract: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SysrootManifestToml {
    provider: Vec<ProviderEntry>,
}

#[derive(Debug)]
pub struct SysrootManifest {
    providers_by_name: HashMap<String, ProviderEntry>,
    providers_by_contract: HashMap<String, String>, // contract -> name
}

impl SysrootManifest {
    pub fn load_and_validate(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read sysroot manifest {}: {}", path.display(), e))?;
        
        let toml_data: SysrootManifestToml = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse sysroot manifest {}: {}", path.display(), e))?;

        let mut providers_by_name = HashMap::new();
        let mut providers_by_contract = HashMap::new();
        let mut physical_paths = HashSet::new();

        for entry in toml_data.provider {
            // Validate name uniqueness
            if providers_by_name.contains_key(&entry.name) {
                return Err(format!("Duplicate provider name in manifest: {}", entry.name));
            }
            
            // Validate physical path security and uniqueness
            if entry.path.contains("..") || Path::new(&entry.path).is_absolute() {
                return Err(format!("Invalid physical path for provider {}: {}", entry.name, entry.path));
            }
            if !physical_paths.insert(entry.path.clone()) {
                return Err(format!("Duplicate physical path mapping in manifest: {}", entry.path));
            }

            // Validate contract uniqueness
            if let Some(contract) = &entry.lang_contract {
                if providers_by_contract.contains_key(contract) {
                    return Err(format!("Duplicate lang_contract mapping in manifest: {}", contract));
                }
                providers_by_contract.insert(contract.clone(), entry.name.clone());
            }

            providers_by_name.insert(entry.name.clone(), entry);
        }

        Ok(Self {
            providers_by_name,
            providers_by_contract,
        })
    }

    pub fn find_provider(&self, name: &str) -> Option<&ProviderEntry> {
        self.providers_by_name.get(name)
    }

    pub fn find_by_lang_contract(&self, contract_name: &str) -> Option<&ProviderEntry> {
        let name = self.providers_by_contract.get(contract_name)?;
        self.find_provider(name)
    }

    pub fn providers(&self) -> impl Iterator<Item = &ProviderEntry> {
        self.providers_by_name.values()
    }
}

impl Default for SysrootManifest {
    fn default() -> Self {
        Self {
            providers_by_name: std::collections::HashMap::new(),
            providers_by_contract: std::collections::HashMap::new(),
        }
    }
}
