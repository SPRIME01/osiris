use anyhow::Result;
use image::DynamicImage;
use leptess::{LepTess, Variable};
use std::env;

pub fn extract_text_from_image(image: &DynamicImage) -> Result<String> {
    // Determine language, fallback to "eng"
    let lang = env::var("TESSERACT_LANG").unwrap_or_else(|_| "eng".to_string());

    // Convert DynamicImage to TIFF in memory, as leptess supports reading from memory
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, image::ImageFormat::Tiff)?;
    let tiff_data = cursor.into_inner();

    let mut tess = LepTess::new(None, &lang)
        .map_err(|e| anyhow::anyhow!("Failed to initialize Tesseract: {:?}", e))?;

    tess.set_variable(Variable::TesseditPagesegMode, "3")
        .map_err(|e| anyhow::anyhow!("Failed to set PSM: {:?}", e))?;

    tess.set_image_from_mem(&tiff_data)
        .map_err(|e| anyhow::anyhow!("Failed to set image: {:?}", e))?;

    let text = tess.get_utf8_text()
        .map_err(|e| anyhow::anyhow!("Failed to extract text: {:?}", e))?;

    Ok(text)
}
