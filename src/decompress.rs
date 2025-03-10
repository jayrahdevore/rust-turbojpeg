use std::convert::TryInto as _;
use raw::tjscalingfactor;
use crate::{Image, YuvImage, raw};
use crate::common::{PixelFormat, Subsamp, Colorspace, Result, Error};
use crate::handle::Handle;

/// Decompresses JPEG data into raw pixels.
#[derive(Debug)]
#[doc(alias = "tjhandle")]
pub struct Decompressor {
    handle: Handle,
    scaling_factor: ScalingFactor,
}

unsafe impl Send for Decompressor {}

/// JPEG header that describes the compressed image.
///
/// The header can be obtained without decompressing the image by calling
/// [`Decompressor::read_header()`] or [`read_header()`][crate::read_header].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DecompressHeader {
    /// Width of the image in pixels (number of columns).
    pub width: usize,
    /// Height of the image in pixels (number of rows).
    pub height: usize,
    /// Chrominance subsampling that is used in the compressed image.
    pub subsamp: Subsamp,
    /// Colorspace of the compressed image.
    pub colorspace: Colorspace,
}

/// Scaling factor for efficient scaling
///
/// Turbojpeg gives us the ability to scaling by skipping DCT coefficients
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScalingFactor {
    /// No scaling
    None,
    /// 1/2 scale resolution
    OneHalf,
    /// 1/4 scale resolution
    OneQuarter,
    /// 1/8 scale resolution
    OneEighth
}

impl Into<tjscalingfactor> for ScalingFactor {
    fn into(self) -> tjscalingfactor {
        match self {
            Self::None => tjscalingfactor { num: 1, denom: 1 },
            Self::OneHalf => tjscalingfactor { num: 1, denom: 2 },
            Self::OneQuarter => tjscalingfactor { num: 1, denom: 4 },
            Self::OneEighth => tjscalingfactor { num: 1, denom: 8 },
        }
    }
}

impl DecompressHeader {
    /// Convenience method that returns a new instance of the decompress header with the given scale
    /// # Example
    ///
    /// ```
    /// // read JPEG data from file
    /// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
    ///
    /// // read JPEG header to check initial dimensions
    /// let header = turbojpeg::read_header(&jpeg_data)?;
    /// assert_eq!((header.width, header.height), (384, 256));
    ///
    ///
    /// // check header size after scale
    ///
    /// let scale_factor = turbojpeg::ScalingFactor::OneHalf;
    /// let scaled_header = header.with_scale(scale_factor);
    /// assert_eq!((scaled_header.width, scaled_header.height), (192, 128));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    pub fn with_scale(&self, factor: ScalingFactor) -> Self {
        let factor: tjscalingfactor = factor.into();
        Self {
            width: (self.width + (factor.denom - 1) as usize) / factor.denom as usize,
            height: (self.height + (factor.denom - 1) as usize) / factor.denom as usize,
            subsamp: self.subsamp,
            colorspace: self.colorspace
        }
    }
}

impl Decompressor {
    /// Create a new decompressor instance.
    #[doc(alias = "tj3Init")]
    pub fn new() -> Result<Decompressor> {
        let handle = Handle::new(raw::TJINIT_TJINIT_DECOMPRESS)?;
        Ok(Self { handle, scaling_factor: ScalingFactor::None })
    }

    /// Read the JPEG header without decompressing the image.
    ///
    /// # Example
    ///
    /// ```
    /// // read JPEG data from file
    /// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
    ///
    /// // initialize a decompressor
    /// let mut decompressor = turbojpeg::Decompressor::new()?;
    ///
    /// // read the JPEG header
    /// let header = decompressor.read_header(&jpeg_data)?;
    /// assert_eq!((header.width, header.height), (384, 256));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[doc(alias = "tj3DecompressHeader")]
    pub fn read_header(&mut self, jpeg_data: &[u8]) -> Result<DecompressHeader> {
        let jpeg_data_len = jpeg_data.len().try_into()
            .map_err(|_| Error::IntegerOverflow("jpeg_data.len()"))?;
        let res = unsafe {
            raw::tj3DecompressHeader(self.handle.as_ptr(), jpeg_data.as_ptr(), jpeg_data_len)
        };
        if res != 0 {
            return Err(self.handle.get_error())
        }

        let width = self.handle.get(raw::TJPARAM_TJPARAM_JPEGWIDTH)
            .try_into().map_err(|_| Error::IntegerOverflow("width"))?;
        let height = self.handle.get(raw::TJPARAM_TJPARAM_JPEGHEIGHT)
            .try_into().map_err(|_| Error::IntegerOverflow("height"))?;
        let subsamp = Subsamp::from_int(self.handle.get(raw::TJPARAM_TJPARAM_SUBSAMP))?;
        let colorspace = Colorspace::from_int(self.handle.get(raw::TJPARAM_TJPARAM_COLORSPACE))?;
        Ok(DecompressHeader { width, height, subsamp, colorspace })
    }

