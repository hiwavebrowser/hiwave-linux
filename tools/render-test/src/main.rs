//! Render test harness for RustKit browser engine.
//!
//! Provides commands for:
//! - Running golden screenshot tests
//! - Comparing screenshots against baselines
//! - Generating diff reports
//!
//! ## Usage
//!
//! ```bash
//! # Run all golden tests
//! render-test run-all --output .ai/artifacts/
//!
//! # Compare two images
//! render-test diff expected.png actual.png --output diff.png
//!
//! # Generate a verification report
//! render-test report --artifacts .ai/artifacts/ --output report.json
//! ```

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod diff;

#[derive(Parser)]
#[command(name = "render-test")]
#[command(about = "Render test harness for RustKit browser engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two PNG images and produce a diff
    Diff {
        /// Expected (golden) image
        expected: PathBuf,
        /// Actual (test) image
        actual: PathBuf,
        /// Output diff image path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Per-channel difference threshold (0-255)
        #[arg(short, long, default_value = "0")]
        threshold: u8,
        /// Output JSON report path
        #[arg(long)]
        report: Option<PathBuf>,
    },
    
    /// Generate a verification report from artifacts
    Report {
        /// Artifacts directory
        #[arg(short, long, default_value = ".ai/artifacts")]
        artifacts: PathBuf,
        /// Output report path
        #[arg(short, long)]
        output: PathBuf,
    },
    
    /// Run minimal golden scene tests (requires hiwave binary)
    RunScenes {
        /// Output directory for screenshots
        #[arg(short, long, default_value = ".ai/artifacts")]
        output: PathBuf,
        /// Golden images directory for comparison
        #[arg(short, long)]
        golden: Option<PathBuf>,
        /// Path to hiwave executable
        #[arg(long, default_value = "target/release/hiwave.exe")]
        hiwave: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Diff {
            expected,
            actual,
            output,
            threshold,
            report,
        } => {
            let result = if let Some(out) = output {
                diff::compare_and_visualize(&expected, &actual, &out, threshold)?
            } else {
                diff::compare_images(&expected, &actual, threshold)?
            };
            
            println!("Comparison result:");
            println!("  Total pixels: {}", result.total_pixels);
            println!("  Diff pixels:  {} ({:.2}%)", result.diff_pixels, result.diff_percent);
            println!("  Max diff:     {}", result.max_diff);
            println!("  Mean diff:    {:.2}", result.mean_diff);
            println!("  Matches:      {}", result.matches);
            
            if let Some(report_path) = report {
                let json = serde_json::to_string_pretty(&result)?;
                std::fs::write(&report_path, json)?;
                println!("Report written to: {}", report_path.display());
            }
            
            if !result.matches {
                std::process::exit(1);
            }
        }
        
        Commands::Report { artifacts, output } => {
            generate_report(&artifacts, &output)?;
        }
        
        Commands::RunScenes { output, golden, hiwave } => {
            run_scenes(&output, golden.as_deref(), &hiwave)?;
        }
    }
    
    Ok(())
}

/// Verification report structure.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct VerificationReport {
    timestamp: String,
    total_tests: usize,
    passed: usize,
    failed: usize,
    tests: Vec<TestResult>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TestResult {
    name: String,
    passed: bool,
    diff_percent: Option<f64>,
    error: Option<String>,
}

fn generate_report(artifacts: &PathBuf, output: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    
    let mut tests = Vec::new();
    let mut passed = 0;
    let mut failed = 0;
    
    // Look for diff result JSON files in artifacts
    if artifacts.exists() {
        for entry in fs::read_dir(artifacts)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(diff_result) = serde_json::from_str::<diff::DiffResult>(&content) {
                        let name = path.file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        
                        if diff_result.matches {
                            passed += 1;
                        } else {
                            failed += 1;
                        }
                        
                        tests.push(TestResult {
                            name,
                            passed: diff_result.matches,
                            diff_percent: Some(diff_result.diff_percent),
                            error: None,
                        });
                    }
                }
            }
        }
    }
    
    let report = VerificationReport {
        timestamp: chrono_lite_timestamp(),
        total_tests: tests.len(),
        passed,
        failed,
        tests,
    };
    
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(output, json)?;
    
    println!("Verification report: {}/{} tests passed", passed, passed + failed);
    println!("Written to: {}", output.display());
    
    if failed > 0 {
        std::process::exit(1);
    }
    
    Ok(())
}

fn run_scenes(
    output: &PathBuf,
    golden: Option<&std::path::Path>,
    hiwave: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::process::Command;
    
    // Create output directory
    fs::create_dir_all(output)?;
    
    // Define test scenes (built-in deterministic scenes in hiwave screenshot mode)
    let scenes = [
        "solid_colors",
        "text_basic",
        "alpha_blend",
        "borders",
    ];
    
    println!("Running {} render test scenes...", scenes.len());
    
    for name in scenes {
        println!("  Running scene: {}", name);

        // We use the deterministic GPU readback capture of the Content view.
        // hiwave will write multiple artifacts; we key off this expected path.
        let screenshot_path = output.join(format!("{}_gpu_content.png", name));

        // Run hiwave with screenshot capture (native mode)
        let status = Command::new(hiwave)
            .args([
                "--render-test",
                "--render-test-scene",
                name,
                "--screenshot-out",
                output.to_str().unwrap(),
                "--screenshot-frames",
                "3",
            ])
            .status();
        
        match status {
            Ok(s) if s.success() => {
                println!("    [OK] Screenshot captured");
                
                // Compare against golden if available
                if let Some(golden_dir) = golden {
                    let golden_path = golden_dir.join(format!("{}_gpu_content.png", name));
                    if golden_path.exists() {
                        let diff_path = output.join(format!("{}_diff.png", name));
                        let result_path = output.join(format!("{}_result.json", name));
                        
                        match diff::compare_and_visualize(&golden_path, &screenshot_path, &diff_path, 5) {
                            Ok(result) => {
                                let json = serde_json::to_string_pretty(&result)?;
                                fs::write(&result_path, json)?;
                                
                                if result.matches {
                                    println!("    [PASS] Matches golden image");
                                } else {
                                    println!("    [FAIL] Differs from golden ({:.2}% diff)", result.diff_percent);
                                }
                            }
                            Err(e) => {
                                println!("    [ERROR] Comparison failed: {}", e);
                            }
                        }
                    } else {
                        println!("    [SKIP] No golden image found");
                    }
                }
            }
            Ok(s) => {
                println!("    [ERROR] HiWave exited with code: {:?}", s.code());
            }
            Err(e) => {
                println!("    [ERROR] Failed to run HiWave: {}", e);
                println!("    Note: Make sure hiwave is built with --features native-win32");
            }
        }
    }
    
    Ok(())
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let remaining = (days % 365) as u32;
    let month = remaining / 30 + 1;
    let day = remaining % 30 + 1;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        years, month, day, hours, minutes, seconds
    )
}

