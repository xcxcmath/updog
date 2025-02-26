use clap::Parser;
use std::process;
use tracing::{error, info};
use updog::{cli::{Cli, Commands}, Config, PackageManager};
use std::collections::HashMap;

// 실행 결과를 추적하기 위한 구조체
struct ExecutionResult {
    success: bool,
    message: String,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(level)
        .init();

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
        Commands::Check { package_manager, .. } => {
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
                        results.insert(name.clone(), ExecutionResult {
                            success: true,
                            message: "Successfully checked for updates".to_string(),
                        });
                    }
                    Err(e) => {
                        error!("{}: {}", name, e);
                        results.insert(name.clone(), ExecutionResult {
                            success: false,
                            message: format!("Error: {}", e),
                        });
                        has_error = true;
                    }
                }
            }
            
            // 실행 결과 요약 출력
            print_summary("Check", &results);

            if has_error {
                process::exit(1);
            }
        }

        Commands::Update { package_manager, .. } => {
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
                        results.insert(name.clone(), ExecutionResult {
                            success: true,
                            message: "Successfully updated".to_string(),
                        });
                    }
                    Err(e) => {
                        error!("{}: {}", name, e);
                        results.insert(name.clone(), ExecutionResult {
                            success: false,
                            message: format!("Error: {}", e),
                        });
                        has_error = true;
                    }
                }
            }
            
            // 실행 결과 요약 출력
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

// 실행 결과 요약을 출력하는 함수
fn print_summary(operation: &str, results: &HashMap<String, ExecutionResult>) {
    println!("\n{} Summary:", operation);
    println!("==============================================");
    
    let mut success_count = 0;
    let mut failure_count = 0;
    
    // 성공한 항목 계산
    for (_, result) in results.iter() {
        if result.success {
            success_count += 1;
        } else {
            failure_count += 1;
        }
    }
    
    // 성공한 항목이 있는 경우에만 출력
    if success_count > 0 {
        println!("✅ Successful:");
        for (name, result) in results.iter() {
            if result.success {
                println!("  - {}: {}", name, result.message);
            }
        }
    }
    
    // 실패한 항목이 있는 경우에만 출력
    if failure_count > 0 {
        if success_count > 0 {
            println!(); // 성공 항목과 실패 항목 사이에 빈 줄 추가
        }
        println!("❌ Failed:");
        for (name, result) in results.iter() {
            if !result.success {
                println!("  - {}: {}", name, result.message);
            }
        }
    }
    
    // 통계 출력
    println!("\nStats: {} total, {} succeeded, {} failed", 
             results.len(), success_count, failure_count);
    println!("==============================================");
}