    /// Decompress a JPEG image in `jpeg_data` into `output`.
    ///
    /// The decompressed image is stored in the pixel data of the given `output` image, which must
    /// be fully initialized by the caller. Use [`read_header()`](Decompressor::read_header) to
    /// determine the image size before calling this method.
    ///
    /// # Example
    ///
    /// ```
    /// // read JPEG data from file
    /// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
    ///
    /// // initialize a decompressor
    /// let mut decompressor = turbojpeg::Decompressor::new()?;
    ///
    /// // read the JPEG header
    /// let header = decompressor.read_header(&jpeg_data)?;
    ///
    /// // initialize the image (Image<Vec<u8>>)
    /// let mut image = turbojpeg::Image {
    ///     pixels: vec![0; 4 * header.width * header.height],
    ///     width: header.width,
    ///     pitch: 4 * header.width, // size of one image row in memory
    ///     height: header.height,
    ///     format: turbojpeg::PixelFormat::RGBA,
    /// };
    ///
    /// // decompress the JPEG into the image
    /// // (we use as_deref_mut() to convert from &mut Image<Vec<u8>> into Image<&mut [u8]>)
    /// decompressor.decompress(&jpeg_data, image.as_deref_mut())?;
    /// assert_eq!(&image.pixels[0..4], &[122, 118, 89, 255]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[doc(alias = "tj3Decompress8")]
    pub fn decompress(&mut self, jpeg_data: &[u8], output: Image<&mut [u8]>) -> Result<()> {
        output.assert_valid(output.pixels.len());
        let Image { pixels, width, pitch, height, format } = output;
        let width: libc::c_int = width.try_into().map_err(|_| Error::IntegerOverflow("width"))?;
        let pitch: libc::c_int = pitch.try_into().map_err(|_| Error::IntegerOverflow("pitch"))?;
        let height: libc::c_int = height.try_into().map_err(|_| Error::IntegerOverflow("height"))?;

        let res = unsafe {
            raw::tj3DecompressHeader(
                self.handle.as_ptr(),
                jpeg_data.as_ptr(),
                jpeg_data.len() as raw::size_t,
            )
        };
        if res != 0 {
            return Err(self.handle.get_error())
        }

        let scaled_header = self.read_header(jpeg_data)?.with_scale(self.scaling_factor);

        if width < scaled_header.width as i32 || height < scaled_header.height as i32 {
            return Err(Error::OutputTooSmall(scaled_header.width as i32, scaled_header.height as i32))
        }

        let res = unsafe {
            raw::tj3Decompress8(
                self.handle.as_ptr(),
                jpeg_data.as_ptr(), jpeg_data.len() as raw::size_t,
                pixels.as_mut_ptr(), pitch, format as i32,
            )
        };
        if res != 0 {
            return Err(self.handle.get_error())
        }

        Ok(())
    }


