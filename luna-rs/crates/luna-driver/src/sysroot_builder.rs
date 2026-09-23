use crate::sysroot::Sysroot;
use crate::{compile, CompilerOptions};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// RAII guard for build lock lifecycle.
/// Ensures the `.build.lock` file is removed on scope exit, regardless of success or error.
///
/// # Limitation
/// Like all filesystem-level advisory locks, if the host process crashes abruptly
/// (e.g. `SIGKILL`, sudden power failure) without stack unwinding, `.build.lock`
/// remains on disk and must be manually cleared.
struct BuildLockGuard {
    path: PathBuf,
}

impl BuildLockGuard {
    /// Acquire the build lock exclusively via `create_new(true)`.
    /// Returns error if lock already exists (another builder is active).
    fn acquire(path: &Path) -> Result<Self, String> {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(_file) => Ok(Self {
                path: path.to_path_buf(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err("Build lock already exists".to_string())
            }
            Err(e) => Err(format!("Failed to create build lock: {}", e)),
        }
    }
}

impl Drop for BuildLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Debug)]
pub struct SysrootBuilder {
    pub sysroot: Sysroot,
}

impl SysrootBuilder {
    pub fn new(sysroot: Sysroot) -> Self {
        Self { sysroot }
    }

    pub fn build_all(&self, quiet: bool) -> Result<(), String> {
        let external_dir = self.sysroot.external_dir();
        if !external_dir.exists() {
            return Err("external_dir does not exist".to_string());
        }

        let lock_path = external_dir.join(".build.lock");

        // Use RAII guard for lock lifecycle
        let _lock = BuildLockGuard::acquire(&lock_path)?;

        let manifest = self.sysroot.manifest();

        let mut providers: Vec<String> = Vec::new(); // logical names
        let mut provider_paths: HashMap<String, PathBuf> = HashMap::new();

        for entry in manifest.providers() {
            let logical_name = entry.name.clone();
            let path = external_dir.join(format!("{}.ln", entry.path));
            if !path.exists() {
                return Err(format!(
                    "Source file for provider {} not found at {}",
                    entry.name,
                    path.display()
                ));
            }
            providers.push(logical_name.clone());
            provider_paths.insert(logical_name, path);
        }

        // 1. Build logical provider dependency graph
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for provider in &providers {
            let path = &provider_paths[provider];
            let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let mut deps = Vec::new();

            let lexer = luna_lexer::Lexer::new(&content, luna_common::ids::FileId(0));
            let mut arena = luna_ast::AstArena::new();
            let mut parser =
                luna_parser::Parser::new(lexer, &mut arena, luna_common::ids::FileId(0));

            if let Ok(items) = parser.parse_file() {
                for item in items {
                    if let luna_ast::Item::Decl(decl_id) = item {
                        if let luna_ast::Decl::Import {
                            name: name_span,
                            kind: luna_ast::ImportKind::External,
                            ..
                        } = &arena.decls[decl_id.0 as usize]
                        {
                            let mut name_str =
                                &content[name_span.start as usize..name_span.end as usize];
                            if name_str.starts_with('"') && name_str.ends_with('"') {
                                name_str = &name_str[1..name_str.len() - 1];
                            }
                            if let Some(dep_entry) = manifest.find_provider(name_str) {
                                deps.push(dep_entry.name.clone());
                            } else {
                                return Err(format!(
                                    "Unknown sysroot provider '{}' imported by '{}'",
                                    name_str, provider
                                ));
                            }
                        }
                    }
                }
            } else {
                return Err(format!("Failed to parse provider: {}", provider));
            }

            graph.insert(provider.clone(), deps);
        }

        // 2. Deterministic topological sort using Kahn's algorithm
        // Tie-break policy: lexicographical order of canonical logical provider ID via BTreeSet
        let ordered = Self::topological_sort_with_stable_order(&graph)?;

        // 3. Quarantine obsolete artifacts (Phase 3 legacy monolithic artifacts)
        let obsolete = ["core.llib", "alloc.llib", "io.llib"];
        for obsolete_name in &obsolete {
            let path = external_dir.join(obsolete_name);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }
        let obsolete_obj = ["core.obj", "alloc.obj", "io.obj"];
        for obsolete_name in &obsolete_obj {
            let path = external_dir.join(obsolete_name);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }

        // 4. Emit `.llib` and `.obj` in dependency order
        for provider in ordered {
            if !quiet {
                println!("Building {} ...", provider);
            }
            let path = &provider_paths[&provider];
            let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

            let llib_path = external_dir.join(format!(
                "{}.llib",
                manifest.find_provider(&provider).unwrap().path
            ));

            let options = CompilerOptions {
                search_paths: vec![self.sysroot.root().to_string_lossy().to_string()],
                quiet: true,
                emit_llib: true,
                no_link: true,
                output_path: Some(llib_path.to_string_lossy().to_string()),
                is_sysroot_build: true,
                ..Default::default()
            };

            // 5. Validate each artifact immediately
            match compile(path.to_str().unwrap(), content, &options) {
                Ok(_) => {
                    if !quiet {
                        println!("Successfully built {}", provider);
                    }
                }
                Err(diags) => {
                    let mut err_str = format!("Failed to build {}:\n", provider);
                    for d in diags {
                        err_str.push_str(&format!("  {:?}\n", d));
                    }
                    return Err(err_str);
                }
            }
        }

        if !quiet {
            println!("Sysroot built successfully.");
        }

        Ok(())
    }

