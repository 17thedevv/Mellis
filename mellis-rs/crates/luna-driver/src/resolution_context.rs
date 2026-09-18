#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderResolutionContext {
    UserImport,
    SysrootDependency,
    Bootstrap,
    SysrootBuild,
}

impl ProviderResolutionContext {
    pub fn can_access_internal(&self) -> bool {
        matches!(
            self,
            Self::SysrootDependency | Self::Bootstrap | Self::SysrootBuild
        )
    }
}