    /// Set scaling factor to take effect when decompressing
    /// Note that this will not work with lossless jpeg images
    // ///
    // /// # Example
    // ///
    // /// ```
    // /// // read JPEG data from file
    // /// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
    // ///
    // /// // define a scaling
    // /// let scaling = turbojpeg::ScalingFactor::OneHalf;
    // ///
    // /// // initialize a decompressor with the scaling factor
    // /// let mut decompressor = turbojpeg::Decompressor::new()?;
    // /// decompressor.set_scaling_factor(scaling);
    // ///
    // /// 
    // ///
    // /// // read the JPEG header, downscaling the width and height
    // /// let scaled_header = decompressor.read_header(&jpeg_data)?.with_scale(scaling);
    // ///
    // /// // initialize the image (Image<Vec<u8>>)
    // /// let mut image = turbojpeg::Image {
    // ///     pixels: vec![0; 4 * scaled_header.width * scaled_header.height],
    // ///     width: scaled_header.width,
    // ///     pitch: 4 * scaled_header.width, // size of one image row in memory
    // ///     height: scaled_header.height,
    // ///     format: turbojpeg::PixelFormat::RGBA,
    // /// };
    // ///
    // /// // decompress the JPEG into the image
    // /// // (we use as_deref_mut() to convert from &mut Image<Vec<u8>> into Image<&mut [u8]>)
    // /// decompressor.decompress_with_scaling(&jpeg_data, image.as_deref_mut(), scaling)?;
    // /// assert_eq!(&image.pixels[0..5], &[125, 121, 92, 255, 127]);
    // ///
    // /// # Ok::<(), Box<dyn std::error::Error>>(())
    // /// ```
    pub fn set_scaling_factor(&mut self, scaling_factor: ScalingFactor) -> Result<()> {
        self.scaling_factor = scaling_factor;
        self.handle.set_scaling_factor(scaling_factor.into())?;
        Ok(())
    }

    /// Decompress a JPEG image in `jpeg_data` into `output` as YUV without changing color space.
    ///
    /// The decompressed image is stored in the pixel data of the given `output` image, which must
    /// be fully initialized by the caller. Use [`read_header()`](Decompressor::read_header) to
    /// determine the image size before calling this method.
    ///
    /// # Example
    ///
    /// ```
    /// // read JPEG data from file
    /// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
    ///
    /// // initialize a decompressor
    /// let mut decompressor = turbojpeg::Decompressor::new()?;
    ///
    /// // read the JPEG header
    /// let header = decompressor.read_header(&jpeg_data)?;
    /// // calculate YUV pixels length
    /// let align = 4;
    /// let yuv_pixels_len = turbojpeg::yuv_pixels_len(header.width, align, header.height, header.subsamp);
    ///
    /// // initialize the image (YuvImage<Vec<u8>>)
    /// let mut image = turbojpeg::YuvImage {
    ///     pixels: vec![0; yuv_pixels_len.unwrap()],
    ///     width: header.width,
    ///     align,
    ///     height: header.height,
    ///     subsamp: header.subsamp,
    /// };
    ///
    /// // decompress the JPEG into the image
    /// // (we use as_deref_mut() to convert from &mut YuvImage<Vec<u8>> into YuvImage<&mut [u8]>)
    /// decompressor.decompress_to_yuv(&jpeg_data, image.as_deref_mut())?;
    /// assert_eq!(&image.pixels[0..4], &[116, 117, 118, 119]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[doc(alias = "tj3DecompressToYUV8")]
    pub fn decompress_to_yuv(&mut self, jpeg_data: &[u8], output: YuvImage<&mut [u8]>) -> Result<()> {
        output.assert_valid(output.pixels.len());
        let YuvImage { pixels, width, align, height, subsamp: _ } = output;
        let width: libc::c_int = width.try_into().map_err(|_| Error::IntegerOverflow("width"))?;
        let align = align.try_into().map_err(|_| Error::IntegerOverflow("align"))?;
        let height: libc::c_int = height.try_into().map_err(|_| Error::IntegerOverflow("height"))?;
        let jpeg_data_len = jpeg_data.len().try_into()
            .map_err(|_| Error::IntegerOverflow("jpeg_data.len()"))?;

        let scaled_header = self.read_header(jpeg_data)?.with_scale(self.scaling_factor);

        if width < scaled_header.width as i32 || height < scaled_header.height as i32 {
            return Err(Error::OutputTooSmall(scaled_header.width as i32, scaled_header.height as i32))
        }

        let res = unsafe {
            raw::tj3DecompressToYUV8(
                self.handle.as_ptr(),
                jpeg_data.as_ptr(), jpeg_data_len,
                pixels.as_mut_ptr(), align,
            )
        };
        if res != 0 {
            return Err(self.handle.get_error())
        }

        Ok(())
    }
}

