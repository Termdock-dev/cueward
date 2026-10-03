use super::super::{TrashPlan, TrashTargetKind};
use super::*;

pub(super) struct Context<'a, P: TrashPlatform> {
    pub platform: &'a P,
    pub plan: TrashPlan,
    pub root: File,
    pub source: File,
    pub parent: File,
    pub trash: File,
    pub trash_info: FileInfo,
}
impl<'a, P: TrashPlatform> Context<'a, P> {
    pub(super) fn prepare(platform: &'a P, receipt: &mut TrashReceipt) -> Result<Self, FileError> {
        let plan = super::super::plan(platform, &receipt.request.plan_request())?;
        supported(&plan, &receipt.request)?;
        let root = platform.open_directory(Path::new(&plan.root.path))?;
        let parent = platform.open_directory(Path::new(&plan.source_parent.path))?;
        let source = platform.open_object(&plan.source)?;
        platform.validate_copy_source(&source, &plan.source)?;
        let trash_path = platform.trash_directory(&plan.source)?;
        let trash = platform.open_directory(&trash_path)?;
        platform.validate_trash(&root, &source, &trash)?;
        let scope = Scope::new(platform, &trash_path)?;
        let trash_info = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
        if Path::new(&trash_info.path) != trash_path
            || Path::new(&plan.root.path).starts_with(&trash_path)
            || Path::new(&plan.source.path).starts_with(&trash_path)
        {
            return Err(changed(
                "Trash mapping changed or source root is already within Trash",
            ));
        }
        let context = Self {
            platform,
            plan,
            root,
            source,
            parent,
            trash,
            trash_info,
        };
        context.record(receipt)?;
        context.revalidate(receipt)?;
        Ok(context)
    }
    fn record(&self, receipt: &mut TrashReceipt) -> Result<(), FileError> {
        receipt.root_before = Some(self.plan.root.clone());
        receipt.source_before = Some(self.plan.source.clone());
        receipt.parent_before = Some(self.plan.source_parent.clone());
        receipt.trash_before = Some(self.trash_info.clone());
        let size = serde_json::to_vec_pretty(receipt)
            .map_err(json_error)?
            .len();
        if size > 64 * 1024 {
            let error = FileError::new(
                FileErrorCode::ScanLimit,
                "trash preflight evidence exceeds 64 KiB safety budget",
            );
            receipt.root_before = None;
            receipt.source_before = None;
            receipt.parent_before = None;
            receipt.trash_before = None;
            receipt.trash_path = None;
            receipt.preflight_observations_omitted = true;
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn revalidate(&self, receipt: &TrashReceipt) -> Result<(), FileError> {
        let fresh = super::super::plan(self.platform, &receipt.request.plan_request())?;
        let mut effective = receipt.request.clone();
        effective.expected_parent_version = self.plan.source_parent.version.clone();
        supported(&fresh, &effective)?;
        if fresh.root.path != self.plan.root.path
            || fresh.root.version != self.plan.root.version
            || fresh.source.path != self.plan.source.path
            || fresh.source.version != self.plan.source.version
            || fresh.source_parent.path != self.plan.source_parent.path
            || fresh.source_parent.version != self.plan.source_parent.version
            || self.platform.stamp(&self.root.metadata()?).version != self.plan.root.version
            || self.platform.stamp(&self.source.metadata()?).version != self.plan.source.version
        {
            return Err(changed(
                "source/root/parent path or held revision changed before trash",
            ));
        }
        self.check_trash(true)
    }
    pub(super) fn check_trash(&self, before: bool) -> Result<(), FileError> {
        let scope = Scope::new(self.platform, Path::new(&self.trash_info.path))?;
        let now = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
        let held = self.platform.stamp(&self.trash.metadata()?);
        if now.path != self.trash_info.path
            || now.identity != self.trash_info.identity
            || now.version != held.version
            || (before && now.version != self.trash_info.version)
        {
            return Err(changed("Trash directory namespace or descriptor changed"));
        }
        self.platform
            .validate_trash(&self.root, &self.source, &self.trash)?;
        scope.revalidate_root()
    }
}
fn supported(plan: &TrashPlan, request: &TrashRequest) -> Result<(), FileError> {
    if !matches!(
        plan.target_kind,
        TrashTargetKind::RegularFile
            | TrashTargetKind::Directory
            | TrashTargetKind::PackageDirectory
            | TrashTargetKind::Symlink
            | TrashTargetKind::FinderAlias
    ) || plan.root.data_state != DataState::NotDataless
        || plan.source_parent.data_state != DataState::NotDataless
        || plan.source.data_state != DataState::NotDataless
    {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "trash execution requires a known available file, directory, package, alias or symlink object",
        ));
    }
    check_version(
        &Some(request.expected_parent_version.clone()),
        &plan.source_parent.version,
    )?;
    if plan.source.kind != FileKind::Directory && plan.source.size > request.max_bytes {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "source exceeds backup max-bytes",
        ));
    }
    Ok(())
}
