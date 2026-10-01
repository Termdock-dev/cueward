use super::*;
use cueward_adapter_macos::files::preview::{
    PreviewKind, PreviewOptions, PreviewRequest, execute_worker, run,
};

#[derive(Args)]
pub(crate) struct PreviewScope {
    #[arg(long)]
    root: PathBuf,
    /// Explicit file path relative to root.
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    follow_links: bool,
    #[arg(long)]
    expected_version: Option<String>,
    /// Includes snapshot, Swift startup, extraction and preview generation.
    #[arg(long, default_value_t = 10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}

#[derive(Args)]
pub(crate) struct BudgetArgs {
    /// Maximum source size copied into a private local snapshot.
    #[arg(long, default_value_t = 67108864, value_parser=clap::value_parser!(u64).range(1..=268435456))]
    max_input_bytes: u64,
    /// Aggregate returned UTF-8 text limit across selected pages.
    #[arg(long, default_value_t = 65536, value_parser=clap::value_parser!(u64).range(1..=1048576))]
    max_text_bytes: u64,
    /// Maximum width and height of rendered/OCR images.
    #[arg(long, default_value_t = 1024, value_parser=clap::value_parser!(u64).range(1..=2048))]
    max_dimension: u64,
    /// Aggregate accepted PNG byte limit.
    #[arg(long, default_value_t = 8388608, value_parser=clap::value_parser!(u64).range(1..=33554432))]
    max_preview_bytes: u64,
}

#[derive(Subcommand)]
pub(crate) enum PreviewCommand {
    /// Extract native PDF text and selected page images; OCR fallback is explicit.
    Pdf {
        #[command(flatten)]
        scope: PreviewScope,
        #[command(flatten)]
        budget: BudgetArgs,
        #[arg(long, default_value_t = 1, value_parser=clap::value_parser!(u64).range(1..=1000000))]
        start_page: u64,
        #[arg(long, default_value_t = 10, value_parser=clap::value_parser!(u64).range(1..=20))]
        page_count: u64,
        #[arg(long)]
        ocr: bool,
        #[arg(long)]
        no_render: bool,
    },
    /// Read ImageIO information and preview the first frame; OCR is explicit.
    Image {
        #[command(flatten)]
        scope: PreviewScope,
        #[command(flatten)]
        budget: BudgetArgs,
        #[arg(long)]
        ocr: bool,
        #[arg(long)]
        no_render: bool,
    },
    /// Request a Quick Look content thumbnail, without text extraction or visible UI.
    Thumbnail {
        #[command(flatten)]
        scope: PreviewScope,
        #[command(flatten)]
        budget: BudgetArgs,
    },
}

impl BudgetArgs {
    fn options(
        self,
        kind: PreviewKind,
        range: (usize, usize),
        ocr: bool,
        render: bool,
    ) -> PreviewOptions {
        PreviewOptions {
            kind,
            start_page: range.0,
            page_count: range.1,
            ocr,
            render,
            max_input_bytes: self.max_input_bytes,
            max_text_bytes: self.max_text_bytes as usize,
            max_dimension: self.max_dimension as usize,
            max_preview_bytes: self.max_preview_bytes,
        }
    }
}
impl PreviewCommand {
    fn request(self) -> (PreviewRequest, u64) {
        let (scope, options) = match self {
            Self::Pdf {
                scope,
                budget,
                start_page,
                page_count,
                ocr,
                no_render,
            } => (
                scope,
                budget.options(
                    PreviewKind::Pdf,
                    (start_page as usize, page_count as usize),
                    ocr,
                    !no_render,
                ),
            ),
            Self::Image {
                scope,
                budget,
                ocr,
                no_render,
            } => (
                scope,
                budget.options(PreviewKind::Image, (1, 1), ocr, !no_render),
            ),
            Self::Thumbnail { scope, budget } => (
                scope,
                budget.options(PreviewKind::Thumbnail, (1, 1), false, true),
            ),
        };
        (
            PreviewRequest {
                root: scope.root,
                path: scope.path,
                follow_links: scope.follow_links,
                expected_version: scope.expected_version,
                options,
            },
            scope.timeout_ms,
        )
    }
}

/// Dispatch through the existing bounded files worker protocol.
pub(super) fn dispatch(action: PreviewCommand) {
    let (request, timeout) = action.request();
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| run(&exe, &request, timeout));
    output("files", result);
}
/// Decode one private operation request in the dedicated worker process.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|request| execute_worker(&request)),
    );
}
#[cfg(test)]
#[path = "files_preview_tests.rs"]
mod tests;