/// Decompress a JPEG image.
///
/// Returns a newly allocated image with the given pixel `format`. If you have specific
/// requirements regarding memory layout or allocations, please see [`Decompressor`].
///
/// # Example
///
/// ```
/// // read JPEG data from file
/// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
///
/// // decompress the JPEG into RGB image
/// let image = turbojpeg::decompress(&jpeg_data, turbojpeg::PixelFormat::RGB)?;
/// assert_eq!(image.format, turbojpeg::PixelFormat::RGB);
/// assert_eq!((image.width, image.height), (384, 256));
/// assert_eq!(image.pixels.len(), 384 * 256 * 3);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn decompress(jpeg_data: &[u8], format: PixelFormat) -> Result<Image<Vec<u8>>> {
    let mut decompressor = Decompressor::new()?;
    let header = decompressor.read_header(jpeg_data)?;

    let pitch = header.width * format.size();
    let mut image = Image {
        pixels: vec![0; header.height * pitch],
        width: header.width,
        pitch,
        height: header.height,
        format,
    };
    decompressor.decompress(jpeg_data, image.as_deref_mut())?;

    Ok(image)
}

/// Decompress a JPEG image to YUV.
///
/// Returns a newly allocated YUV image with row alignment of 4. If you have specific requirements
/// regarding memory layout or allocations, please see [`Decompressor`].
///
/// # Example
///
/// ```
/// // read JPEG data from file
/// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
///
/// // decompress the JPEG into YUV image
/// let image = turbojpeg::decompress_to_yuv(&jpeg_data)?;
/// assert_eq!((image.width, image.height), (384, 256));
/// assert_eq!(image.pixels.len(), 294912);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn decompress_to_yuv(jpeg_data: &[u8]) -> Result<YuvImage<Vec<u8>>> {
    let mut decompressor = Decompressor::new()?;
    let header = decompressor.read_header(jpeg_data)?;
    let align = 4;
    let yuv_pixels_len = yuv_pixels_len(
        header.width,
        align,
        header.height,
        header.subsamp,
    )?;

    let mut yuv_image = YuvImage {
        pixels: vec![0; yuv_pixels_len],
        width: header.width,
        align,
        height: header.height,
        subsamp: header.subsamp,
    };
    decompressor.decompress_to_yuv(jpeg_data, yuv_image.as_deref_mut())?;

    Ok(yuv_image)
}

/// Determine size in bytes of a YUV image.
///
/// Calculates the size for [`YuvImage::pixels`] based on the image width, height, chrominance
/// subsampling and row alignment.
///
/// Returns an error on integer overflow. You can just `.unwrap()` the result if you don't care
/// about this edge case.
/// 
/// # Example
///
/// ```
/// // read JPEG data from file
/// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
///
/// // read the JPEG header
/// let header = turbojpeg::read_header(&jpeg_data)?;
/// // get YUV pixels length
/// let align = 4;
/// let yuv_pixels_len = turbojpeg::yuv_pixels_len(header.width, align, header.height, header.subsamp);
/// assert_eq!(yuv_pixels_len.unwrap(), 294912);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[doc(alias = "tj3YUVBufSize")]
pub fn yuv_pixels_len(width: usize, align: usize, height: usize, subsamp: Subsamp) -> Result<usize> {
    let width = width.try_into().map_err(|_| Error::IntegerOverflow("width"))?;
    let align = align.try_into().map_err(|_| Error::IntegerOverflow("align"))?;
    let height = height.try_into().map_err(|_| Error::IntegerOverflow("height"))?;
    let len = unsafe { raw::tj3YUVBufSize(width, align, height, subsamp as libc::c_int) };
    let len = len.try_into().map_err(|_| Error::IntegerOverflow("yuv size"))?;
    Ok(len)
}

/// Read the JPEG header without decompressing the image.
///
/// # Example
///
/// ```
/// // read JPEG data from file
/// let jpeg_data = std::fs::read("examples/parrots.jpg")?;
///
/// // read the JPEG header
/// let header = turbojpeg::read_header(&jpeg_data)?;
/// assert_eq!((header.width, header.height), (384, 256));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn read_header(jpeg_data: &[u8]) -> Result<DecompressHeader> {
    let mut decompressor = Decompressor::new()?;
    decompressor.read_header(jpeg_data)
}
