#[cfg(windows)]
use std::path::Path;

#[cfg(windows)]
fn main() {
    use std::env;
    use std::path::PathBuf;

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let assets_dir = manifest_dir.join("assets");
    let png_icon = assets_dir.join("whiskey.png");
    let ico_icon = assets_dir.join("whiskey.ico");

    println!("cargo:rerun-if-changed={}", assets_dir.display());

    let icon_path = if ico_icon.is_file() {
        Some(ico_icon)
    } else if png_icon.is_file() {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
        let generated = out_dir.join("distill-tui.ico");
        if let Err(err) = convert_png_to_ico(&png_icon, &generated) {
            println!("cargo:warning=failed to convert whiskey.png into icon: {err}");
            None
        } else {
            Some(generated)
        }
    } else {
        None
    };

    if let Some(icon) = icon_path {
        let mut res = winres::WindowsResource::new();
        res.set_icon(icon.to_string_lossy().as_ref());
        if let Err(err) = res.compile() {
            println!("cargo:warning=failed to embed Windows icon: {err}");
        }
    }
}

#[cfg(windows)]
fn convert_png_to_ico(input: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use image::imageops::FilterType;

    let image = image::open(input)?;
    let resized = image.resize(256, 256, FilterType::Lanczos3);
    resized.save(output)?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
