use clap::Parser;
use std::collections::HashMap;
use std::process;
use tracing::{error, info};
use updog::{
    cli::{Cli, Commands},
    Config, PackageManager,
};

// Execution result tracking struct
struct ExecutionResult {
    success: bool,
    message: String,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(level).init();

    // Load configuration
    let config_path = cli.get_config_path();
    let config = match Config::from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load config from {:?}: {}", config_path, e);
            info!("Using default configuration");
            Config::default()
        }
    };

    // Check if dry run mode is enabled
    let is_dry_run = cli.command.is_dry_run();
    if is_dry_run {
        info!("Dry run mode - no changes will be made");
    }

    let pm = PackageManager::with_dry_run(config, is_dry_run);

    match cli.command {
        Commands::Check {
            package_manager, ..
        } => {
            let managers = if let Some(name) = package_manager {
                vec![name]
            } else {
                pm.config.commands.keys().cloned().collect()
            };

            let mut has_error = false;
            let mut results = HashMap::new();

            for name in managers {
                info!("Checking updates for {}", name);
                match pm.check(&name) {
                    Ok(_) => {
                        results.insert(
                            name.clone(),
                            ExecutionResult {
                                success: true,
                                message: "Successfully checked for updates".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        error!("{}: {}", name, e);
                        results.insert(
                            name.clone(),
                            ExecutionResult {
                                success: false,
                                message: format!("Error: {}", e),
                            },
                        );
                        has_error = true;
                    }
                }
            }

            // Print summary of execution result
            print_summary("Check", &results);

            if has_error {
                process::exit(1);
            }
        }

        Commands::Update {
            package_manager, ..
        } => {
            let managers = if let Some(name) = package_manager {
                vec![name]
            } else {
                pm.config.commands.keys().cloned().collect()
            };

            let mut has_error = false;
            let mut results = HashMap::new();

            for name in managers {
                info!("Updating {}", name);
                match pm.update(&name) {
                    Ok(_) => {
                        results.insert(
                            name.clone(),
                            ExecutionResult {
                                success: true,
                                message: "Successfully updated".to_string(),
                            },
                        );
                    }
                    Err(e) => {
                        error!("{}: {}", name, e);
                        results.insert(
                            name.clone(),
                            ExecutionResult {
                                success: false,
                                message: format!("Error: {}", e),
                            },
                        );
                        has_error = true;
                    }
                }
            }

            // Print summary of execution result
            print_summary("Update", &results);

            if has_error {
                process::exit(1);
            }
        }

        Commands::Tui => {
            info!("TUI mode not implemented yet");
        }
    }
}

// Print summary of execution result
fn print_summary(operation: &str, results: &HashMap<String, ExecutionResult>) {
    println!("\n{} Summary:", operation);
    println!("==============================================");

    let mut success_count = 0;
    let mut failure_count = 0;

    // Calculate successful items
    for (_, result) in results.iter() {
        if result.success {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }

    // Print successful items if there are any
    if success_count > 0 {
        println!("✅ Successful:");
        for (name, result) in results.iter() {
            if result.success {
                println!("  - {}: {}", name, result.message);
            }
        }
    }

    // Print failed items if there are any
    if failure_count > 0 {
        if success_count > 0 {
            println!(); // Add empty line between success and failure items
        }
        println!("❌ Failed:");
        for (name, result) in results.iter() {
            if !result.success {
                println!("  - {}: {}", name, result.message);
            }
        }
    }

    // Print statistics
    println!(
        "\nStats: {} total, {} succeeded, {} failed",
        results.len(),
        success_count,
        failure_count
    );
    println!("==============================================");
}
