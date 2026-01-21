//! llama.cpp backend implementation
//!
//! This module encapsulates all llama.cpp-specific logic.

use crate::llm::{LLMBackend, LLMInput, LLMOutput};
use anyhow::{Context, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend as LlamaCppLlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use std::fs::OpenOptions;
use std::num::NonZeroU32;
use std::os::fd::AsRawFd;
use std::path::Path;

/// llama.cpp backend implementation
///
/// This struct encapsulates llama.cpp state and provides a safe interface.
/// We use Box to store the dependencies to ensure stable addresses.
pub struct LlamaCppBackend {
    // Boxed to ensure stable memory addresses
    _backend: Box<LlamaCppLlamaBackend>,
    model: Box<LlamaModel>,
    // Store context as raw pointer with manual lifetime management
    context: *mut llama_cpp_2::context::LlamaContext<'static>,
}

impl LlamaCppBackend {
    /// Initialize a new llama.cpp backend from a GGUF model file
    pub fn new(model_path: &Path) -> Result<Self> {
        // Initialize llama.cpp backend (must be kept alive)
        let backend = Box::new(LlamaCppLlamaBackend::init()?);

        // Load model
        let model_params = LlamaModelParams::default();
        let model = Box::new(
            LlamaModel::load_from_file(&backend, model_path, &model_params)
                .context("Failed to load model")?,
        );

        // Create context - it borrows from model
        let ctx_params = LlamaContextParams::default().with_n_ctx(NonZeroU32::new(2048));

        let context = model
            .new_context(&backend, ctx_params)
            .context("Failed to create context")?;

        // SAFETY: We box the model and backend to ensure stable addresses.
        // The context pointer remains valid as long as model and backend are alive.
        // We manually manage the context lifetime via Drop.
        let context_ptr = Box::into_raw(Box::new(unsafe {
            std::mem::transmute::<_, llama_cpp_2::context::LlamaContext<'static>>(context)
        }));

        Ok(Self {
            _backend: backend,
            model,
            context: context_ptr,
        })
    }
}

impl Drop for LlamaCppBackend {
    fn drop(&mut self) {
        // SAFETY: We created this pointer in new() and haven't dropped it yet
        unsafe {
            if !self.context.is_null() {
                let _ = Box::from_raw(self.context);
            }
        }
    }
}

impl LLMBackend for LlamaCppBackend {
    fn infer(&mut self, input: LLMInput) -> Result<LLMOutput> {
        // SAFETY: context pointer is valid for the lifetime of Self
        let context = unsafe { self.context.as_mut().context("Context pointer is null")? };

        // Suppress stderr during first decode (Metal shader compilation logs)
        let _stderr_redirect = if input.first_generation {
            Some(suppress_stderr_temporarily())
        } else {
            None
        };

        // Tokenize prompt
        let tokens = self
            .model
            .str_to_token(&input.prompt, AddBos::Always)
            .context("Failed to tokenize prompt")?;

        // Create batch with size based on prompt length + generation headroom
        let batch_size = (tokens.len() + input.max_tokens).max(512);
        let mut batch = LlamaBatch::new(batch_size, 1);
        for (i, token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch.add(*token, input.current_pos + i as i32, &[0], is_last)?;
        }

        // Decode the prompt
        context
            .decode(&mut batch)
            .context("Failed to decode batch")?;

        // Generate tokens
        let mut result = String::new();
        let mut n_generated = 0;
        let prompt_len = tokens.len() as i32;

        while n_generated < input.max_tokens {
            // Get token candidates and sample greedily
            let candidates = context.candidates();
            let mut candidates_array = LlamaTokenDataArray::from_iter(candidates, false);

            // Select token with highest probability (greedy sampling)
            candidates_array.sample_token_greedy();
            let token = match candidates_array.selected_token() {
                Some(t) => t,
                None => break, // No token selected, end generation
            };

            // Check for EOS
            if self.model.is_eog_token(token) {
                break;
            }

            // Decode token
            if let Ok(piece) = self.model.token_to_str(token, Special::Tokenize) {
                result.push_str(&piece);
            }

            // Prepare next batch
            batch.clear();
            batch.add(
                token,
                input.current_pos + prompt_len + n_generated as i32,
                &[0],
                true,
            )?;

            context
                .decode(&mut batch)
                .context("Failed to decode batch")?;

            n_generated += 1;

            // Early stopping heuristics
            if result.trim().starts_with('{') {
                // For JSON tool calls: stop when we have valid complete JSON
                if result.contains('}') {
                    if serde_json::from_str::<serde_json::Value>(result.trim()).is_ok() {
                        break;
                    }
                }
            } else {
                // For text responses: stop when we see natural ending patterns
                // Check for double newline after sentence (paragraph break)
                if result.contains("\n\n")
                    && (result.trim_end().ends_with('.')
                        || result.trim_end().ends_with('!')
                        || result.trim_end().ends_with('?'))
                {
                    break;
                }
            }
        }

        // Return generated text and total tokens processed (prompt + generated)
        Ok(LLMOutput {
            text: result.trim().to_string(),
            tokens_processed: prompt_len + n_generated as i32,
        })
    }
}

/// Temporarily suppress stderr (for Metal shader compilation logs)
fn suppress_stderr_temporarily() -> impl Drop {
    struct StderrRedirect {
        old_stderr: i32,
    }

    impl Drop for StderrRedirect {
        fn drop(&mut self) {
            unsafe {
                libc::dup2(self.old_stderr, 2);
                libc::close(self.old_stderr);
            }
        }
    }

    unsafe {
        let old_stderr = libc::dup(2);
        let devnull = OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("Failed to open /dev/null");
        libc::dup2(devnull.as_raw_fd(), 2);

        StderrRedirect { old_stderr }
    }
}
