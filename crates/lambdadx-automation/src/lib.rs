//! Transport-independent tools. The editor owns documents and executes requests.

#[cfg(all(
    any(feature = "mcp", feature = "agent"),
    not(any(target_os = "linux", target_os = "macos", target_os = "windows"))
))]
compile_error!("MCP and agent runtimes currently support desktop targets only");

pub mod bridge;
pub mod catalog;
pub mod contract;
pub mod proposal;
#[cfg(feature = "mcp")]
pub mod mcp;
#[cfg(feature = "agent")]
pub mod agent;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "mcp", feature = "agent"))]
pub mod runtime;