    /// Deterministic topological sort using Kahn's algorithm.
    ///
    /// # Tie-Break Policy
    /// For nodes with equal ready priority (0 remaining prerequisites), the canonical
    /// logical provider ID (lexicographical order) is the frozen stable tie-break.
    /// The ready queue is backed by `BTreeSet<String>` to enforce this naturally without `sort()`.
    fn topological_sort_with_stable_order(
        graph: &BTreeMap<String, Vec<String>>,
    ) -> Result<Vec<String>, String> {
        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
        for node in graph.keys() {
            in_degree.insert(node.clone(), 0);
        }

        for (node, deps) in graph.iter() {
            let valid_deps: Vec<_> = deps.iter()
                .filter(|dep| in_degree.contains_key(*dep))
                .collect();
            if let Some(cnt) = in_degree.get_mut(node) {
                *cnt = valid_deps.len();
            }
        }

        // Ready queue with BTreeSet guarantees lowest canonical logical ID is popped first
        let mut queue: BTreeSet<String> = in_degree
            .iter()
            .filter(|(_, &cnt)| cnt == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut result: Vec<String> = Vec::with_capacity(graph.len());

        while let Some(node) = queue.pop_first() {
            result.push(node.clone());

            let successors: Vec<String> = graph
                .iter()
                .filter(|(_, deps)| deps.contains(&node))
                .map(|(name, _)| name.clone())
                .collect();

            for successor in successors {
                if let Some(cnt) = in_degree.get_mut(&successor) {
                    *cnt = cnt.saturating_sub(1);
                    if *cnt == 0 {
                        queue.insert(successor);
                    }
                }
            }
        }

        if result.len() != graph.len() {
            let blocked_nodes: BTreeSet<String> = graph
                .keys()
                .filter(|n| !result.contains(n))
                .cloned()
                .collect();
            let cycle = Self::reconstruct_cycle(graph, &blocked_nodes);
            return Err(format!("Dependency cycle detected: {}", cycle.join(" -> ")));
        }

        Ok(result)
    }

    /// Reconstruct an actual closed directed cycle (e.g. `["A", "B", "A"]`) from among blocked nodes.
    /// Uses DFS with recursion stack. Only nodes actively participating in the cycle are returned;
    /// downstream nodes that merely depend on the cycle (e.g. C -> A when A <-> B) are excluded.
    fn reconstruct_cycle(
        graph: &BTreeMap<String, Vec<String>>,
        blocked_nodes: &BTreeSet<String>,
    ) -> Vec<String> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut in_stack: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = Vec::new();

        fn dfs(
            current: &str,
            graph: &BTreeMap<String, Vec<String>>,
            blocked_nodes: &BTreeSet<String>,
            visited: &mut BTreeSet<String>,
            in_stack: &mut BTreeSet<String>,
            stack: &mut Vec<String>,
        ) -> Option<Vec<String>> {
            stack.push(current.to_string());
            in_stack.insert(current.to_string());

            if let Some(deps) = graph.get(current) {
                // Traverse neighbors sorted by canonical logical ID for determinism
                let mut sorted_deps: Vec<_> = deps.iter()
                    .filter(|d| blocked_nodes.contains(*d))
                    .collect();
                sorted_deps.sort();

                for dep in sorted_deps {
                    if in_stack.contains(dep) {
                        let start_idx = stack.iter().position(|s| s == dep).unwrap_or(0);
                        let mut cycle: Vec<String> = stack[start_idx..].to_vec();
                        cycle.push(dep.clone());
                        return Some(cycle);
                    }
                    if !visited.contains(dep) {
                        if let Some(cycle) = dfs(dep, graph, blocked_nodes, visited, in_stack, stack) {
                            return Some(cycle);
                        }
                    }
                }
            }

            in_stack.remove(current);
            stack.pop();
            visited.insert(current.to_string());
            None
        }

        for start_node in blocked_nodes {
            if !visited.contains(start_node) {
                if let Some(cycle) = dfs(start_node, graph, blocked_nodes, &mut visited, &mut in_stack, &mut stack) {
                    return cycle;
                }
            }
        }

        blocked_nodes.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_order_simple_tie_break() {
        // Independent nodes: order must follow lexicographical canonical logical ID (A, B, C)
        let mut graph = BTreeMap::new();
        graph.insert("C".to_string(), vec![]);
        graph.insert("A".to_string(), vec![]);
        graph.insert("B".to_string(), vec![]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph)
            .expect("Should not have cycle");

        assert_eq!(result, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_deterministic_order_chain() {
        // C depends on B, B depends on A
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec![]);
        graph.insert("B".to_string(), vec!["A".to_string()]);
        graph.insert("C".to_string(), vec!["B".to_string()]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph)
            .expect("Should not have cycle");

        assert_eq!(result, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_deterministic_order_diamond() {
        // Diamond: B -> A, C -> A, D -> B, C
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec![]);
        graph.insert("B".to_string(), vec!["A".to_string()]);
        graph.insert("C".to_string(), vec!["A".to_string()]);
        graph.insert("D".to_string(), vec!["B".to_string(), "C".to_string()]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph)
            .expect("Should not have cycle");

        assert_eq!(result, vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn test_cycle_detection_self() {
        // A -> A
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec!["A".to_string()]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, "Dependency cycle detected: A -> A");
    }

    #[test]
    fn test_cycle_detection_direct() {
        // A -> B -> A
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec!["B".to_string()]);
        graph.insert("B".to_string(), vec!["A".to_string()]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, "Dependency cycle detected: A -> B -> A");
    }

    #[test]
    fn test_cycle_detection_transitive() {
        // A -> B -> C -> A
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec!["B".to_string()]);
        graph.insert("B".to_string(), vec!["C".to_string()]);
        graph.insert("C".to_string(), vec!["A".to_string()]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err, "Dependency cycle detected: A -> B -> C -> A");
    }

    #[test]
    fn test_cycle_with_downstream_blocked_node_excluded() {
        // C depends on A, but A <-> B is the cycle.
        // C is blocked by the cycle, but C is NOT in the cycle!
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec!["B".to_string()]);
        graph.insert("B".to_string(), vec!["A".to_string()]);
        graph.insert("C".to_string(), vec!["A".to_string()]);

        let result = SysrootBuilder::topological_sort_with_stable_order(&graph);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The cycle diagnostic must isolate the actual cycle, excluding C!
        assert!(
            err.contains("A -> B -> A") || err.contains("B -> A -> B"),
            "Expected cycle between A and B, got: {}",
            err
        );
        assert!(!err.contains("C"), "Downstream node C must not be in cycle diagnostic! Got: {}", err);
    }

    #[test]
    fn test_topo_order_repeatability() {
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), vec![]);
        graph.insert("B".to_string(), vec![]);
        graph.insert("C".to_string(), vec![]);
        graph.insert("D".to_string(), vec!["B".to_string()]);
        graph.insert("E".to_string(), vec!["A".to_string(), "C".to_string()]);

