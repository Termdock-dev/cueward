use clap::{Subcommand, ValueEnum};

mod chatgpt;
mod gemini;
mod grok;
mod threads;
mod x;

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum SafariAiProvider {
    Gemini,
    Chatgpt,
    Grok,
    Threads,
    X,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum GeminiMode {
    Image,
    DeepResearch,
    Video,
    Music,
}

#[derive(Subcommand)]
pub(crate) enum SafariAiAction {
    /// Send a prompt to the AI provider
    Prompt {
        /// Prompt text
        #[arg(long)]
        prompt: String,
        /// Optional mode (e.g. deep-research, image, video, music)
        #[arg(long)]
        mode: Option<GeminiMode>,
        /// Automatically confirm (e.g. Deep Research plan)
        #[arg(long, default_value_t = false)]
        auto_confirm: bool,
    },
    /// Switch to a specific mode without sending a prompt
    Mode {
        /// Mode to switch into
        mode: GeminiMode,
    },
    /// List conversations from the sidebar
    List,
    /// Read a conversation's text content by URL
    Read {
        /// Conversation URL
        url: String,
    },
    /// Poll an in-progress workflow (e.g. Deep Research)
    Poll {
        /// Timeout in seconds
        #[arg(long, default_value = "900")]
        timeout: u64,
    },
    /// Save AI-generated images as PNG files
    SaveImages {
        /// Conversation URL
        url: String,
        /// Output directory
        #[arg(long, default_value = ".")]
        output: String,
    },
    /// Download media (video/music) via browser
    SaveMedia {
        /// Conversation URL
        url: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GeminiAiAction {
    ModeOnly(GeminiMode),
    PromptOnly(String),
    ModeThenPrompt(GeminiMode, String),
    DeepResearchPlan(String, bool),
}

pub(crate) fn build_gemini_ai_action(
    mode: Option<GeminiMode>,
    prompt: Option<&str>,
    auto_confirm: bool,
) -> Result<GeminiAiAction, &'static str> {
    if auto_confirm && !matches!((&mode, prompt), (Some(GeminiMode::DeepResearch), Some(_))) {
        return Err("--auto-confirm requires --mode deep-research and --prompt");
    }

    match (mode, prompt) {
        (Some(GeminiMode::DeepResearch), Some(prompt)) => {
            Ok(GeminiAiAction::DeepResearchPlan(prompt.to_string(), auto_confirm))
        }
        (Some(mode), Some(prompt)) => Ok(GeminiAiAction::ModeThenPrompt(mode, prompt.to_string())),
        (Some(mode), None) => Ok(GeminiAiAction::ModeOnly(mode)),
        (None, Some(prompt)) => Ok(GeminiAiAction::PromptOnly(prompt.to_string())),
        (None, None) => Err("--mode or --prompt is required for Gemini Safari AI workflow"),
    }
}

pub(crate) fn dispatch(provider: SafariAiProvider, profile: Option<String>, action: SafariAiAction) {
    let p = profile.as_deref();
    match provider {
        SafariAiProvider::Gemini => gemini::dispatch(action, p),
        SafariAiProvider::Chatgpt => chatgpt::dispatch(action, p),
        SafariAiProvider::Grok => grok::dispatch(action, p),
        SafariAiProvider::Threads => threads::dispatch(action, p),
        SafariAiProvider::X => x::dispatch(action, p),
    }
}
