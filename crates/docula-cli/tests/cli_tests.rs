use assert_cmd::Command;
use image::{ImageEncoder, Rgba, RgbaImage};
use pdf_oxide::api::Pdf;
use predicates::prelude::*;
use rxing::{BarcodeFormat, MultiFormatWriter, Writer};

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("docula").expect("binary must exist");
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Sovereign pure-Rust"));
}

#[test]
fn test_cli_missing_file() {
    let mut cmd = Command::cargo_bin("docula").expect("binary must exist");
    cmd.args(["inspect", "--input", "non_existent_file.pdf"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Input file not found"));
}

#[test]
fn test_cli_inspect() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let pdf_path = temp_dir.path().join("sample.pdf");

    let mut pdf = Pdf::from_html("<h1>Inspection Test</h1><p>Born digital content</p>").unwrap();
    pdf.save(&pdf_path).unwrap();

    let mut cmd = Command::cargo_bin("docula").expect("binary must exist");
    cmd.args(["inspect", "--input", pdf_path.to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Document Ingestion Routing Report",
        ))
        .stdout(predicate::str::contains("Total Pages: 1"));
}

#[test]
fn test_cli_codes() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let img_path = temp_dir.path().join("qr.png");

    let epc_payload = "BCD\n001\n1\nSCT\nTESTBIC\nAcme Corp\nES1234567890123456789012\nEUR450.75\n\nRF18539007547034\nInvoice 1234\n";
    let bitmatrix = MultiFormatWriter
        .encode(epc_payload, &BarcodeFormat::QR_CODE, 200, 200)
        .expect("QR encode");

    let width = bitmatrix.getWidth();
    let height = bitmatrix.getHeight();
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = if bitmatrix.get(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            img.put_pixel(x, y, pixel);
        }
    }

    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("PNG encoding");

    std::fs::write(&img_path, bytes).unwrap();

    let mut cmd = Command::cargo_bin("docula").expect("binary must exist");
    cmd.args(["codes", "--input", img_path.to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Detected 1 optical code(s)"))
        .stdout(predicate::str::contains("QrCode"))
        .stdout(predicate::str::contains("SepaEpc"));
}
