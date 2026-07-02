use std::env;
use std::path::Path;
use std::process;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Err("missing command".to_string());
    };
    match command.as_str() {
        "verify-manifest" => {
            let manifest = one_arg(&mut args, "manifest")?;
            no_more_args(&mut args)?;
            let parsed = minimodel_core::verify_manifest_path(Path::new(&manifest))?;
            println!("MiniModel manifest validation: PASS");
            println!("manifest: {manifest}");
            println!("model.id: {}", parsed.require("model.id")?);
            println!("artifact.kind: {}", parsed.require("artifact.kind")?);
            println!(
                "artifact.acquisition: {}",
                parsed.require("artifact.acquisition")?
            );
            println!(
                "runtime.compatibility: {}",
                parsed.require("runtime.compatibility")?
            );
        }
        "verify-artifact" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            no_more_args(&mut args)?;
            let report =
                minimodel_core::verify_artifact_path(Path::new(&manifest), Path::new(&artifact))?;
            println!("MiniModel artifact validation: PASS");
            println!("manifest: {manifest}");
            println!("artifact: {artifact}");
            println!("bytes: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("slm.model_shape: {}", report.slm.model_shape);
            println!("slm.quantization: {}", report.slm.quantization);
        }
        "write-import-receipt" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            let receipt = one_arg(&mut args, "receipt.minimodel-receipt")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::write_import_receipt_path(
                Path::new(&manifest),
                Path::new(&artifact),
                Path::new(&receipt),
            )?;
            println!("MiniModel import receipt written: {receipt}");
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
        }
        _ => {
            print_usage();
            return Err(format!("unsupported command {command}"));
        }
    }
    Ok(())
}

fn one_arg(args: &mut impl Iterator<Item = String>, label: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing {label} argument"))
}

fn no_more_args(args: &mut impl Iterator<Item = String>) -> Result<(), String> {
    if let Some(extra) = args.next() {
        return Err(format!("unexpected extra argument {extra}"));
    }
    Ok(())
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  minimodel verify-manifest <manifest>");
    eprintln!("  minimodel verify-artifact <manifest> <artifact.slm>");
    eprintln!(
        "  minimodel write-import-receipt <manifest> <artifact.slm> <receipt.minimodel-receipt>"
    );
}
