use image::{DynamicImage, GenericImageView};
use screenshots::Screen;
use anyhow::Result;

pub fn take_screenshots(primary_only: bool) -> Result<Vec<DynamicImage>> {
    let screens = Screen::all()?;
    let mut images = Vec::new();

    for screen in screens {
        if primary_only && !screen.display_info.is_primary {
            continue;
        }

        let image = screen.capture()?;
        let width = image.width();
        let height = image.height();

        let rgba_image = image::RgbaImage::from_raw(width, height, image.into_raw())
            .ok_or_else(|| anyhow::anyhow!("Failed to convert raw screenshot data"))?;

        let dyn_img = DynamicImage::ImageRgba8(rgba_image);
        images.push(dyn_img);
    }

    Ok(images)
}

pub fn is_similar(img1: &DynamicImage, img2: &DynamicImage, threshold: f64) -> bool {
    mean_structured_similarity_index(img1, img2, 255.0) >= threshold
}

pub fn mean_structured_similarity_index(img1: &DynamicImage, img2: &DynamicImage, l: f64) -> f64 {
    // Port of the Python MSSIM logic
    let k1 = 0.01;
    let k2 = 0.03;
    let c1 = (k1 * l).powi(2);
    let c2 = (k2 * l).powi(2);

    let (width1, height1) = img1.dimensions();
    let (width2, height2) = img2.dimensions();

    if width1 != width2 || height1 != height2 {
        return 0.0;
    }

    let mut mu1 = 0.0;
    let mut mu2 = 0.0;

    let mut img1_gray = Vec::with_capacity((width1 * height1) as usize);
    let mut img2_gray = Vec::with_capacity((width2 * height2) as usize);

    for y in 0..height1 {
        for x in 0..width1 {
            let p1 = img1.get_pixel(x, y);
            let p2 = img2.get_pixel(x, y);

            // RGB -> Grayscale conversion weights
            let g1 = 0.2989 * p1[0] as f64 + 0.5870 * p1[1] as f64 + 0.1140 * p1[2] as f64;
            let g2 = 0.2989 * p2[0] as f64 + 0.5870 * p2[1] as f64 + 0.1140 * p2[2] as f64;

            img1_gray.push(g1);
            img2_gray.push(g2);

            mu1 += g1;
            mu2 += g2;
        }
    }

    let n = (width1 * height1) as f64;
    mu1 /= n;
    mu2 /= n;

    let mut sigma1_sq = 0.0;
    let mut sigma2_sq = 0.0;
    let mut sigma12 = 0.0;

    for (g1, g2) in img1_gray.iter().zip(img2_gray.iter()) {
        let diff1 = g1 - mu1;
        let diff2 = g2 - mu2;

        sigma1_sq += diff1.powi(2);
        sigma2_sq += diff2.powi(2);
        sigma12 += diff1 * diff2;
    }

    sigma1_sq /= n;
    sigma2_sq /= n;
    sigma12 /= n;

    ((2.0 * mu1 * mu2 + c1) * (2.0 * sigma12 + c2)) /
    ((mu1.powi(2) + mu2.powi(2) + c1) * (sigma1_sq + sigma2_sq + c2))
}
