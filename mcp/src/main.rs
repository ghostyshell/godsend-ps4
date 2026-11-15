//! `godsend-ps4-mcp` entry point.
//!
//! Default transport is stdio, which is what an MCP client spawns. `--http`
//! serves the same tools on loopback instead. The desktop app starts its own
//! HTTP instance, so this binary is only needed when a client insists on
//! spawning the server itself.

use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut http = false;
    let mut port: u16 = 8744;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--http" => http = true,
            "--stdio" => http = false,
            "--port" | "-p" => {
                if let Some(value) = args.get(index + 1).and_then(|v| v.parse().ok()) {
                    port = value;
                }
                index += 1;
            }
            "-h" | "--help" => {
                eprintln!(
                    "godsend-ps4-mcp {}\n\n  godsend-ps4-mcp            stdio transport\n  godsend-ps4-mcp --http [--port N]   loopback HTTP on 127.0.0.1\n\nThe desktop app already serves HTTP on its configured MCP port.",
                    godsend_core::VERSION
                );
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }

    if http {
        if let Err(err) = godsend_ps4_mcp::serve_http(port) {
            eprintln!("MCP HTTP failed on 127.0.0.1:{port}: {err}");
            std::process::exit(1);
        }
    } else if let Err(err) = godsend_ps4_mcp::run_stdio() {
        eprintln!("MCP stdio failed: {err}");
        std::process::exit(1);
    }
}
