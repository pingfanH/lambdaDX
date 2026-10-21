fn main() {
    if let Err(error) = lambdadx_automation::mcp::proxy::run() { eprintln!("lambdadx_mcp: {error}"); std::process::exit(1); }
}