        let results: Vec<Vec<String>> = (0..5)
            .map(|_| {
                SysrootBuilder::topological_sort_with_stable_order(&graph)
                    .expect("Should not have cycle")
            })
            .collect();

        for i in 1..results.len() {
            assert_eq!(results[i], results[0], "Order must be completely repeatable");
        }
    }

    #[test]
    fn test_lock_guard_cleanup_on_drop() {
        let temp_dir = std::env::temp_dir().join("luna_lock_test_drop");
        let lock_path = temp_dir.join(".build.lock");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        {
            let guard = BuildLockGuard::acquire(&lock_path);
            assert!(guard.is_ok());
            assert!(lock_path.exists());
        }

        assert!(!lock_path.exists(), "Lock must be deleted when guard drops");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lock_guard_cleanup_on_error() {
        let temp_dir = std::env::temp_dir().join("luna_lock_test_err");
        let lock_path = temp_dir.join(".build.lock");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        fn return_early(path: &Path) -> Result<(), String> {
            let _guard = BuildLockGuard::acquire(path)?;
            Err("Simulated error".to_string())
        }

        let err = return_early(&lock_path);
        assert!(err.is_err());
        assert!(!lock_path.exists(), "Lock must be cleaned up after error exit");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lock_guard_mutual_exclusion() {
        let temp_dir = std::env::temp_dir().join("luna_lock_test_mutex");
        let lock_path = temp_dir.join(".build.lock");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let guard1 = BuildLockGuard::acquire(&lock_path);
        assert!(guard1.is_ok());

        // Second acquire while lock held must fail
        let guard2 = BuildLockGuard::acquire(&lock_path);
        assert!(guard2.is_err());

        // Lock file must still exist and be intact
        assert!(lock_path.exists());

        drop(guard1);
        // After guard1 drops, lock file is removed
        assert!(!lock_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

