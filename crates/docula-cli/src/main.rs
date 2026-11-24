use clap::{Parser, Subcommand};
use docula::ingest::DocumentRouter;
use docula::pipeline::PipelineConfig;
use docula::schema::{DocuValidate, ValidationError};
use docula::vision_codes::OpticalCodeDetector;
use docula::{DoculaPipeline, DocumentSource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "docula")]
#[command(about = "Sovereign pure-Rust document intelligence CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Extracts structured entities from a document using pure-Rust pipeline
    Extract {
        /// Path to the input PDF or image document
        #[arg(short, long)]
        input: PathBuf,

        /// Resolution in DPI for rasterization
        #[arg(long, default_value_t = 150)]
        dpi: u32,

        /// Disables compliance redaction
        #[arg(long)]
        no_redaction: bool,

        /// Disables optical code scanning
        #[arg(long)]
        no_codes: bool,
    },

    /// Inspects PDF object trees and routing heuristics (born-digital vs scanned)
    Inspect {
        /// Path to the PDF document
        #[arg(short, long)]
        input: PathBuf,
    },

    /// Scans document for 1D/2D barcodes, QR codes, and financial metadata
    Codes {
        /// Path to the document
        #[arg(short, long)]
        input: PathBuf,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
struct GenericDocumentOutput {
    summary: String,
    fields: std::collections::HashMap<String, String>,
}

impl DocuValidate for GenericDocumentOutput {
    fn validate_integrity(&self) -> Result<(), Vec<ValidationError>> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Extract {
            input,
            dpi,
            no_redaction,
            no_codes,
        } => handle_extract(input, dpi, no_redaction, no_codes).await,
        Commands::Inspect { input } => handle_inspect(input),
        Commands::Codes { input } => handle_codes(input),
    }
}

async fn handle_extract(
    input: PathBuf,
    dpi: u32,
    no_redaction: bool,
    no_codes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !input.exists() {
        return Err(format!("Input file not found: {}", input.display()).into());
    }

    let config = PipelineConfig {
        dpi,
        enable_redaction: !no_redaction,
        enable_vision_codes: !no_codes,
        ..Default::default()
    };

    let pipeline = DoculaPipeline::builder().with_config(config).build();
    let source = DocumentSource::from(input);
    let result = pipeline.process::<GenericDocumentOutput>(&source).await?;

    let json_output = serde_json::to_string_pretty(&result)?;
    println!("{json_output}");
    Ok(())
}

fn handle_inspect(input: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !input.exists() {
        return Err(format!("Input file not found: {}", input.display()).into());
    }

    let router = DocumentRouter::new();
    let source = DocumentSource::from(input);
    let report = router.route_source(&source)?;

    println!("Document Ingestion Routing Report:");
    println!("- Total Pages: {}", report.total_pages);
    println!("- Scanned Pages Count: {}", report.needs_ocr_count());
    println!(
        "- Born Digital Pages Count: {}",
        report.born_digital_count()
    );
    println!("- Is Entirely Digital: {}", report.is_entirely_digital());
    println!("- Requires OCR: {}", report.requires_ocr());
    Ok(())
}

fn handle_codes(input: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !input.exists() {
        return Err(format!("Input file not found: {}", input.display()).into());
    }

    let img = load_image_from_path(&input)?;
    let detector = OpticalCodeDetector::new();
    let results = detector.detect(&img)?;

    println!("Detected {} optical code(s):", results.len());
    for (i, code) in results.iter().enumerate() {
        println!("[{}] Type: {:?}", i + 1, code.code_type);
        println!("    Raw Content: {}", code.raw_content);
        println!(
            "    Bounding Box: ({:.3}, {:.3}) -> ({:.3}, {:.3})",
            code.bounding_box.top_left.x,
            code.bounding_box.top_left.y,
            code.bounding_box.bottom_right.x,
            code.bounding_box.bottom_right.y
        );
        if let Some(ref fin) = code.financial_payload {
            println!("    Financial Payload: {fin:?}");
        }
    }
    Ok(())
}

fn load_image_from_path(path: &Path) -> Result<image::RgbaImage, Box<dyn std::error::Error>> {
    let dyn_img = image::open(path)?;
    Ok(dyn_img.to_rgba8())
}
