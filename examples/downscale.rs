fn main() -> Result<(), Box<dyn std::error::Error>> {
    use turbojpeg::{Decompressor, Image, PixelFormat, DownscaleFactor};

    // get the JPEG data
    let jpeg_data = std::fs::read("image.jpg")?;

    // initialize a Decompressor
    let mut decompressor = Decompressor::new()?;

    let scale = DownscaleFactor::OneHalf;

    // read the JPEG header with image size
    let header = decompressor.read_header(&jpeg_data)?.with_scale(scale);

    println!("{},{}", header.width, header.height);

    // prepare the destination image
    let mut image = Image {
        pixels: vec![0; 3 * header.width * header.height],
        width: header.width,
        pitch: 3 * header.width, // we use no padding between rows
        height: header.height,
        format: PixelFormat::RGB,
    };

    // decompress the JPEG data
    decompressor.decompress_with_downscale(&jpeg_data, image.as_deref_mut(), scale)?;

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
