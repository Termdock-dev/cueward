use super::*;

pub(super) struct Context<'a, P: TrashPlatform> {
    pub platform: &'a P,
    pub original: &'a TrashReceipt,
    pub root: File,
    pub root_info: FileInfo,
    pub parent: File,
    pub parent_info: FileInfo,
    pub backup: File,
    pub backup_info: FileInfo,
    pub destination: PathBuf,
}
impl<'a, P: TrashPlatform> Context<'a, P> {
    /// Hold and check original scope, fresh parent and retained backup without writing.
    pub(super) fn prepare(
        platform: &'a P,
        receipt: &mut RestoreReceipt,
        original: &'a TrashReceipt,
    ) -> Result<Self, FileError> {
        let evidence = evidence::validate(&receipt.request, original)?;
        let scope = Scope::new(platform, &receipt.request.root)?;
        let root_info = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
        if root_info.path != evidence.root.path
            || root_info.identity != evidence.root.identity
            || root_info.data_state != DataState::NotDataless
        {
            return Err(changed(
                "original selected root moved, replaced or unavailable",
            ));
        }
        let destination = original.request.path.clone();
        let parent_info = original_parent(&scope, receipt, &destination, evidence.parent)?;
        let root = platform.open_directory(Path::new(&root_info.path))?;
        let parent = platform.open_directory(Path::new(&parent_info.path))?;
        let backup = platform.open_object(evidence.backup)?;
        platform.validate_copy_source(&backup, evidence.backup)?;
        let context = Self {
            platform,
            original,
            root,
            root_info,
            parent,
            parent_info,
            backup,
            backup_info: evidence.backup.clone(),
            destination,
        };
        context.revalidate(receipt, false)?;
        context.record(receipt, evidence.source)?;
        Ok(context)
    }
    fn record(&self, receipt: &mut RestoreReceipt, source: &FileInfo) -> Result<(), FileError> {
        receipt.trash_receipt_sha256 = Some(fingerprint(self.original)?);
        receipt.destination = Some(self.destination.clone());
        receipt.original_source = Some(source.clone());
        receipt.root_before = Some(self.root_info.clone());
        receipt.parent_before = Some(self.parent_info.clone());
        receipt.backup_before = Some(self.backup_info.clone());
        if let Err(error) = bound(receipt, 64 * 1024) {
            receipt.original_source = None;
            receipt.root_before = None;
            receipt.parent_before = None;
            receipt.backup_before = None;
            receipt.preflight_observations_omitted = true;
            return Err(error);
        }
        Ok(())
    }
    /// Return the saved full backup verification, never derive proof from current bytes.
    pub(super) fn proof(&self) -> Result<&CopyVerification, FileError> {
        self.original
            .backup_verification
            .as_ref()
            .ok_or_else(|| invalid("backup proof missing"))
    }
    /// Detect namespace/descriptor changes and conflicts without following links.
    pub(super) fn revalidate(
        &self,
        receipt: &RestoreReceipt,
        published: bool,
    ) -> Result<(FileInfo, FileInfo), FileError> {
        let scope = Scope::new(self.platform, &receipt.request.root)?;
        let root = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
        let parent_path = self
            .destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = scope.info(&scope.resolve(parent_path, false, false)?)?;
        if root.path != self.root_info.path
            || root.identity != self.root_info.identity
            || root.version != self.platform.stamp(&self.root.metadata()?).version
            || parent.path != self.parent_info.path
            || parent.identity != self.parent_info.identity
            || parent.version != self.platform.stamp(&self.parent.metadata()?).version
            || (!published
                && (root.version != self.root_info.version
                    || parent.version != self.parent_info.version))
        {
            return Err(changed(
                "restore root/parent path or held descriptor changed",
            ));
        }
        if !published {
            require_absent(&scope, &self.destination)?;
        }
        self.check_backup()?;
        scope.revalidate_root()?;
        Ok((root, parent))
    }
    /// Read back retained backup and match its original identity, revision and proof.
    pub(super) fn check_backup(&self) -> Result<FileInfo, FileError> {
        let now = observe(self.platform, Path::new(&self.backup_info.path))?;
        if now.path != self.backup_info.path
            || now.version != self.backup_info.version
            || self.platform.stamp(&self.backup.metadata()?).version != now.version
        {
            return Err(changed("saved backup path or descriptor changed"));
        }
        verify_file(self.platform, &now, self.proof()?)?;
        Ok(now)
    }
}
fn require_absent(scope: &Scope<'_, impl FilePlatform>, path: &Path) -> Result<(), FileError> {
    match scope.resolve(path, false, true) {
        Err(e) if e.code == FileErrorCode::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) => Err(FileError::new(
            FileErrorCode::Conflict,
            "original path already occupied; nothing overwritten",
        )),
    }
}

fn original_parent(
    scope: &Scope<'_, impl FilePlatform>,
    receipt: &RestoreReceipt,
    destination: &Path,
    original: &FileInfo,
) -> Result<FileInfo, FileError> {
    let path = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = scope.info(&scope.resolve(path, false, false)?)?;
    check_version(
        &Some(receipt.request.expected_parent_version.clone()),
        &parent.version,
    )?;
    if parent.path != original.path
        || parent.identity != original.identity
        || parent.data_state != DataState::NotDataless
    {
        return Err(changed("original parent moved, replaced or unavailable"));
    }
    Ok(parent)
}
