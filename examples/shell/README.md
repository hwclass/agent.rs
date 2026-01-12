# Shell Tool Example

This example demonstrates the **agent-native** CLI with the shell tool.

## Overview

The shell tool allows the agent to execute bash commands with human-in-the-loop approval. This is useful for local development, automation tasks, and system operations.

## Running the Example

**Prerequisites:**
- Rust 1.75+
- C/C++ compiler
- CMake
- A GGUF model file

**Build and run:**

```bash
# From the repository root
make setup

# Download a model if you haven't already
wget https://huggingface.co/ibm-granite/granite-4.0-micro-GGUF/resolve/main/granite-4.0-micro-Q8_0.gguf

# Run the demo
./target/release/agent-native \
  --model ./granite-4.0-micro-Q8_0.gguf \
  --query "List files and show disk usage"
```

## Example Session

```
=== agent.rs ===
Query: List files and show disk usage

→ shell: ls -la
  Execute? (y/n): y

total 48
drwxr-xr-x  11 user  staff   352 Jan  6 16:42 .
drwxr-xr-x  27 user  staff   864 Jan  6 15:30 ..
-rw-r--r--   1 user  staff  6148 Jan  6 15:30 .DS_Store
-rw-r--r--   1 user  staff 18520 Jan  6 15:31 Cargo.lock
-rw-r--r--   1 user  staff   254 Jan  6 15:30 Cargo.toml
-rw-r--r--   1 user  staff  9705 Jan  6 16:42 README.md
drwxr-xr-x   5 user  staff   160 Jan  6 15:30 crates
drwxr-xr-x   8 user  staff   256 Jan  6 16:52 target

OBSERVATIONS
- Directory contains 11 items
- Key files: Cargo.toml, README.md, Makefile
- Includes crates/ and target/ directories

FINAL ANSWER
The directory contains 11 items including project files and build artifacts.
```

## How It Works

1. **User Query** - You provide a natural language request
2. **Agent Decision** - The LLM decides to invoke the shell tool with a command
3. **Human Approval** - You're prompted to approve or reject the command
4. **Execution** - If approved, the command runs and output is fed back to the agent
5. **Final Answer** - The agent synthesizes observations into a final answer

## Safety

All shell commands require explicit approval before execution. Rejected commands return an error to the agent, allowing it to:
- Try a different approach
- Ask for clarification
- Provide a final answer without tool use

## Configuration

Adjust these CLI parameters as needed:

```bash
--model <PATH>           # Path to GGUF model
--query <STRING>         # User query
--max-iterations <N>     # Max agent loop iterations (default: 5)
--max-tokens <N>         # Tokens per generation (default: 256)
```
