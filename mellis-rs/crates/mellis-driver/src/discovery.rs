use std::path::PathBuf;
use crate::module::ModuleName;
use mellis_ast::ImportKind;

#[derive(Default, Clone)]
pub struct SearchPaths {
    paths: Vec<PathBuf>,
}

impl SearchPaths {
    pub fn new() -> Self { Self { paths: Vec::new() } }
    pub fn push(&mut self, path: PathBuf) { self.paths.push(path); }
    pub fn paths(&self) -> &[PathBuf] { &self.paths }
}

#[derive(Debug)]
pub enum DiscoveryResult {
    Mlib(PathBuf),
    Source(PathBuf),
    NotFound,
}

pub struct ModuleDiscovery {
    source_dir: PathBuf,
    search_paths: SearchPaths,
}

impl ModuleDiscovery {
    pub fn new(source_dir: PathBuf, search_paths: SearchPaths) -> Self {
        Self { source_dir, search_paths }
    }

    pub fn resolve(&self, module_name: &ModuleName, _kind: ImportKind) -> DiscoveryResult {
        let mlib_name = format!("{}.mlib", module_name.as_str());
        let ms_name = format!("{}.ms", module_name.as_str());

        let mut paths_to_search = vec![self.source_dir.clone()];
        paths_to_search.extend(self.search_paths.paths().iter().cloned());

        for search_dir in paths_to_search {
            let mlib_path = search_dir.join(&mlib_name);
            if mlib_path.exists() {
                return DiscoveryResult::Mlib(mlib_path);
            }

            let ms_path = search_dir.join(&ms_name);
            if ms_path.exists() {
                return DiscoveryResult::Source(ms_path);
            }
        }
        DiscoveryResult::NotFound
    }
}
