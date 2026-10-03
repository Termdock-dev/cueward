use super::super::CopyTreeIssue;
use super::*;

pub(super) struct Context<'a, P: TreeExecutionPlatform> {
    pub platform: &'a P,
    pub plan: CopyTreePlan,
    pub root: File,
    pub parent: File,
    pub sources: Vec<File>,
    pub attributes: Vec<(String, usize)>,
}
impl<'a, P: TreeExecutionPlatform> Context<'a, P> {
    pub(super) fn prepare(platform: &'a P, receipt: &mut TreeReceipt) -> Result<Self, FileError> {
        let plan = super::super::plan(platform, &receipt.request)?;
        require_supported(&plan, false)?;
        let root = super::super::traversal::open(platform, &plan.root)?;
        let parent = super::super::traversal::open(platform, &plan.destination_parent)?;
        let Sources {
            files: sources,
            attributes,
        } = open_sources(platform, &plan)?;
        record_preflight(receipt, &plan, &attributes)?;
        let context = Self {
            platform,
            plan,
            root,
            parent,
            sources,
            attributes,
        };
        context.revalidate(receipt, false)?;
        Ok(context)
    }
    pub(super) fn revalidate(
        &self,
        receipt: &TreeReceipt,
        published: bool,
    ) -> Result<(), FileError> {
        if receipt.request.root.canonicalize()? != Path::new(&self.plan.root.path) {
            return Err(changed("supplied root mapping changed"));
        }
        let mut request = receipt.request.clone();
        request.root = self.plan.root.path.clone().into();
        request.expected_parent_version = if published {
            self.platform.stamp(&self.parent.metadata()?).version
        } else {
            self.plan.destination_parent.version.clone()
        };
        let fresh = super::super::plan(self.platform, &request)?;
        require_supported(&fresh, published)?;
        if fresh.root.path != self.plan.root.path
            || fresh.root.identity != self.plan.root.identity
            || fresh.root.version != self.platform.stamp(&self.root.metadata()?).version
            || (!published && fresh.root.version != self.plan.root.version)
            || fresh.destination_parent.identity != self.plan.destination_parent.identity
            || fresh.destination_parent.path != self.plan.destination_parent.path
            || fresh.destination_parent.version
                != self.platform.stamp(&self.parent.metadata()?).version
            || fresh.entries.len() != self.plan.entries.len()
        {
            return Err(changed("tree/root/parent changed"));
        }
        self.check_sources(&fresh)?;
        if receipt.request.root.canonicalize()? != Path::new(&self.plan.root.path) {
            return Err(changed("supplied root mapping changed during preflight"));
        }
        Ok(())
    }
    fn check_sources(&self, fresh: &CopyTreePlan) -> Result<(), FileError> {
        for (index, (before, after)) in self.plan.entries.iter().zip(&fresh.entries).enumerate() {
            if before.relative_path != after.relative_path
                || before.source.path != after.source.path
                || before.source.version != after.source.version
                || self
                    .platform
                    .stamp(&self.sources[index].metadata()?)
                    .version
                    != before.source.version
                || self.platform.tree_attribute_digest(&self.sources[index])?
                    != self.attributes[index]
            {
                return Err(changed("source tree revision or attributes changed"));
            }
        }
        Ok(())
    }
    pub(super) fn check_node(&self, receipt: &TreeReceipt, index: usize) -> Result<(), FileError> {
        if receipt.request.root.canonicalize()? != Path::new(&self.plan.root.path) {
            return Err(changed("supplied root mapping changed before staged write"));
        }
        let scope = self.scope()?;
        if self.platform.stamp(&self.root.metadata()?).version != self.plan.root.version
            || self.platform.stamp(&self.parent.metadata()?).version
                != self.plan.destination_parent.version
        {
            return Err(changed(
                "root or destination parent changed before staged write",
            ));
        }
        let before = &self.plan.entries[index].source;
        let path = Path::new(&before.path)
            .strip_prefix(&scope.root)
            .map_err(|_| changed("source escaped root"))?;
        let now = scope.info(&scope.resolve(path, false, true)?)?;
        if now.version != before.version
            || now.path != before.path
            || self
                .platform
                .stamp(&self.sources[index].metadata()?)
                .version
                != before.version
        {
            return Err(changed(
                "source path/descriptor changed before staged write",
            ));
        }
        scope.revalidate_root()
    }
    pub(super) fn scope(&self) -> Result<Scope<'_, P>, FileError> {
        let scope = Scope::new(self.platform, Path::new(&self.plan.root.path))?;
        let root = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
        if root.identity != self.plan.root.identity
            || root.version != self.platform.stamp(&self.root.metadata()?).version
        {
            return Err(changed("held root no longer matches namespace"));
        }
        Ok(scope)
    }
}
struct Sources {
    files: Vec<File>,
    attributes: Vec<(String, usize)>,
}
fn open_sources(
    platform: &impl TreeExecutionPlatform,
    plan: &CopyTreePlan,
) -> Result<Sources, FileError> {
    let mut sources = Vec::new();
    let mut attributes = Vec::new();
    let mut total = 0usize;
    for entry in &plan.entries {
        let file = super::super::traversal::open(platform, &entry.source)?;
        let digest = platform.tree_attribute_digest(&file)?;
        if digest.0.len() != 64 || !digest.0.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(FileError::new(
                FileErrorCode::Internal,
                "invalid source attribute digest",
            ));
        }
        total = total.checked_add(digest.1).ok_or_else(attribute_limit)?;
        if total > MAX_TREE_ATTRIBUTE_BYTES {
            return Err(attribute_limit());
        }
        sources.push(file);
        attributes.push(digest);
    }
    Ok(Sources {
        files: sources,
        attributes,
    })
}
fn record_preflight(
    receipt: &mut TreeReceipt,
    plan: &CopyTreePlan,
    attributes: &[(String, usize)],
) -> Result<(), FileError> {
    receipt.root_before = Some(plan.root.clone());
    receipt.parent_before = Some(plan.destination_parent.clone());
    receipt.source_before = plan.entries.first().map(|e| e.source.clone());
    for (entry, (digest, bytes)) in plan.entries.iter().zip(attributes) {
        receipt.nodes.push(TreeNode {
            relative_path: entry.relative_path.clone(),
            kind: entry.source.kind.clone(),
            source_identity: entry.source.identity.clone(),
            source_version: entry.source.version.clone(),
            source_bytes: entry.source.size,
            source_attributes_sha256: digest.clone(),
            source_attribute_bytes: *bytes,
            verification: None,
            destination_identity: None,
            destination_version: None,
        });
    }
    if let Err(error) = super::verification::bound_initial(receipt) {
        receipt.preflight_nodes_omitted = receipt.nodes.len();
        receipt.nodes.clear();
        receipt.root_before = None;
        receipt.parent_before = None;
        receipt.source_before = None;
        return Err(error);
    }
    Ok(())
}
fn require_supported(plan: &CopyTreePlan, published: bool) -> Result<(), FileError> {
    let issues_ok = if published {
        plan.issues == [CopyTreeIssue::ExistingDestination]
    } else {
        plan.issues.is_empty()
    };
    if !issues_ok || !plan.enumeration_complete || plan.entries.iter().any(|e| e.error.is_some()) {
        return Err(FileError::new(
            FileErrorCode::Conflict,
            "tree has namespace/unsupported-source blockers; inspect a fresh read-only plan",
        ));
    }
    Ok(())
}
fn attribute_limit() -> FileError {
    FileError::new(
        FileErrorCode::ScanLimit,
        "tree xattrs exceed 8 MiB aggregate values",
    )
}
