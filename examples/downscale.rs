fn main() -> Result<(), Box<dyn std::error::Error>> {
    use turbojpeg::{Decompressor, Image, PixelFormat};

    // get the JPEG data
    let jpeg_data = std::fs::read("image.jpg")?;

    // initialize a Decompressor
    let mut decompressor = Decompressor::new()?;

    decompressor.set_downscale_factor(8);

    // read the JPEG header with image size
    let header = decompressor.read_header(&jpeg_data)?;
    let (width, height) = ((header.width + 7) / 8, (header.height + 7) / 8);

    println!("{},{}", width, height);

    // prepare the destination image
    let mut image = Image {
        pixels: vec![0; 3 * width * height],
        width: width,
        pitch: 3 * width, // we use no padding between rows
        height: height,
        format: PixelFormat::RGB,
    };

    // decompress the JPEG data 
    decompressor.decompress(&jpeg_data, image.as_deref_mut())?;

    // use the raw pixel data
    println!("{:?}", &image.pixels[0..9]);

    // initialize a Compressor
    let mut compressor = turbojpeg::Compressor::new()?;

    compressor.set_quality(40)?;

    // compress the Image to a Vec<u8> of JPEG data
    let jpeg_data = compressor.compress_to_vec(image.as_deref())?;

    std::fs::write("image-downscaled.jpg", jpeg_data)?;
    Ok(())
}
